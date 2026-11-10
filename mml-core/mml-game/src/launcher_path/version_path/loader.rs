//! 加载器数据
//!
//! Forge / NeoForge / Fabric / Quilt / 自定义加载器的登记与查询。

use super::*;

use std::sync::Arc;

use mml_base::{serialize_tools, version_parse};
use mml_names::names;
use mml_sys::path_helper;
use uuid::Uuid;

use crate::error::serializer_err;
use crate::{
    launcher::custom_loader_obj::CustomLoaderType,
    loader::{
        LoaderKey, fabric_loader_obj::FabricLoaderObj, forge_install_obj::ForgeInstallObj,
        forge_launch_obj::ForgeLaunchObj, quilt_loader_obj::QuiltLoaderObj,
    },
};

/// 保存Fabric-Loader信息
///
/// - `obj`: Fabric 加载器数据
/// - `data`: json 文本
/// - `mc`: 游戏版本
/// - `version`: 加载器版本
///
/// # 返回值
///
/// 返回缓存的加载器信息
pub fn add_fabric(
    obj: FabricLoaderObj,
    data: &Vec<u8>,
    mc: &str,
    version: &str,
) -> Arc<FabricLoaderObj> {
    let file = FABRIC_DIR.get().unwrap().join(format!("{}.json", obj.id));
    path_helper::write_bytes(&file, &data).unwrap();

    let key = LoaderKey::new(mc, version);
    let mut list = FABRIC_LOADERS.write().unwrap();
    let info = Arc::new(obj);
    list.insert(key.clone(), info.clone());

    info
}

/// 添加Forge启动信息
///
/// - `obj`: 启动信息
/// - `data`: json 文本
/// - `mc`: 游戏版本
/// - `version`: 加载器版本
/// - `neo`: 是否为NeoForge
///
/// # 返回值
///
/// 返回缓存的启动信息
pub fn add_forge(
    obj: ForgeLaunchObj,
    data: &Vec<u8>,
    mc: &str,
    version: &str,
    neo: bool,
) -> Arc<ForgeLaunchObj> {
    let v222 = version_parse::is_game_version_1202(mc);
    let name = if neo && v222 {
        format!("{}-{}", names::NEOFORGE_KEY, version)
    } else {
        format!("{}-{}-{}", names::FORGE_KEY, mc, version)
    };
    let file = if neo {
        NEOFORGE_DIR.get().unwrap().join(format!("{}.json", name))
    } else {
        FORGE_DIR.get().unwrap().join(format!("{}.json", name))
    };
    path_helper::write_bytes(&file, &data).unwrap();

    let key = LoaderKey::new(mc, version);
    let mut list = if neo {
        NEOFORGE_LAUNCHS.write().unwrap()
    } else {
        FORGE_LAUNCHS.write().unwrap()
    };
    let info = Arc::new(obj);
    list.insert(key, info.clone());

    info
}

/// 添加Forge安装信息
///
/// - `obj`: 安装信息
/// - `data`: json 文本
/// - `mc`: 游戏版本
/// - `version`: 加载器版本
/// - `neo`: 是否为NeoForge
///
/// # 返回值
///
/// 返回缓存的安装信息
pub fn add_forge_install(
    obj: ForgeInstallObj,
    data: &Vec<u8>,
    mc: &str,
    version: &str,
    neo: bool,
) -> Arc<ForgeInstallObj> {
    let name = get_forge_json_name(mc, version, neo, true);
    let file = if neo {
        NEOFORGE_DIR.get().unwrap().join(format!("{}.json", name))
    } else {
        FORGE_DIR.get().unwrap().join(format!("{}.json", name))
    };
    path_helper::write_bytes(&file, &data).unwrap();

    let key = LoaderKey::new(mc, version);
    let mut list = if neo {
        NEOFORGE_INSTALLS.write().unwrap()
    } else {
        FORGE_INSTALLS.write().unwrap()
    };
    let info = Arc::new(obj);
    list.insert(key, info.clone());

    info
}

/// 添加Quilt信息
///
/// - `obj`: Quilt加载器数据
/// - `data`: json 文本
/// - `mc`: 游戏版本
/// - `version`: 加载器版本
///
/// # 返回值
///
/// 返回缓存的加载器信息
pub fn add_quilt(
    obj: QuiltLoaderObj,
    data: &Vec<u8>,
    mc: &str,
    version: &str,
) -> Arc<QuiltLoaderObj> {
    let file = QUILT_DIR.get().unwrap().join(format!("{}.json", obj.id));
    path_helper::write_bytes(&file, &data).unwrap();

    let key = LoaderKey::new(mc, version);
    let mut list = QUILT_LOADERS.write().unwrap();
    let info = Arc::new(obj);
    list.insert(key.clone(), info.clone());

    info
}

