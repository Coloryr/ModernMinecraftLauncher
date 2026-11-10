//! LiteLoader 数据
//!
//! LiteLoader 版本表的读写与缓存。

use super::*;

use std::{collections::HashMap, sync::Arc};

use mml_base::serialize_tools;
use mml_config::config_save;
use mml_names::uuids;

use crate::loader::liteloader_meta_obj::{LiteloaderMetaObj, LiteloaderVersionObj};

/// 加载liteloader版本信息
///
/// 从文件读取并填充缓存；文件不存在时忽略，解析失败记录日志
pub(super) fn load_liteloader() {
    let local = LITELOADER_FILE.get().unwrap();
    if !local.exists() || !local.is_file() {
        return;
    }

    let json = serialize_tools::json_from_file::<LiteloaderMetaObj>(local);

    match json {
        Err(err) => {
            mml_log::error_type(err);
        }
        Ok(data) => {
            let mut list = LITE_LOADER.write().unwrap();
            list.clear();

            list.extend(data.versions.into_iter().map(|(k, v)| (k, Arc::new(v))));
        }
    };
}

/// 保存liteloader版本信息
///
/// 将缓存写回文件；写盘失败仅记录错误，不影响调用方
pub(super) fn save_liteloader() {
    let file = LITELOADER_FILE.get().unwrap();
    let list = LITE_LOADER.read().unwrap();
    let value = &*list;

    let ref_map: HashMap<&String, &LiteloaderVersionObj> =
        value.iter().map(|(k, v)| (k, v.as_ref())).collect();

    config_save::save(uuids::LITELOADER_UUID, &ref_map, file);
}

/// 保存liteloader信息
///
/// - `obj`: LiteLoader 版本元数据
pub fn add_liteloader(obj: LiteloaderMetaObj) {
    // 同 `add_optifine`：写锁先释放，再调内部取读锁的 `save_liteloader`，
    // 否则同线程自死锁，这是"加载器列表卡在 1/6"的直接原因。
    let mut list = LITE_LOADER.write().unwrap();
    list.extend(obj.versions.into_iter().map(|(k, v)| (k, Arc::new(v))));

    drop(list);

    save_liteloader();
}

/// 获取liteloader信息
///
/// - `version`: 游戏版本号
///
/// # 返回值
///
/// 返回 LiteLoader 版本信息；不存在返回 `None`
pub fn get_liteloader(version: &str) -> Option<Arc<LiteloaderVersionObj>> {
    let list = LITE_LOADER.read().unwrap();
    // 缓存键为版本前缀，如 1.12 / 1.7.10，取能匹配游戏版本的最长前缀
    list.iter()
        .filter(|(key, _)| version.starts_with(key.as_str()))
        .max_by_key(|(key, _)| key.len())
        .map(|(_, value)| value.clone())
}
