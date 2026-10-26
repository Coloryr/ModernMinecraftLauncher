//! 云连接、跨电脑实例绑定与手柄配置由 Rust 持久化。

use mml_base::serialize_tools;
use mml_config::config_save;
use mml_gamepad::runtime::{Runtime, Status};
use mml_names::{i18_items::error_type::CoreResult, names, uuids};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex, OnceLock, RwLock},
};
use uuid::Uuid;

/// 独立于界面缓存的集成配置。
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Config {
    /// 云服务器连接参数。
    pub cloud: mml_cloud::client::Settings,
    /// 本地实例到云端实例 UUID 的绑定。
    pub cloud_instances: HashMap<Uuid, Uuid>,
    /// 手柄映射与目标实例。
    pub gamepad: mml_gamepad::Settings,
}

static FILE: OnceLock<PathBuf> = OnceLock::new();
static CONFIG: LazyLock<RwLock<Config>> = LazyLock::new(|| RwLock::new(Config::default()));
static GAMEPAD: Mutex<Option<Runtime>> = Mutex::new(None);

/// 记录配置位置。
///
/// - `path`: 启动器运行目录
pub fn init(path: &Path) {
    FILE.get_or_init(|| path.join(names::INTEGRATIONS_FILE));
}

/// 在日志系统就绪后加载配置。
///
/// # 返回值
///
/// 配置损坏时返回解析错误，不覆盖原文件。
pub fn load() -> CoreResult<()> {
    let path = FILE.get().unwrap();
    if path.exists() {
        let mut config: Config = serialize_tools::json_from_file(path)?;
        if !config.gamepad.valid() {
            config.gamepad = Default::default();
        }
        *CONFIG.write().unwrap() = config;
    }
    Ok(())
}

/// 读取配置快照。
///
/// # 返回值
///
/// 调用方可独立修改的副本。
pub fn get() -> Config {
    CONFIG.read().unwrap().clone()
}

/// 原子修改内存配置并排队落盘。
///
/// - `update`: 只修改本次负责的配置部分
pub fn update(update: impl FnOnce(&mut Config)) {
    let mut config = CONFIG.write().unwrap();
    update(&mut config);
    config_save::save_compact(uuids::INTEGRATIONS_UUID, &*config, FILE.get().unwrap());
}

/// 启动设备采样线程；启动失败会在状态接口显示。
pub fn start_gamepad() {
    *GAMEPAD.lock().unwrap() = Runtime::start(get().gamepad).ok();
}

/// 更新映射并保存，下个采样周期生效。
///
/// - `settings`: 已验证的手柄设置
pub fn configure_gamepad(settings: mml_gamepad::Settings) {
    if let Some(runtime) = GAMEPAD.lock().unwrap().as_ref() {
        runtime.configure(settings.clone());
    }
    update(|config| config.gamepad = settings);
}

/// 获取设备采样状态。
///
/// # 返回值
///
/// 线程未启动时返回驱动错误。
pub fn gamepad_status() -> Status {
    GAMEPAD
        .lock()
        .unwrap()
        .as_ref()
        .map(Runtime::status)
        .unwrap_or_else(|| Status {
            error: Some("err.gamepadDriver".into()),
            ..Default::default()
        })
}

/// 真正退出应用前释放游戏按键并结束采样线程。
pub fn stop_gamepad() {
    GAMEPAD.lock().unwrap().take();
}