/// 添加自定义加载器信息
///
/// - `obj`: 自定义加载器
/// - `uuid`: 游戏实例标识
pub fn add_custom_loader(obj: CustomLoaderType, uuid: Uuid) {
    let mut list = CUSTOM_LOADERS.write().unwrap();
    list.insert(uuid, Arc::new(obj));
}

/// 获取json名字
///
/// - `mc`: 游戏版本
/// - `version`: 加载器版本
/// - `neo`: 是否为NeoForge
/// - `install`: 是否为安装信息
///
/// # 返回值
///
/// 返回 json 文件名
pub fn get_forge_json_name(mc: &str, version: &str, neo: bool, install: bool) -> String {
    if neo {
        let v222 = version_parse::is_game_version_1202(&mc);

        if install {
            if v222 {
                format!(
                    "{}-{}-{}{}",
                    names::NEOFORGE_KEY,
                    version,
                    names::FILE_INSTALL,
                    names::JSON_DOT_EXT
                )
            } else {
                format!(
                    "{}-{}-{}-{}{}",
                    names::FORGE_KEY,
                    mc,
                    version,
                    names::FILE_INSTALL,
                    names::JSON_DOT_EXT
                )
            }
        } else {
            if v222 {
                format!("{}-{}{}", names::NEOFORGE_KEY, version, names::JSON_DOT_EXT)
            } else {
                format!(
                    "{}-{}-{}{}",
                    names::FORGE_KEY,
                    mc,
                    version,
                    names::JSON_DOT_EXT
                )
            }
        }
    } else {
        if install {
            format!(
                "{}-{}-{}{}",
                names::FORGE_KEY,
                version,
                names::FILE_INSTALL,
                names::JSON_DOT_EXT
            )
        } else {
            format!("{}-{}{}", names::FORGE_KEY, version, names::JSON_DOT_EXT)
        }
    }
}

/// 获取NeoForge安装数据
///
/// - `mc`: 游戏版本
/// - `version`: 加载器版本
///
/// # 返回值
///
/// 返回安装数据；文件缺失或解析失败返回 `None`
pub fn get_neoforge_install_obj(mc: &str, version: &str) -> Option<Arc<ForgeInstallObj>> {
    let key = LoaderKey::new(mc, version);

    // 读锁先释放：读锁未放再取写锁会自死锁，std RwLock 不可升级
    let hit = NEOFORGE_INSTALLS.read().unwrap().get(&key).cloned();
    if let Some(item) = hit {
        return Some(item);
    }

    let local = NEOFORGE_DIR
        .get()
        .unwrap()
        .join(get_forge_json_name(mc, version, true, true));
    match serialize_tools::json_from_file::<ForgeInstallObj>(&local) {
        Ok(json) => {
            let temp = Arc::new(json);
            let mut list = NEOFORGE_INSTALLS.write().unwrap();
            Some(list.entry(key).or_insert_with(|| temp.clone()).clone())
        }
        Err(err) => {
            mml_log::error_type(serializer_err(err));

            None
        }
    }
}

/// 获取NeoForge启动数据
///
/// - `mc`: 游戏版本
/// - `version`: 加载器版本
///
/// # 返回值
///
/// 返回启动数据；文件缺失或解析失败返回 `None`
pub fn get_neoforge(mc: &str, version: &str) -> Option<Arc<ForgeLaunchObj>> {
    let key = LoaderKey::new(mc, version);

    // 读锁先释放：读锁未放再取写锁会自死锁，std RwLock 不可升级
    let hit = NEOFORGE_LAUNCHS.read().unwrap().get(&key).cloned();
    if let Some(item) = hit {
        return Some(item);
    }

    let local = NEOFORGE_DIR
        .get()
        .unwrap()
        .join(get_forge_json_name(mc, version, true, false));
    match serialize_tools::json_from_file::<ForgeLaunchObj>(&local) {
        Ok(json) => {
            let temp = Arc::new(json);
            let mut list = NEOFORGE_LAUNCHS.write().unwrap();
            Some(list.entry(key).or_insert_with(|| temp.clone()).clone())
        }
        Err(err) => {
            mml_log::error_type(serializer_err(err));

            None
        }
    }
}

