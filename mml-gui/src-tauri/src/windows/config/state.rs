//! 窗口自身的偏好
//!
//! 目前只有文件树的宽度，落在 gui 配置里跟着窗口走。






/// 配置文件窗口左栏的宽度，单位像素
///
/// # 返回值
///
/// 上次拖出来的宽度；没拖过时是默认的 300
#[gui_macros::ipc_group("config")]
#[tauri::command]
pub fn config_tree_width() -> f64 {
    crate::gui_config::get().config_window.tree_width
}

/// 记住左栏宽度，落 `gui_config.json`
///
/// - `width`: 宽度，单位像素；越界夹到 200 到 640
#[gui_macros::ipc_group("config")]
#[tauri::command]
pub fn config_set_tree_width(width: f64) {
    let width = width.clamp(200.0, 640.0);
    let mut config = crate::gui_config::get();
    if (config.config_window.tree_width - width).abs() < f64::EPSILON {
        return;
    }
    config.config_window.tree_width = width;
    crate::gui_config::set(config);
}
