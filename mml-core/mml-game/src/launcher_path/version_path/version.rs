//! 版本数据
//!
//! 版本清单、版本 json 的读取与缓存，以及在线更新检查。

use super::*;

use std::{path::PathBuf, sync::Arc};

use mml_base::{
    hash_helper::{self, HashType},
    serialize_tools,
};
use mml_config::config_obj::SourceLocal;
use mml_names::{
    i18_items::error_type::{CoreResult, DataNotFoundData, ErrorType},
    names,
};
use mml_net::{mojang_api, url_helper};
use mml_sys::path_helper;
use tokio::task;

use crate::error::serializer_err;
use crate::mojang::{
    game_arg_obj::GameArgObj,
    version_obj::{VersionObj, VersionsObj},
};

/// 获取目录
///
/// # 返回值
///
/// 返回版本根目录
pub fn get_version_dir() -> PathBuf {
    BASE_DIR.get().unwrap().clone()
}

/// 从在线获取版本信息
///
/// 优先使用默认源，失败后回退官方源再取一次。
///
/// # 返回值
///
/// 成功返回 `Ok(())`；两次请求或解析失败返回对应错误
pub(super) async fn get_version_from_online() -> CoreResult<()> {
    /// 把版本清单写回本地缓存文件
    fn save_versions(data: &Vec<u8>) {
        let file = BASE_DIR.get().unwrap().join(names::VERSION_FILE);
        path_helper::write_bytes(&file, data).unwrap();
    }

    let res = mojang_api::get_versions(None).await;
    if res.is_ok() {
        let data: Vec<u8> = res.unwrap();
        let json = serialize_tools::json_from_bytes::<VersionObj>(&data);
        if json.is_ok() {
            *VERSION.write().unwrap() = Some(Arc::new(json.unwrap()));
            save_versions(&data);

            return Ok(());
        }
    }

    // 获取失败再从官方源获取一次
    let data = mojang_api::get_versions(Some(SourceLocal::Offical)).await?;
    let json = serialize_tools::json_from_bytes::<VersionObj>(&data).map_err(serializer_err)?;
    *VERSION.write().unwrap() = Some(Arc::new(json));
    save_versions(&data);

    Ok(())
}

/// 从文件读取版本信息
/// 如果文件不存在就去在线读取
///
/// # 返回值
///
/// 成功返回 `Ok(())`；在线读取失败返回对应错误
pub(super) async fn read_version() -> CoreResult<()> {
    let local = BASE_DIR.get().unwrap().join(names::VERSION_FILE);
    if local.exists()
        && local.is_file()
        && let Ok(json) = serialize_tools::json_from_file::<VersionObj>(&local)
    {
        *VERSION.write().unwrap() = Some(Arc::new(json));
        task::spawn(async { get_version_from_online().await });
        return Ok(());
    }

    get_version_from_online().await?;

    Ok(())
}

/// 获取最新版本
///
/// # 返回值
///
/// 返回最新正式版版本号；无缓存返回内置默认版本号
pub fn get_latest_version() -> String {
    let read = VERSION.read().unwrap();
    let versions = read.as_ref();
    match versions {
        Some(data) => data.latest.release.clone(),
        None => names::NOW_VERSION.to_string(),
    }
}

/// 判断游戏版本是否存在
///
/// 版本清单平时由 [`get_version_obj_online`] 加载并落盘缓存，但这个函数是**同步**的，
/// 调用它的地方，如外部启动器实例导入、Forge 坐标识别，可能在清单加载**之前**就跑到这里 ——
/// 那样会恒为 false，把明明认识的版本判成未知版本。所以内存里没有时，这里先从本地缓存
/// [`names::VERSION_FILE`] 同步读一次；读不到、没缓存过也没加载过，才算真的不知道。
///
/// - `version`: 游戏版本号
///
/// # 返回值
///
/// 返回该版本是否存在于版本列表中；清单不可用返回 `false`
pub fn have_version(version: &str) -> bool {
    ensure_version_loaded();
    let read = VERSION.read().unwrap();
    let versions = read.as_ref();
    match versions {
        Some(data) => data
            .versions
            .iter()
            .find(|data| data.id == version)
            .is_some(),
        None => false,
    }
}

