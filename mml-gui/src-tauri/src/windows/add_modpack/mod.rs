//! 下载整合包窗口：在线整合包的搜索 / 版本列表 / 安装
//!
//! 安装走 `mml_game::add_game`，CurseForge / Modrinth 各自一套流程，
//! 进度与重名确认复用添加实例窗口的回调 `super::add`，因为压缩包 / 网址
//! 导入走的是同一套安装流程。
//!
//! 安装是多任务的：任务登记在全局表 [`DOWNLOAD_NOW`]，键为 pid+fid，同键
//! 不允许重复安装；与窗口生命周期解耦——窗口关闭任务照跑，进度改由主窗口
//! 显示；任务进度通过 `add-modpack-status` 事件广播，携带窗口开关状态。

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use mml_net::curseforge_api::file_obj::CurseForgeFileDataObj;
use mml_net::curseforge_api::list_obj::CurseForgeListDataObj;
use mml_net::modrinth_api::search_obj::HitObj;
use mml_net::modrinth_api::version_obj::ModrinthVersionObj;
use tokio::sync::RwLock as AsyncRwLock;
use tokio_util::sync::CancellationToken;

// 带命令的子模块必须 `pub(crate)`：生成的 `tauri_commands!` 从 crate 根引用它们
pub(crate) mod install;
pub(crate) mod list;

/// 下载整合包窗口上次选的下载源
///
/// # 返回值
///
/// 下载源线串；没选过时是空串，由界面落回列表第一个
#[gui_macros::command]
pub fn add_modpack_last_source() -> String {
    crate::gui_config::get().resource_window.add_modpack_source
}

/// 记住这次选的下载源，落 `gui_config.json`
///
/// - `source`: 下载源线串，来自列表接口
#[gui_macros::command]
pub fn add_modpack_set_last_source(source: String) {
    let mut config = crate::gui_config::get();
    if config.resource_window.add_modpack_source == source {
        return;
    }
    config.resource_window.add_modpack_source = source;
    crate::gui_config::set(config);
}
/// CurseForge 项目列表缓存，pid → 项目数据，列表页请求时填充
pub(super) static CURSEFOGRE_INFO: LazyLock<AsyncRwLock<HashMap<String, CurseForgeListDataObj>>> =
    LazyLock::new(|| AsyncRwLock::new(HashMap::new()));
/// Modrinth 项目列表缓存，pid → 搜索命中项，列表页请求时填充
pub(super) static MODRINTH_INFO: LazyLock<AsyncRwLock<HashMap<String, HitObj>>> =
    LazyLock::new(|| AsyncRwLock::new(HashMap::new()));

/// CurseForge 文件缓存，pid → fid → 文件数据，文件列表页请求时填充
pub(super) static CURSEFOGRE_FILE: LazyLock<
    AsyncRwLock<HashMap<String, HashMap<String, CurseForgeFileDataObj>>>,
> = LazyLock::new(|| AsyncRwLock::new(HashMap::new()));
/// Modrinth 版本缓存，pid → 版本号 → 版本数据，文件列表页请求时填充
pub(super) static MODRINTH_FILE: LazyLock<
    AsyncRwLock<HashMap<String, HashMap<String, ModrinthVersionObj>>>,
> = LazyLock::new(|| AsyncRwLock::new(HashMap::new()));

/// 各窗口正在进行的列表搜索，键为窗口 label，窗口关闭时取消
static SEARCH_CANCEL: LazyLock<Mutex<HashMap<String, CancellationToken>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 登记本窗口这一发搜索并返回取消令牌，顺手取消上一发
///
/// - `label`: 窗口标签，搜索按它分组
///
/// # 返回值
///
/// 本发搜索的取消令牌，调用方用它感知"被新一发顶掉"
pub(super) fn register_search(label: &str) -> CancellationToken {
    let mut lock = SEARCH_CANCEL.lock().unwrap();

    if let Some(old) = lock.get(label) {
        old.cancel();
    }

    let token = CancellationToken::new();
    lock.insert(label.to_string(), token.clone());

    token
}

/// 取消该窗口正在进行的列表搜索，窗口关闭时调用
///
/// - `label`: 窗口标签
pub fn cancel_search(label: &str) {
    if let Some(token) = SEARCH_CANCEL.lock().unwrap().remove(label) {
        token.cancel();
    }
}