/// 获取Forge安装数据
///
/// - `mc`: 游戏版本
/// - `version`: 加载器版本
///
/// # 返回值
///
/// 返回安装数据；文件缺失或解析失败返回 `None`
pub fn get_forge_install_obj(mc: &str, version: &str) -> Option<Arc<ForgeInstallObj>> {
    let key = LoaderKey::new(mc, version);

    // 读锁先释放：读锁未放再取写锁会自死锁，std RwLock 不可升级
    let hit = FORGE_INSTALLS.read().unwrap().get(&key).cloned();
    if let Some(item) = hit {
        return Some(item);
    }

    let local = FORGE_DIR
        .get()
        .unwrap()
        .join(get_forge_json_name(mc, version, false, true));
    match serialize_tools::json_from_file::<ForgeInstallObj>(&local) {
        Ok(json) => {
            let temp = Arc::new(json);
            let mut list = FORGE_INSTALLS.write().unwrap();
            Some(list.entry(key).or_insert_with(|| temp.clone()).clone())
        }
        Err(err) => {
            mml_log::error_type(serializer_err(err));

            None
        }
    }
}

/// 获取Forge启动数据
///
/// - `mc`: 游戏版本
/// - `version`: 加载器版本
///
/// # 返回值
///
/// 返回启动数据；文件缺失或解析失败返回 `None`
pub fn get_forge(mc: &str, version: &str) -> Option<Arc<ForgeLaunchObj>> {
    let key = LoaderKey::new(mc, version);

    // 读锁先释放：读锁未放再取写锁会自死锁，std RwLock 不可升级
    let hit = FORGE_LAUNCHS.read().unwrap().get(&key).cloned();
    if let Some(item) = hit {
        return Some(item);
    }

    let local = FORGE_DIR
        .get()
        .unwrap()
        .join(get_forge_json_name(mc, version, false, false));
    match serialize_tools::json_from_file::<ForgeLaunchObj>(&local) {
        Ok(json) => {
            let temp = Arc::new(json);
            let mut list = FORGE_LAUNCHS.write().unwrap();
            Some(list.entry(key).or_insert_with(|| temp.clone()).clone())
        }
        Err(err) => {
            mml_log::error_type(serializer_err(err));

            None
        }
    }
}

/// 获取Fabric加载器数据
///
/// - `mc`: 游戏版本
/// - `version`: 加载器版本
///
/// # 返回值
///
/// 返回加载器数据；文件缺失或解析失败返回 `None`
pub fn get_fabric(mc: &str, version: &str) -> Option<Arc<FabricLoaderObj>> {
    let key = LoaderKey::new(mc, version);
    // 读锁先释放：读锁未放再取写锁会自死锁，std RwLock 不可升级
    let hit = FABRIC_LOADERS.read().unwrap().get(&key).cloned();
    if let Some(data) = hit {
        return Some(data);
    }
    let local = FABRIC_DIR.get().unwrap().join(format!(
        "{}-{}-{}{}",
        names::FABRIC_LOADER_KEY,
        version,
        mc,
        names::JSON_DOT_EXT
    ));
    match serialize_tools::json_from_file::<FabricLoaderObj>(&local) {
        Ok(json) => {
            let temp = Arc::new(json);
            let mut list = FABRIC_LOADERS.write().unwrap();
            Some(list.entry(key).or_insert_with(|| temp.clone()).clone())
        }
        Err(err) => {
            mml_log::error_type(serializer_err(err));

            None
        }
    }
}

/// 获取Quilt加载器数据
///
/// - `mc`: 游戏版本
/// - `version`: 加载器版本
///
/// # 返回值
///
/// 返回加载器数据；文件缺失或解析失败返回 `None`
pub fn get_quilt(mc: &str, version: &str) -> Option<Arc<QuiltLoaderObj>> {
    let key = LoaderKey::new(mc, version);
    // 读锁先释放：读锁未放再取写锁会自死锁，std RwLock 不可升级
    let hit = QUILT_LOADERS.read().unwrap().get(&key).cloned();
    if let Some(data) = hit {
        return Some(data);
    }
    let local = QUILT_DIR.get().unwrap().join(format!(
        "{}-{}-{}{}",
        names::QUILT_LOADER_KEY,
        version,
        mc,
        names::JSON_DOT_EXT
    ));
    match serialize_tools::json_from_file::<QuiltLoaderObj>(&local) {
        Ok(json) => {
            let temp = Arc::new(json);
            let mut list = QUILT_LOADERS.write().unwrap();
            Some(list.entry(key).or_insert_with(|| temp.clone()).clone())
        }
        Err(err) => {
            mml_log::error_type(serializer_err(err));

            None
        }
    }
}
