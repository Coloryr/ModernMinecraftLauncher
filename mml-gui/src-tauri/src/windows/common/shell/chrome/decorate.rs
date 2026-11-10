//! macOS 原生红黄绿：位置与隐藏
//!
//! 只在 macOS 上编译；插件摆的是 AppKit 原生按钮，位置要在窗口激活后、显示前定好。

/// macOS 原生红黄绿距窗口左缘的点数
#[cfg(target_os = "macos")]
const TRAFFIC_LIGHTS_INSET_X: f32 = 16.0;
/// macOS 原生红黄绿所在标题栏容器比按钮多出的高度，按钮 14 + 38 = 52，与 `--titlebar-h` 对齐
#[cfg(target_os = "macos")]
const TRAFFIC_LIGHTS_INSET_Y: f32 = 38.0;

/// 按本地日期判断今天是否要换用另一套标题栏样式
///
/// # 返回值
///
/// 需要换用时返回 `true`
#[cfg(target_os = "macos")]
fn is_april_fools() -> bool {
    use chrono::{Datelike, Local};

    let now = Local::now();
    now.month() == 4 && now.day() == 1
}

/// 藏掉 macOS 的原生红黄绿，改由前端按对调后的样式自己画
///
/// - `window`: 目标窗口
///
/// # 返回值
///
/// 取不到原生窗口或主线程调度失败时返回错误文案
#[cfg(target_os = "macos")]
fn hide_native_traffic_lights(window: &WebviewWindow) -> Result<(), String> {
    use objc2_app_kit::{NSWindow, NSWindowButton};
    use tauri::Manager;

    let handle = window.app_handle().clone();
    let window = window.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    handle
        .run_on_main_thread(move || {
            let result = (|| -> Result<(), String> {
                let pointer = window.ns_window().map_err(|err| err.to_string())?;
                if pointer.is_null() {
                    return Ok(());
                }
                // SAFETY: 主线程上执行；句柄取自本窗口且期间存活，只改按钮可见性
                unsafe {
                    let ns_window = &*(pointer as *const NSWindow);
                    for kind in [
                        NSWindowButton::CloseButton,
                        NSWindowButton::MiniaturizeButton,
                        NSWindowButton::ZoomButton,
                    ] {
                        if let Some(button) = ns_window.standardWindowButton(kind) {
                            button.setHidden(true);
                        }
                    }
                }
                Ok(())
            })();
            let _ = tx.send(result);
        })
        .map_err(|err| err.to_string())?;
    rx.recv().map_err(|err| err.to_string())?
}
