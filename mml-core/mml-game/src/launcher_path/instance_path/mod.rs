//! 实例目录与实例内文件路径

pub(crate) use std::{
    collections::HashMap,
    fs::{self},
    path::{Path, PathBuf},
    sync::OnceLock,
};

pub(crate) use mml_base::serialize_tools;
pub(crate) use mml_config::config_save;
pub(crate) use mml_names::{
    i18_items::{
        error_type::{CoreResult, ErrorType, FileSystemErrorData},
        info_type::InfoType,
    },
    names, uuids,
};
pub(crate) use mml_sys::path_helper;

pub(crate) use crate::launcher::{
    custom_game_arg_obj::CustomGameArgObj,
    file_online_info_obj::OnlineInfoObj,
    instance_setting_obj::CustomLoaderObj,
    instance_setting_obj::InstanceSettingObj,
    mod_update_obj::{ModUpdateList, ModUpdateRecordObj},
};
pub(crate) use crate::other_launcher::{is_mmc_version, mmc_obj::MMCObj};

/// 在线文件信息表，键为 mod_id
pub type OnlineInfoList = HashMap<String, OnlineInfoObj>;
/// 自定义启动配置表，键为文件名
pub type CustomGameArgList = HashMap<String, CustomGameArgObj>;

/// 实例根目录
static BASE_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 初始化实例路径
///
/// - `dir`: 运行路径
///
/// # 返回值
///
/// 创建目录失败时返回对应错误
pub(crate) fn init<P: AsRef<Path>>(dir: P) -> CoreResult<()> {
    let dir = BASE_DIR.get_or_init(|| dir.as_ref().join(names::INSTANCE_DIR));

    if !dir.exists() {
        path_helper::create_dir_all(&dir)?;
    }

    Ok(())
}

/// 获取实例目录
///
/// # 返回值
///
/// 返回实例根目录；未初始化会 panic
pub fn get_instance_dir() -> PathBuf {
    BASE_DIR.get().unwrap().clone()
}

/// 读取所有实例
///
/// # 返回值
///
/// 返回实例列表；读取目录失败返回对应错误
pub(crate) fn load_instance_dir() -> CoreResult<Vec<InstanceSettingObj>> {
    let dir = BASE_DIR.get().unwrap();
    let dirs = fs::read_dir(dir).map_err(|err| {
        ErrorType::FileSystemError(FileSystemErrorData {
            path: dir.clone(),
            error: err.to_string(),
        })
    })?;

    let mut list = Vec::new();

    for item in dirs {
        if let Ok(dir) = item
            // 没有 game.json 时再试 MMC：Labymod 这类工具会直接按 MMC 格式在实例目录下建目录
            && let Some(instance) = load_instance(dir.path()).or_else(|| adopt_mmc(&dir.path()))
        {
            list.push(instance);
        }
    }

    Ok(list)
}

/// 把标准的 MMC 实例目录接管成 mml 实例
///
/// Labymod 之类的工具会直接按 MMC 格式在 `instances/` 下建目录（`mmc-pack.json` + `instance.cfg`），
/// 这种目录没有 `game.json`，正常加载会整个跳过 —— 用户看到的现象就是"实例没被识别"。
/// 这里按 MMC 元数据补出实例配置并落盘：**目录原地保留、文件不动**，只多一个 `game.json`。
///
/// - `dir`: 实例目录
///
/// # 返回值

///
/// 补好配置的实例；不是 MMC 实例或元数据读不出来时返回 `None`
/// 目录里有没有 MMC 补丁，即 `patches/*.json`
///
/// - `dir`: 实例目录
///
/// # 返回值
///
/// 至少有一个补丁文件返回 `true`
fn has_patches(dir: &Path) -> bool {
    fs::read_dir(dir.join(names::JSON_DIR))
        .map(|items| {
            items.flatten().any(|item| {
                let path = item.path();
                path.is_file() && path.extension().is_some_and(|ext| ext == names::JSON_EXT)
            })
        })
        .unwrap_or(false)
}

/// 把 MMC 实例目录读成 M²L 的实例配置
///
/// - `dir`: 实例目录
///
/// # 返回值
///
/// 不是 MMC 版本目录或配置读不出来时返回 `None`
fn adopt_mmc(dir: &Path) -> Option<InstanceSettingObj> {
    if !is_mmc_version(dir) {
        return None;
    }

    let mmc = serialize_tools::json_from_file::<MMCObj>(dir.join(names::MMCJSON_FILE)).ok()?;
    let cfg = crate::game_options::read_options_from_file(dir.join(names::MMCCFG_FILE), Some('='))
        .ok()?;

    let mut instance = mmc.to_instance(cfg);
    let name = dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    instance.dir = name.clone();
    if instance.name.is_empty() {
        instance.name = name;
    }
    // 目录里有 patches/*.json 就按 MMC 模式启动：补丁是 MMC 改启动配置的方式
    // （Labymod 就靠它注入 launchwrapper + LabyMod-4.jar）。
    // to_instance 只在碰到未知组件时才打开这个开关，而 Labymod 的 components 只有 net.minecraft，
    // 不补这一步的话 patches 会被忽略、实例退化成原版
    if has_patches(dir) {
        instance.custom_loader = Some(CustomLoaderObj {
            custom_json: true,
            ..Default::default()
        });
    }

    instance.save();
    mml_log::info_type(InfoType::AdoptMmcInstance {
        name: instance.name.clone(),
    });
    Some(instance)
}

/// 从文件夹路径加载实例
///
/// - `dir`: 实例目录
///
/// # 返回值
///
/// 目录或配置文件不存在时返回 `None`
pub(crate) fn load_instance<P: AsRef<Path>>(dir: P) -> Option<InstanceSettingObj> {
    let file = dir.as_ref();
    if !file.is_dir() {
        return None;
    }

    let config = file.join(names::GAME_FILE);
    if !config.exists() || !config.is_file() {
        return None;
    }

    if let Ok(mut obj) = serialize_tools::json_from_file::<InstanceSettingObj>(&config) {
        let path = file.file_name().unwrap_or_default();
        let path = path.to_string_lossy();
        if !path.eq(&obj.dir) {
            obj.dir = path.to_string();
            obj.save();
        }

        Some(obj)
    } else {
        None
    }
}

mod load;
mod settings;
