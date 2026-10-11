//! M²L 启动器内核总入口
//!
//! 聚合 mml-core 下各子 crate：按顺序完成 init 初始化、load 配置加载与 stop 停止，
//! 并提供核心停止事件的注册与触发。
//! 各子 crate 的职责见其自身的 `//!` 头部说明。

/// 核心初始化参数
#[derive(Debug)]
pub struct CoreInitObj {
    /// 启动器运行根目录，须已存在，否则 [`init`] 报错
    pub path: PathBuf,
    /// 微软登录 OAuth 密钥，空串表示未配置
    pub oauth_key: String,
    /// CurseForge 平台密钥，空串表示未配置
    pub curseforge_key: String,
}

use std::{
    path::PathBuf,
    sync::{LazyLock, RwLock},
};

use mml_auth::{auths, oauth};
use mml_base::events::EventHandler;
use mml_config::config_save;
use mml_log;
use mml_names::i18_items::{
    error_type::{CoreResult, ErrorType::Panic},
    info_type::InfoType,
    panic_type::PanicType,
};
use mml_net::curseforge_api;

/// 是否为第一次启动
pub static NEW_START: RwLock<bool> = RwLock::new(false);

/// 核心是否已通过 load 完成加载
static STATE: RwLock<bool> = RwLock::new(false);

/// 核心停止事件，stop 时触发，各子 crate 在此挂清理回调
static CORE_STOP_EVENT: LazyLock<EventHandler> = LazyLock::new(|| EventHandler::new());

/// 注册核心停止事件回调
///
/// - `handler`: 停止时执行的回调
///
/// # 返回值
///
/// 返回回调 ID，供 [`remove_core_stop`] 使用
pub fn add_core_stop<F>(handler: F) -> u64
where
    F: Fn() + Send + Sync + 'static,
{
    CORE_STOP_EVENT.add_handler(Box::new(handler))
}

/// 移除核心停止事件回调
///
/// - `id`: [`add_core_stop`] 返回的回调 ID
pub fn remove_core_stop(id: u64) {
    CORE_STOP_EVENT.remove_handle(id);
}

/// 触发核心停止事件，执行全部已注册回调
pub fn invoke_core_stop() {
    CORE_STOP_EVENT.emit();
}

/// 核心是否已加载完成
///
/// # 返回值
///
/// load 完成后为 true，stop 后回到 false
pub fn get_state() -> bool {
    return *STATE.read().unwrap();
}

/// 初始化核心，只设置允许目录，不加载内容
///
/// - `arg`: 核心初始化参数
///
/// # 返回值
///
/// 运行路径为空或不存在、子模块初始化失败时返回相应错误，成功返回 `Ok(())`
pub fn init(arg: CoreInitObj) -> CoreResult<()> {
    if arg.path.as_os_str().is_empty() {
        return Err(Panic(PanicType::CoreArgLocalEmpty));
    }
    if !arg.path.exists() {
        return Err(Panic(PanicType::CoreArgLocalError));
    }

    mml_base::init(arg.path.to_path_buf());

    oauth::set_key(&arg.oauth_key);
    curseforge_api::set_key(&arg.curseforge_key);

    let path = mml_base::get_base_dir();
    mml_names::init(&path)?;
    mml_log::start(&path)?;
    mml_log::info_type(InfoType::CoreStart);
    mml_tex_draw::init(&path)?;
    mml_config::init(&path)?;
    mml_game::init(&path)?;
    mml_jvms::init(&path)?;
    mml_downloader::init(&path)?;

    config_save::start();

    Ok(())
}

/// 加载配置
///
/// # 返回值
///
/// 子模块加载失败时返回相应错误，成功返回 `Ok(())`
pub fn load() -> CoreResult<()> {
    mml_net::init();
    auths::init();
    mml_jvms::load();

    mml_game::load()?;
    mml_tex_draw::load()?;

    CORE_STOP_EVENT.add_handler(config_save::stop);
    CORE_STOP_EVENT.add_handler(mml_downloader::stop);
    // 退出时中断贴图渲染：渲染循环是协作式的，不打断会一直跑到本轮结束
    CORE_STOP_EVENT.add_handler(mml_tex_draw::cancel);
    CORE_STOP_EVENT.add_handler(mml_log::stop);

    *STATE.write().unwrap() = true;

    Ok(())
}

/// 停止核心，触发停止事件，各子 crate 的清理回调随之执行
pub fn stop() {
    mml_log::info(String::from("M²L stop"));

    invoke_core_stop();

    *STATE.write().unwrap() = false;
}
