//! 系统通知：启动器不在前台时，把用户看不到的结果推给系统
//!
//! 三个平台都由 tauri-plugin-notification 落到各自的原生形态：Windows 是操作中心的吐司，
//! macOS 是右上角的气泡，Linux 走桌面环境的通知服务。
//!
//! 判据是**启动器有没有窗口在前台**：用户正看着界面时再弹一条系统提示是多余的，
//! 界面上本来就有进度与结果。

use tauri::AppHandle;
use tauri::Manager;
use tauri_plugin_notification::NotificationExt;

use crate::log_text::{LogText, NotifyText};

/// 启动器是否有窗口在前台
///
/// 多窗口模式下任何一个窗口在前台都算：用户正看着某个子窗口时不必再弹系统提示。
///
/// - `app`: 应用句柄
///
/// # 返回值
///
/// 有窗口处于前台时为 true
fn is_foreground(app: &AppHandle) -> bool {
    app.webview_windows()
        .values()
        .any(|window| window.is_focused().unwrap_or(false))
}

/// 启动器不在前台时发一条系统通知
///
/// 发送失败只记日志：通知是锦上添花，不该影响下载或安装本身的结果。
///
/// - `app`: 应用句柄
/// - `text`: 通知文案，标题与正文都跟界面语言走
pub(crate) fn notify_if_unfocused(app: &AppHandle, text: NotifyText) {
    if is_foreground(app) {
        return;
    }

    let (title, body) = text.parts();
    if let Err(error) = app.notification().builder().title(title).body(body).show() {
        mml_log::error(
            LogText::NotificationFailed {
                error: error.to_string(),
            }
            .text(),
        );
    }
}

/// 请求一次系统通知权限，启动时调用
///
/// Windows 与 Linux 上直接就是已授权；macOS 首次会弹系统授权框，
/// 之后由系统记住，所以只在启动时问一次。
///
/// - `app`: 应用句柄
pub(crate) fn request_permission(app: &AppHandle) {
    if let Err(error) = app.notification().request_permission() {
        mml_log::error(
            LogText::NotificationPermissionFailed {
                error: error.to_string(),
            }
            .text(),
        );
    }
}
