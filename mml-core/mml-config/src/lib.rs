//! 启动器配置管理模块
//!
//! 本模块负责启动器的全局配置管理，包括：
//!
//! # 核心功能
//!
//! - **配置文件读写** — 以 JSON 格式存储在程序目录的 `config.json` 中
//! - **线程安全访问** — 使用 `RwLock` 保护全局配置，支持多线程并发读取
//! - **延迟保存** — 通过 [`config_save`] 模块实现后台异步保存，避免频繁 IO
//! - **版本迁移** — 启动时自动检测配置文件版本，版本不匹配时更新
//!
//! # 使用方式
//!
//! ```ignore
//! // 初始化，程序启动时调用一次
//! mml_config::init(&run_dir);
//!
//! // 读取配置
//! let config = mml_config::read_config();
//!
//! // 修改配置
//! let mut config = mml_config::write_config();
//! config.http.source = SourceLocal::Bmclapi;
//! drop(config); // 释放锁
//!
//! // 触发保存
//! mml_config::save();
//! ```

pub mod config_obj;
pub mod config_save;

use std::{
    path::{Path, PathBuf},
    sync::{OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use mml_base::serialize_tools;
use mml_log;
use mml_names::{i18_items::error_type::CoreResult, names, uuids};

use crate::config_obj::ConfigObj;

/// 全局配置对象，惰性初始化并用读写锁保护
static CONFIG: OnceLock<RwLock<ConfigObj>> = OnceLock::new();
/// 配置文件路径，惰性初始化
static FILE: OnceLock<PathBuf> = OnceLock::new();

/// 初始化配置系统
///
/// 设置配置文件路径并从磁盘加载配置，文件不存在则创建默认配置；应在程序启动时调用一次。
///
/// - `dir`: 程序运行目录，配置文件保存在 `{dir}/config.json`
///
/// # 返回值
///
/// 加载成功返回 `Ok(())`；创建默认配置失败只记日志不返回错误，JSON 解析失败时返回相应错误
pub fn init<P: AsRef<Path>>(dir: P) -> CoreResult<()> {
    FILE.get_or_init(|| dir.as_ref().join(names::CONFIG_FILE));
    load(FILE.get().unwrap())
}

/// 获取配置文件的只读锁
///
/// 返回一个 RAII 守卫，持有期间阻塞写操作。
/// 使用完毕后应尽快 drop，以免阻塞保存。
pub fn read_config() -> RwLockReadGuard<'static, ConfigObj> {
    CONFIG.get().unwrap().read().unwrap()
}

/// 获取配置文件的读写锁
///
/// 返回一个 RAII 守卫，持有期间阻塞所有读写操作。
/// 修改配置后，需调用 [`save()`] 将更改持久化。
pub fn write_config() -> RwLockWriteGuard<'static, ConfigObj> {
    CONFIG.get().unwrap().write().unwrap()
}

/// 立即同步保存配置到磁盘
///
/// 直接调用序列化函数写入文件，不经过后台保存队列。
/// 适用于需要立即持久化的场景，如程序退出前。
pub fn save_now() {
    let file = FILE.get().unwrap();
    if let Err(err) = serialize_tools::json_to_file(&*CONFIG.get().unwrap().read().unwrap(), file) {
        mml_log::error_type(err);
    }
}

/// 将配置加入后台保存队列
///
/// 通过 [`config_save`] 模块异步保存，合并短时间内的多次修改，
/// 减少磁盘 IO 次数。此方法是修改配置后的推荐保存方式。
pub fn save() {
    let config = &*CONFIG.get().unwrap().read().unwrap();
    config_save::save(uuids::CONFIG_UUID, config, FILE.get().unwrap());
}

/// 从文件加载配置
///
/// 如果文件不存在则创建默认配置文件。
/// 如果配置文件版本与当前启动器版本不匹配，则自动更新版本号。
///
/// - `file`: 配置文件路径
///
/// # 返回值
///
/// 成功加载或创建默认配置返回 `Ok(())`；创建失败只记日志，
/// JSON 解析失败时返回相应错误
pub fn load<P: AsRef<Path>>(file: P) -> CoreResult<()> {
    let config = CONFIG.get_or_init(|| RwLock::new(Default::default()));

    if !file.as_ref().exists() {
        save_now();
        return Ok(());
    }

    let json = serialize_tools::json_from_file::<ConfigObj>(&file)?;

    let mut config_obj = json;
    let version = mml_names::VERSION.clone();
    if config_obj.version != version {
        config_obj.version = version;

        save_now();
    }
    let mut guard = config.write().unwrap();
    *guard = config_obj;

    Ok(())
}
