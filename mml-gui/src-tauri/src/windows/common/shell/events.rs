//! 壳层向窗口发的事件
//!
//! 拖入文件路径与原生菜单动作都只发给目标窗口，事件名在 `listens` 里。

use tauri::{AppHandle, Emitter};

use crate::listens;

/// 拖入文件的真实路径，由 DOM 拖放的"路径桥"上报，见 [`file_drop`]
///
/// 只发给**目标窗口**：拖到哪个窗口就只该那个窗口响应。前端把它与 DOM 的 `drop`
/// 里的文本 / 链接合成同一个"拖入"回调。
///
/// - `app`: 应用句柄，发事件用它
/// - `label`: 目标窗口标签
/// - `payload`: 拖入文件的真实绝对路径
#[cfg(windows)]
#[gui_macros::emit]
pub(crate) fn emit_file_drop(app: &AppHandle, label: &str, payload: crate::dtos::FileDropDto) {
    let _ = app.emit_to(label, listens::FILE_DROP, payload);
}

/// 原生菜单点中的动作交给窗口处理
///
/// 菜单按焦点窗口重建，见 [`crate::menu`]；这里只把动作 id 送到那个窗口，
/// 具体做什么由前端按窗口分发。
///
/// - `app`: 应用句柄，发事件用它
/// - `label`: 接收事件的窗口标签
/// - `action`: 动作 id，如 `refresh` / `settings`
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[gui_macros::emit]
pub(crate) fn emit_menu_action(app: &AppHandle, label: &str, action: &str) {
    let _ = app.emit_to(
        label,
        listens::MENU_ACTION,
        crate::dtos::MenuActionEvent {
            action: action.to_string(),
        },
    );
}
