//! 版本目录与版本数据管理：版本 json 与各加载器 json

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, OnceLock, RwLock},
};

use mml_base::{
    hash_helper::{self, HashType},
    serialize_tools, version_parse,
};
use mml_config::{config_obj::SourceLocal, config_save};
use mml_names::{
    i18_items::error_type::{CoreResult, DataNotFoundData, ErrorType},
    names, uuids,
};
use mml_net::{mojang_api, url_helper};
use mml_sys::path_helper;
use tokio::task;
use uuid::Uuid;

use crate::{
    launcher::{custom_loader_obj::CustomLoaderType, instance_setting_obj::InstanceSettingObj},
    loader::{
        LoaderKey, LoaderType,
        fabric_loader_obj::FabricLoaderObj,
        forge_install_obj::ForgeInstallObj,
        forge_launch_obj::ForgeLaunchObj,
        liteloader_meta_obj::{LiteloaderMetaObj, LiteloaderVersionObj},
        optifine_obj::OptifineObj,
        quilt_loader_obj::QuiltLoaderObj,
    },
    mojang::{
        game_arg_obj::GameArgObj,
        version_obj::{VersionObj, VersionsObj},
    },
};
use crate::error::{serializer_err};

/// 版本根目录
static BASE_DIR: OnceLock<PathBuf> = OnceLock::new();
/// Forge 目录
static FORGE_DIR: OnceLock<PathBuf> = OnceLock::new();
/// Fabric 目录
static FABRIC_DIR: OnceLock<PathBuf> = OnceLock::new();
/// Quilt 目录
static QUILT_DIR: OnceLock<PathBuf> = OnceLock::new();
/// NeoForge 目录
static NEOFORGE_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 高清修复版本信息文件路径
static OPTIFINE_FILE: OnceLock<PathBuf> = OnceLock::new();
/// LiteLoader 版本信息文件路径
static LITELOADER_FILE: OnceLock<PathBuf> = OnceLock::new();

/// Mojang 版本列表缓存
static VERSION: LazyLock<RwLock<Option<Arc<VersionObj>>>> = LazyLock::new(|| RwLock::new(None));

/// 高清修复版本信息缓存，版本号 → 信息
static OPTIFINE_LOADER: LazyLock<RwLock<HashMap<String, Arc<OptifineObj>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// 游戏版本参数缓存，版本号 → 启动参数
static GAME_ARGS: LazyLock<RwLock<HashMap<String, Arc<GameArgObj>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// Forge 安装信息缓存
static FORGE_INSTALLS: LazyLock<RwLock<HashMap<LoaderKey, Arc<ForgeInstallObj>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// NeoForge 安装信息缓存
static NEOFORGE_INSTALLS: LazyLock<RwLock<HashMap<LoaderKey, Arc<ForgeInstallObj>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// Forge 启动信息缓存
static FORGE_LAUNCHS: LazyLock<RwLock<HashMap<LoaderKey, Arc<ForgeLaunchObj>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// NeoForge 启动信息缓存
static NEOFORGE_LAUNCHS: LazyLock<RwLock<HashMap<LoaderKey, Arc<ForgeLaunchObj>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// Fabric 加载器信息缓存
static FABRIC_LOADERS: LazyLock<RwLock<HashMap<LoaderKey, Arc<FabricLoaderObj>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// Quilt 加载器信息缓存
static QUILT_LOADERS: LazyLock<RwLock<HashMap<LoaderKey, Arc<QuiltLoaderObj>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// 自定义加载器信息缓存，实例 → 加载器
static CUSTOM_LOADERS: LazyLock<RwLock<HashMap<Uuid, Arc<CustomLoaderType>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// LiteLoader 版本信息缓存，游戏版本 → 信息
static LITE_LOADER: LazyLock<RwLock<HashMap<String, Arc<LiteloaderVersionObj>>>> =
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

    OPTIFINE_FILE.set(dir.join(names::OPTIFINE_FILE)).unwrap();
    LITELOADER_FILE
        .set(dir.join(names::LITELOADER_FILE))
        .unwrap();

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
/// 返回版本根目录
pub fn get_version_dir() -> PathBuf {
    BASE_DIR.get().unwrap().clone()
}

/// 加载高清修复版本信息
///
/// 从文件读取并填充缓存；文件不存在时忽略，解析失败记录日志
fn load_optifine() {
    let local = OPTIFINE_FILE.get().unwrap();
    if !local.exists() || !local.is_file() {
        return;
    }

    let json = serialize_tools::json_from_file::<HashMap<String, OptifineObj>>(local);

    match json {
        Err(err) => {
            mml_log::error_type(err);
        }
        Ok(data) => {
            let mut list = OPTIFINE_LOADER.write().unwrap();
            list.clear();

            list.extend(data.into_iter().map(|(k, v)| (k, Arc::new(v))));
        }
    };
}

/// 保存高清修复版本信息
///
/// 将缓存写回文件；写盘失败仅记录错误，不影响调用方
fn save_optifine() {
    let file = OPTIFINE_FILE.get().unwrap();
    let list = OPTIFINE_LOADER.read().unwrap();
    let value = &*list;

    let ref_map: HashMap<&String, &OptifineObj> =
        value.iter().map(|(k, v)| (k, v.as_ref())).collect();

    config_save::save(uuids::OPTIFINE_UUID, &ref_map, file);
}

