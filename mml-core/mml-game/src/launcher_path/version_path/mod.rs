//! 版本目录与版本数据管理：版本 json 与各加载器 json

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, OnceLock, RwLock},
};

use mml_names::{i18_items::error_type::CoreResult, names};
use mml_sys::path_helper;
use uuid::Uuid;

use crate::{
    launcher::custom_loader_obj::CustomLoaderType,
    loader::{
        LoaderKey, fabric_loader_obj::FabricLoaderObj, forge_install_obj::ForgeInstallObj,
        forge_launch_obj::ForgeLaunchObj, liteloader_meta_obj::LiteloaderVersionObj,
        optifine_obj::OptifineObj, quilt_loader_obj::QuiltLoaderObj,
    },
    mojang::{game_arg_obj::GameArgObj, version_obj::VersionObj},
};

mod instance;
mod liteloader;
mod loader;
mod optifine;
mod version;

pub(super) static BASE_DIR: OnceLock<PathBuf> = OnceLock::new();
/// Forge 目录
pub(super) static FORGE_DIR: OnceLock<PathBuf> = OnceLock::new();
/// Fabric 目录
pub(super) static FABRIC_DIR: OnceLock<PathBuf> = OnceLock::new();
/// Quilt 目录
pub(super) static QUILT_DIR: OnceLock<PathBuf> = OnceLock::new();
/// NeoForge 目录
pub(super) static NEOFORGE_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 高清修复版本信息文件路径
pub(super) static OPTIFINE_FILE: OnceLock<PathBuf> = OnceLock::new();
/// LiteLoader 版本信息文件路径
pub(super) static LITELOADER_FILE: OnceLock<PathBuf> = OnceLock::new();

/// Mojang 版本列表缓存
pub(super) static VERSION: LazyLock<RwLock<Option<Arc<VersionObj>>>> =
    LazyLock::new(|| RwLock::new(None));

/// 高清修复版本信息缓存，版本号 → 信息
pub(super) static OPTIFINE_LOADER: LazyLock<RwLock<HashMap<String, Arc<OptifineObj>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// 游戏版本参数缓存，版本号 → 启动参数
pub(super) static GAME_ARGS: LazyLock<RwLock<HashMap<String, Arc<GameArgObj>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// Forge 安装信息缓存
pub(super) static FORGE_INSTALLS: LazyLock<RwLock<HashMap<LoaderKey, Arc<ForgeInstallObj>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// NeoForge 安装信息缓存
pub(super) static NEOFORGE_INSTALLS: LazyLock<RwLock<HashMap<LoaderKey, Arc<ForgeInstallObj>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// Forge 启动信息缓存
pub(super) static FORGE_LAUNCHS: LazyLock<RwLock<HashMap<LoaderKey, Arc<ForgeLaunchObj>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// NeoForge 启动信息缓存
pub(super) static NEOFORGE_LAUNCHS: LazyLock<RwLock<HashMap<LoaderKey, Arc<ForgeLaunchObj>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// Fabric 加载器信息缓存
pub(super) static FABRIC_LOADERS: LazyLock<RwLock<HashMap<LoaderKey, Arc<FabricLoaderObj>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// Quilt 加载器信息缓存
pub(super) static QUILT_LOADERS: LazyLock<RwLock<HashMap<LoaderKey, Arc<QuiltLoaderObj>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// 自定义加载器信息缓存，实例 → 加载器
pub(super) static CUSTOM_LOADERS: LazyLock<RwLock<HashMap<Uuid, Arc<CustomLoaderType>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// LiteLoader 版本信息缓存，游戏版本 → 信息
pub(super) static LITE_LOADER: LazyLock<RwLock<HashMap<String, Arc<LiteloaderVersionObj>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 初始化版本路径
///
/// - `dir`: 运行路径
///
/// # 返回值
///
/// 成功返回 `Ok(())`；创建目录失败返回对应错误
pub(crate) fn init<P: AsRef<Path>>(dir: P) -> CoreResult<()> {
    let dir = BASE_DIR.get_or_init(|| dir.as_ref().join(names::VERSION_DIR));

    // 用 get_or_init 而不是 set().unwrap()：重复 init 不该 panic（测试里多次起步很常见）
    OPTIFINE_FILE.get_or_init(|| dir.join(names::OPTIFINE_FILE));
    LITELOADER_FILE.get_or_init(|| dir.join(names::LITELOADER_FILE));

    let dir = dir.as_path();
    if !dir.is_dir() {
        path_helper::create_dir_all(dir)?;
    }

    let forge = FORGE_DIR.get_or_init(|| dir.join(names::FORGE_KEY));
    if !forge.is_dir() {
        path_helper::create_dir_all(forge)?;
    }

    let fabric = FABRIC_DIR.get_or_init(|| dir.join(names::FABRIC_KEY));
    if !fabric.is_dir() {
        path_helper::create_dir_all(fabric)?;
    }

    let quilt = QUILT_DIR.get_or_init(|| dir.join(names::QUILT_KEY));
    if !quilt.is_dir() {
        path_helper::create_dir_all(quilt)?;
    }

    let neoforge = NEOFORGE_DIR.get_or_init(|| dir.join(names::NEOFORGED_KEY));
    if !neoforge.is_dir() {
        path_helper::create_dir_all(neoforge)?;
    }

    Ok(())
}

/// 开始读取文件加载
pub(crate) fn load() {
    load_optifine();
    load_liteloader();
}

/// 获取目录
///
/// # 返回值
///
use self::liteloader::load_liteloader;
pub use self::liteloader::{add_liteloader, get_liteloader};
pub use self::loader::{
    add_custom_loader, add_fabric, add_forge, add_forge_install, add_quilt, get_fabric, get_forge,
    get_forge_install_obj, get_forge_json_name, get_neoforge, get_neoforge_install_obj, get_quilt,
};
use self::optifine::load_optifine;
pub use self::optifine::{add_optifine, get_optifine};
pub use self::version::{
    add_game, check_update, get_latest_version, get_version, get_version_dir,
    get_version_obj_online, have_version, is_have_version_info,
};