/// 内存里没有版本清单时，从本地缓存文件同步读一次
///
/// 只读文件、不发请求：联网刷新那条异步路径 [`read_version`] 仍由需要清单的地方触发。
/// 读到之后进 [`VERSION`] 缓存，后续调用不再读盘。
pub(super) fn ensure_version_loaded() {
    if VERSION.read().unwrap().is_some() {
        return;
    }

    let Some(dir) = BASE_DIR.get() else {
        return;
    };

    let file = dir.join(names::VERSION_FILE);
    if !file.is_file() {
        return;
    }

    if let Ok(json) = serialize_tools::json_from_file::<VersionObj>(&file) {
        *VERSION.write().unwrap() = Some(Arc::new(json));
    }
}

/// 获取游戏版本列表
/// 不存在就去读取文件
///
/// # 返回值
///
/// 返回版本列表；读取失败返回对应错误
pub async fn get_version_obj_online() -> CoreResult<Arc<VersionObj>> {
    if let Some(v) = VERSION.read().unwrap().as_ref() {
        return Ok(v.clone());
    }
    read_version().await?;

    Ok(VERSION.read().unwrap().as_ref().unwrap().clone())
}

/// 是否存在版本信息
///
/// # 返回值
///
/// 返回版本信息是否已加载
pub async fn is_have_version_info() -> bool {
    VERSION.read().unwrap().is_some()
}

/// 添加版本信息
///
/// - `obj`: 游戏数据
///
/// # 返回值
///
/// 返回解析出的启动参数；下载或解析失败返回对应错误
pub async fn add_game(obj: &VersionsObj) -> CoreResult<Arc<GameArgObj>> {
    let mut url = obj.url.clone();
    url_helper::change_source(&mut url);

    let data = mojang_api::get_assets(&url).await?;
    let json = serialize_tools::json_from_bytes::<GameArgObj>(&data)?;
    let file = BASE_DIR.get().unwrap().join(format!("{}.json", obj.id));
    path_helper::write_bytes(&file, &data).unwrap();

    let mut list = GAME_ARGS.write().unwrap();

    list.insert(obj.id.clone(), Arc::new(json));
    let json = list.get(&obj.id).unwrap();
    Ok(json.clone())
}

/// 获取版本信息
///
/// - `version`: 游戏版本
///
/// # 返回值
///
/// 返回启动参数；本地文件缺失或解析失败返回对应错误
pub fn get_version(version: &str) -> CoreResult<Arc<GameArgObj>> {
    // 先查缓存。读锁必须在块内释放：同线程读锁未放再取写锁会自死锁
    // std RwLock 不可升级，缓存未命中时正是这个顺序导致主线程永久卡死。
    let cached = {
        let list = GAME_ARGS.read().unwrap();
        list.get(version).cloned()
    };
    if let Some(data) = cached {
        return Ok(data);
    }

    let local = BASE_DIR
        .get()
        .unwrap()
        .join(format!("{}{}", version, names::JSON_DOT_EXT));
    let json = serialize_tools::json_from_file::<GameArgObj>(&local)?;
    let mut list = GAME_ARGS.write().unwrap();
    // 双检：并发重复解析时以先入者为准
    Ok(list
        .entry(String::from(version))
        .or_insert_with(|| Arc::new(json))
        .clone())
}

/// 检查游戏版本更新
///
/// - `mc`: 游戏版本
///
/// # 返回值
///
/// 返回最新的启动参数；在线无此版本或下载失败返回对应错误
pub async fn check_update(mc: &str) -> CoreResult<Arc<GameArgObj>> {
    // 直接从在线更新数据
    get_version_from_online().await?;

    let versions = get_version_obj_online().await?;
    let item = versions
        .versions
        .iter()
        .filter(|&item| item.id.eq_ignore_ascii_case(mc))
        .next();

    match item {
        // 在线没有这个版本号：本地有版本 json 就按自定义版本用（如从 .minecraft 导入的
        // LabyMod 这类），启动参数直接读本地那份，不去在线找
        None => {
            let local = BASE_DIR.get().unwrap().join(format!("{mc}.json"));
            if local.is_file() {
                return get_version(mc);
            }
            Err(ErrorType::DataNotFound(DataNotFoundData::Version(
                mc.to_string(),
            )))
        }
        Some(item) => {
            let local = BASE_DIR.get().unwrap().join(format!("{}.json", mc));
            // 本地没有版本 json，如首次安装整合包，直接在线下载
            if !local.exists() {
                Ok(add_game(item).await?)
            } else {
                let sha1 = hash_helper::gen_hash_from_file_async(HashType::Sha1, &local).await?;
                if sha1 != item.sha1 {
                    Ok(add_game(item).await?)
                } else {
                    Ok(get_version(mc)?)
                }
            }
        }
    }
}
