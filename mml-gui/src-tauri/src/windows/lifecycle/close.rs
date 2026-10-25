//! 关窗与开窗
//!
//! 一族实例窗口随实例一起关；关窗前先把模型里的任务取消。


use tauri::{AppHandle, Error::WindowNotFound, Manager, WebviewWindow};

use uuid::Uuid;

use crate::windows::geometry::{save_window_state};
use crate::windows::registry::{MAIN_WINDOW_UUID, WINDOWS_INFO};
use super::models::{OPEN_WINDOWS};
use super::create::{create_window};


/// 关掉挂在某个实例名下的所有窗口
///
/// 实例被删掉之后调用：那些窗口的模型还指着已经不存在的实例，留着只会点出错误。
/// 资源管理 / 日志 / 添加资源 / 配置文件四族一起关，走 [`close_window`] 的正常流程
/// （先存几何、各自的关闭保护与取消照旧）。
///
/// - `app`: 应用句柄
/// - `instance`: 被删掉的实例 uuid
pub(crate) fn close_instance_windows(app: &AppHandle, instance: &Uuid) {
    for family in crate::windows::registry::families() {
        let id = crate::windows::registry::family_window_id(family, instance);
        // 没开的窗口返回"找不到"，逐个关、失败忽略
        let _ = close_window(app, &id.uuid, &id.label);
    }
}

/// 关闭指定窗口：先保存几何，再真正关闭窗口
///
/// - `app`: 应用句柄，按标签补查窗口用
/// - `uuid`: 窗口 uuid
/// - `label`: 与 `uuid` 对应的窗口标签；静态窗口取自注册表，日志窗口族由实例派生
///
/// # 返回值
///
/// 关闭请求已发出返回 `Ok(())`；窗口已不存在返回错误文案
pub(crate) fn close_window(app: &AppHandle, uuid: &Uuid, label: &str) -> Result<(), String> {
    // 句柄优先取句柄表，缺失时按标签现查，覆盖外部创建的窗口。这里**只克隆、不提前摘表**：
    // `close()` 会先发 `CloseRequested`，关闭保护完全可能拦下它，摘了表就与真实窗口脱节
    let window = OPEN_WINDOWS
        .read()
        .unwrap()
        .get(uuid)
        .cloned()
        .or_else(|| app.get_webview_window(label));
    let Some(window) = window else {
        return Err(WindowNotFound.to_string());
    };

    // 句柄可能已失效，窗口被外部关闭：标签已不存在则清理句柄表并视为已关闭
    if app.get_webview_window(label).is_none() {
        OPEN_WINDOWS.write().unwrap().remove(uuid);
        return Ok(());
    }

    // 最大化 / 全屏时只记标志，几何保留最大化之前的值，见 save_window_state
    let maximized =
        window.is_maximized().unwrap_or(false) || window.is_fullscreen().unwrap_or(false);
    save_window_state(uuid, &window, maximized)?;
    window.close().map_err(|err| err.to_string())?;
    Ok(())
}

/// 关闭注册表里静态登记的窗口
///
/// - `app`: 应用句柄，按标签补查窗口用
/// - `uuid`: 窗口 uuid
///
/// # 返回值
///
/// 关闭请求已发出返回 `Ok(())`；uuid 未注册或窗口已不存在返回错误文案
pub(crate) fn close_window_from_uuid(app: &AppHandle, uuid: &Uuid) -> Result<(), String> {
    let Some(entry) = WINDOWS_INFO.get(uuid) else {
        return Err(WindowNotFound.to_string());
    };
    close_window(app, uuid, entry.label)
}

/// 打开或聚焦一个窗口
///
/// - `app`: 应用句柄，建窗用
/// - `uuid`: 窗口 uuid
/// - `label`: 与 `uuid` 对应的窗口标签
/// - `url_path`: 传给 [`create_window`] 的 webview 路径
/// - `anchor`: 发起这次打开的那个窗口，见 [`create_window`]
///
/// # 返回值
///
/// 窗口已打开或已聚焦返回 `Ok(())`；建窗失败返回错误文案
pub(crate) fn open_window(
    app: &AppHandle,
    uuid: &Uuid,
    label: &str,
    url_path: &str,
    anchor: Option<&WebviewWindow>,
) -> Result<(), String> {
    create_window(app, label, uuid, url_path, anchor)?;
    Ok(())
}

/// 打开或聚焦注册表里静态登记的窗口
///
/// - `app`: 应用句柄，建窗用
/// - `uuid`: 窗口 uuid
/// - `url_path`: 传给 [`create_window`] 的 webview 路径
/// - `anchor`: 发起这次打开的那个窗口，见 [`create_window`]
///
/// # 返回值
///
/// 窗口已打开或已聚焦返回 `Ok(())`；uuid 未注册或建窗失败返回错误文案
pub(crate) fn open_window_from_uuid(
    app: &AppHandle,
    uuid: &Uuid,
    url_path: &str,
    anchor: Option<&WebviewWindow>,
) -> Result<(), String> {
    let Some(entry) = WINDOWS_INFO.get(uuid) else {
        return Err(WindowNotFound.to_string());
    };
    open_window(app, uuid, entry.label, url_path, anchor)
}

/// 创建 / 聚焦主窗口，setup 阶段调用
///
/// 主窗口**第一次显示时不抢前台**：程序由系统、脚本或开发期 watcher 拉起来时，
/// 把用户正在用的窗口顶掉会打断操作，见 `quiet_main_window_startup`。
///
/// - `app`: 应用句柄，建窗用
///
/// # 返回值
///
/// 主窗口已打开返回 `Ok(())`，建窗失败返回错误文案
pub fn show_main_window(app: &AppHandle) -> Result<(), String> {
    crate::windows::quiet_main_window_startup();
    // 主窗口是顶层：没有历史几何时走"屏幕中心"，见 initial_placement
    open_window_from_uuid(app, &MAIN_WINDOW_UUID, "index.html", None)
}