/// 加载liteloader版本信息
///
/// 从文件读取并填充缓存；文件不存在时忽略，解析失败记录日志
fn load_liteloader() {
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
fn save_liteloader() {
    let file = LITELOADER_FILE.get().unwrap();
    let list = LITE_LOADER.read().unwrap();
    let value = &*list;

    let ref_map: HashMap<&String, &LiteloaderVersionObj> =
        value.iter().map(|(k, v)| (k, v.as_ref())).collect();

    config_save::save(uuids::LITELOADER_UUID, &ref_map, file);
}

/// 从在线获取版本信息
///
/// 优先使用默认源，失败后回退官方源再取一次。
///
/// # 返回值
///
/// 成功返回 `Ok(())`；两次请求或解析失败返回对应错误
async fn get_version_from_online() -> CoreResult<()> {
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
async fn read_version() -> CoreResult<()> {
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
fn ensure_version_loaded() {
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

/// 添加高清修复信息
///
/// - `obj`: 高清修复信息
///
/// # 返回值
///
/// 返回缓存的高清修复信息
pub fn add_optifine(obj: OptifineObj) -> Arc<OptifineObj> {
    let info = Arc::new(obj);
    // 写锁必须**先释放**再调 `save_optifine`：后者内部要取 OPTIFINE_LOADER 的**读锁**，
    // 而 `std::sync::RwLock` 不可重入，同线程持写锁时再取读锁会永久阻塞 = 自死锁。
    {
        let mut list = OPTIFINE_LOADER.write().unwrap();
        list.insert(info.version.clone(), info.clone());
    }

    save_optifine();

    info
}

/// 保存liteloader信息
///
/// - `obj`: LiteLoader 版本元数据
pub fn add_liteloader(obj: LiteloaderMetaObj) {
    // 同 `add_optifine`：写锁先释放，再调内部取读锁的 `save_liteloader`，
    // 否则同线程自死锁，这是"加载器列表卡在 1/6"的直接原因。
    {
        let mut list = LITE_LOADER.write().unwrap();
        list.extend(obj.versions.into_iter().map(|(k, v)| (k, Arc::new(v))));
    }

    save_liteloader();
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
        // 在线也没有这个版本号
        None => Err(ErrorType::DataNotFound(DataNotFoundData::Version(
            mc.to_string(),
        ))),
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

/// 获取高清修复信息
///
/// - `version`: 版本号
///
/// # 返回值
///
/// 返回高清修复信息；不存在返回 `None`
pub fn get_optifine(version: &str) -> Option<Arc<OptifineObj>> {
    let list = OPTIFINE_LOADER.read().unwrap();
    Some(list.get(version)?.clone())
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

impl InstanceSettingObj {
    /// 获取游戏版本类型
    ///
    /// # 返回值
    ///
    /// 返回版本类型，取值 release / snapshot / old_beta 等；无信息返回空串
    pub fn get_version_type(&self) -> String {
        let temp = VERSION.read().unwrap();

        if temp.is_none() {
            Default::default()
        } else {
            let temp = temp.clone().unwrap();

            if let Some(data) = temp
                .versions
                .iter()
                .filter(|item| item.id.eq_ignore_ascii_case(&self.version))
                .next()
            {
                data.version_type.clone()
            } else {
                Default::default()
            }
        }
    }

    /// 获取Forge加载器信息
    ///
    /// # 返回值
    ///
    /// 返回启动数据；无加载器版本或加载失败返回 `None`
    pub fn get_forge(&self) -> Option<Arc<ForgeLaunchObj>> {
        match &self.loader_version {
            None => None,
            Some(data) => get_forge(&self.version, &data),
        }
    }

    /// 获取NeoForge加载器信息
    ///
    /// # 返回值
    ///
    /// 返回启动数据；无加载器版本或加载失败返回 `None`
    pub fn get_neoforge(&self) -> Option<Arc<ForgeLaunchObj>> {
        match &self.loader_version {
            None => None,
            Some(data) => get_neoforge(&self.version, &data),
        }
    }

    /// 获取Fabric加载器数据
    ///
    /// # 返回值
    ///
    /// 返回加载器数据；无加载器版本或加载失败返回 `None`
    pub fn get_fabric(&self) -> Option<Arc<FabricLoaderObj>> {
        match &self.loader_version {
            None => None,
            Some(data) => get_fabric(&self.version, &data),
        }
    }

    /// 获取Quilt加载器数据
    ///
    /// # 返回值
    ///
    /// 返回加载器数据；无加载器版本或加载失败返回 `None`
    pub fn get_quilt(&self) -> Option<Arc<QuiltLoaderObj>> {
        match &self.loader_version {
            None => None,
            Some(data) => get_quilt(&self.version, &data),
        }
    }

    /// 获取自定义加载器数据
    ///
    /// # 返回值
    ///
    /// 返回自定义加载器数据；未添加返回 `None`
    pub fn get_custom_loader(&self) -> Option<Arc<CustomLoaderType>> {
        let list = CUSTOM_LOADERS.read().unwrap();
        Some(list.get(&self.uuid)?.clone())
    }

    /// 获取高清修复信息
    ///
    /// # 返回值
    ///
    /// 返回高清修复信息；非高清修复实例或不存在返回 `None`
    pub fn get_optifine(&self) -> Option<Arc<OptifineObj>> {
        if self.loader != LoaderType::OptiFine || self.loader_version.is_none() {
            None
        } else {
            get_optifine(self.loader_version.as_deref().unwrap())
        }
    }

    /// 更新游戏版本json
    ///
    /// # 返回值
    ///
    /// 返回最新的启动参数；在线无此版本或下载失败返回对应错误
    pub async fn check_version_update(&self) -> CoreResult<Arc<GameArgObj>> {
        check_update(&self.version).await
    }
}
