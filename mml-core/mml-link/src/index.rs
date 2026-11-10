//! 索引与落盘
//!
//! 下载目录、索引库的打开与延迟写盘，以及每个实例的链接记录。
use std::path::{Path, PathBuf};

use mml_base::serialize_tools;
use mml_config::config_save;
use mml_names::{
    i18_items::error_type::{CoreResult, ErrorType, FileSystemErrorData},
    names, uuids,
};
use mml_sys::path_helper;
use uuid::Uuid;

use crate::index_store::Store;
use crate::info::{LinkInfoObj, ResourceInfoObj, ResourceType};

use super::*;

/// 初始化公共存储
///
/// 公共目录建在下载目录下，索引落在运行目录；实例映射记录跟着实例目录走。
///
/// - `path`: 启动器运行根目录
///
/// # 返回值
///
/// 目录创建失败时返回对应错误
pub fn init<P: AsRef<Path>>(path: P) -> CoreResult<()> {
    let path = path.as_ref();
    let download = mml_downloader::get_download_path();

    let dir = MOD_DOWNLOAD_PATH.get_or_init(|| download.join(names::GAME_MODS_DIR));
    path_helper::create_dir_all(dir)?;
    let dir =
        RESOURCE_PACK_DOWNLOAD_PATH.get_or_init(|| download.join(names::GAME_RESOURCEPACKS_DIR));
    path_helper::create_dir_all(dir)?;

    INDEX_PATH.get_or_init(|| path.join(names::DOWNLOAD_INDEX_DIR));
    LINK_DATA.get_or_init(Default::default);

    // 索引换成了事务库；旧 JSON 的导入由 mml-upgrade 在启动时单独跑
    let index_path = INDEX_PATH.get().unwrap();
    match Store::open(index_path) {
        Ok(store) => {
            let _ = INDEX.set(store);
        }
        // 打不开就别拦启动：索引退化成只在内存里，下次下载仍会照常写文件
        Err(err) => mml_log::error_type(ErrorType::FileSystemError(FileSystemErrorData {
            path: index_path.clone(),
            error: err,
        })),
    }

    Ok(())
}

/// 取索引库句柄
///
/// # 返回值
///
/// 库没打开时返回 `None`
pub fn index_store() -> Option<&'static Store> {
    INDEX.get()
}

/// 读取一个实例的映射记录
///
/// - `base`: 实例根目录，记录文件就在它下面
///
/// # 返回值
///
/// 读不到或解析失败时返回空列表
pub(super) fn read_link_data(base: &Path) -> Vec<LinkInfoObj> {
    let file = base.join(names::LINK_INFO_FILE);
    if !file.is_file() {
        return Vec::new();
    }

    match serialize_tools::json_from_file::<Vec<LinkInfoObj>>(&file) {
        Ok(data) => data,
        Err(err) => {
            mml_log::error_type(err);
            Vec::new()
        }
    }
}

/// 写入一条索引记录，键相同的覆盖
///
/// - `resource`: 资源类型
/// - `pid`: 项目 ID
/// - `info`: 记录
pub(super) fn save_entry(resource: ResourceType, pid: &str, info: &ResourceInfoObj) {
    let Some(store) = INDEX.get() else {
        return;
    };
    let payload = match serde_json::to_vec(info) {
        Ok(payload) => payload,
        Err(err) => {
            log_index_error(err.to_string());
            return;
        }
    };
    if let Err(err) = store.put(resource.kind(), pid, &info.hash, &payload) {
        log_index_error(err);
    }
}

/// 把索引里攒着的改动落盘
///
/// 批量映射几万条时逐条 fsync 太慢，[Store] 会先攒批次；每个实例处理完调一次这里，
/// 被强杀也只丢最后一个实例的记录
pub fn flush_index() {
    let Some(store) = INDEX.get() else {
        return;
    };
    if let Err(err) = store.flush() {
        log_index_error(err);
    }
}

/// 记录一次索引读写失败
///
/// - `error`: 错误描述
pub(super) fn log_index_error(error: String) {
    let path = INDEX_PATH
        .get()
        .cloned()
        .unwrap_or_else(|| PathBuf::from(names::DOWNLOAD_INDEX_DIR));
    mml_log::error_type(ErrorType::FileSystemError(FileSystemErrorData {
        path,
        error,
    }));
}

/// 标记一个实例的映射记录待保存
///
/// 不立刻写盘：批量映射几万个文件时，每条都整份序列化一次就是 O(n²)。
/// 先记在这里，由 [`flush_pending`] 每个实例收尾时统一写一次
///
/// - `instance`: 实例 uuid
/// - `base`: 实例根目录，记录文件就写在这里
pub(super) fn save_link_data(instance: Uuid, base: &Path) {
    LINK_DIRTY
        .lock()
        .unwrap()
        .insert((instance, base.to_path_buf()));
}

/// 把待写的映射记录落盘
pub fn flush_link_data() {
    let dirty: Vec<(Uuid, PathBuf)> = {
        let mut dirty = LINK_DIRTY.lock().unwrap();
        dirty.drain().collect()
    };
    for (instance, base) in dirty {
        let data = link_data()
            .read()
            .unwrap()
            .get(&instance)
            .cloned()
            .unwrap_or_default();
        config_save::save(
            uuids::mix_uuid(instance, uuids::LINK_UUID),
            &data,
            base.join(names::LINK_INFO_FILE),
        );
    }
}

/// 把索引与映射记录里攒着的改动一起落盘
pub fn flush_pending() {
    flush_index();
    flush_link_data();
}

/// 获取公共模组目录
///
/// # 返回值
///
/// 下载目录下的 `mods`
pub fn get_mod_download_path() -> PathBuf {
    MOD_DOWNLOAD_PATH.get().unwrap().clone()
}

/// 获取公共资源包目录
///
/// # 返回值
///
/// 下载目录下的 `resourcepacks`
pub fn get_resource_pack_download_path() -> PathBuf {
    RESOURCE_PACK_DOWNLOAD_PATH.get().unwrap().clone()
}
