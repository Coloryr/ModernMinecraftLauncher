//! 发布模式下关掉 WebView2 自带的浏览器快捷键与 DevTools
//!
//! F5 / Ctrl+R 会**重新加载页面**，用户视角就是"启动器界面被刷新了"：当前状态全丢，
//! 看着像程序坏了。F12 / Ctrl+Shift+C 会打开 DevTools。这些都不是启动器该有的功能键。
//!
//! 用 WebView2 的两个官方设置项，都只在**发布模式**下关，debug 保留以便调试：
//!
//! - `AreBrowserAcceleratorKeysEnabled = false` —— 关掉"浏览器专有"的快捷键：
//!   页内查找 Ctrl+F / F3、打印 Ctrl+P、**重载 Ctrl+R / F5**、缩放 Ctrl±、
//!   **DevTools Ctrl+Shift+C / F12**、以及后退 / 前进 / 搜索这类特殊键。
//!   **不影响**与移动、文本编辑有关的键：Home / End / PgUp / PgDn、
//!   **Ctrl+X / Ctrl+C / Ctrl+V**、Ctrl+A、Ctrl+Z，微软文档明确列出。
//! - `AreDevToolsEnabled = false` —— DevTools 本身也关掉，F12 之外还有右键"检查"等入口。
//!
//! 时机：`on_webview_ready` 拿到 `ICoreWebView2` 后设一次，**每个 webview 各自设**，
//! 设置项是 per-webview 的。

use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2, ICoreWebView2Settings3};
use windows::core::Interface;

/// 关掉浏览器快捷键与 DevTools，仅发布模式；debug 下原样保留
///
/// - `core`: WebView2 的 `ICoreWebView2`，设置项是 per-webview 的
pub fn disable_browser_keys(core: &ICoreWebView2) {
    if cfg!(debug_assertions) {
        return;
    }

    let Ok(settings) = (unsafe { core.Settings() }) else {
        mml_log::error(String::from("[webview] 取 WebView2 设置失败，快捷键未能关闭"));
        return;
    };

    // DevTools：F12 之外，右键菜单等入口也一并关掉
    if let Err(err) = unsafe { settings.SetAreDevToolsEnabled(false) } {
        mml_log::error(format!("[webview] 关闭 DevTools 失败: {err}"));
    }

    // 浏览器快捷键 F5 / Ctrl+R 重载、Ctrl+P、缩放…：老版本 WebView2 没有 Settings3，
    // 拿不到就只关 DevTools，不报错
    match settings.cast::<ICoreWebView2Settings3>() {
        Ok(settings3) => {
            if let Err(err) = unsafe { settings3.SetAreBrowserAcceleratorKeysEnabled(false) } {
                mml_log::error(format!("[webview] 关闭浏览器快捷键失败: {err}"));
            }
        }
        Err(_) => {
            mml_log::info(String::from(
                "[webview] 当前 WebView2 不支持关闭浏览器快捷键（Settings3 缺失）",
            ));
        }
    }
}
