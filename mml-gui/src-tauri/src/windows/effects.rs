//! 窗口材质：给窗口铺系统级的毛玻璃效果
//!
//! 口径是**按能力逐个降级**，而不是先探测系统版本再选：版本判断容易过时，而库在能力不足时
//! 会返回错误，直接按返回值降级更稳。
//!
//! - Windows 11：云母 Mica；
//! - Windows 10 1809 及以上：亚克力 Acrylic；
//! - 更老的 Windows：Blur；
//! - macOS：Vibrancy，取侧栏材质；
//! - Linux：不支持，什么都不做。
//!
//! 注意：效果要**透明窗口**才看得见，被不透明的窗口底色盖住时它仍在生效但看不出来。

use tauri::WebviewWindow;

/// 给窗口铺系统材质；不支持的平台什么都不做
///
/// - `window`: 已建好的窗口
/// - `mode`: 客户端设置里的窗口材质，`auto` / `mica` / `acrylic` / `blur` / `none`
pub fn apply(window: &WebviewWindow, mode: &str) {
    #[cfg(target_os = "windows")]
    {
        // 换材质要先清掉上一个：它们都是 DWM 的系统背景，直接叠加不会互相替换
        let _ = window_vibrancy::clear_mica(window);
        let _ = window_vibrancy::clear_acrylic(window);
        let _ = window_vibrancy::clear_blur(window);

        match mode {
            "none" => {}
            "mica" => {
                let _ = window_vibrancy::apply_mica(window, None);
            }
            "acrylic" => {
                let _ = window_vibrancy::apply_acrylic(window, None);
            }
            "blur" => {
                let _ = window_vibrancy::apply_blur(window, None);
            }
            // auto：从新到旧逐个试。云母只有 Win11 认，亚克力要 Win10 1809+，
            // 都没有时退回 Blur；按库的返回值降级比先探测系统版本稳
            _ => {
                if window_vibrancy::apply_mica(window, None).is_ok() {
                    return;
                }
                if window_vibrancy::apply_acrylic(window, None).is_ok() {
                    return;
                }
                let _ = window_vibrancy::apply_blur(window, None);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        if mode == "none" {
            return;
        }
        let _ = window_vibrancy::apply_vibrancy(
            window,
            window_vibrancy::NSVisualEffectMaterial::Sidebar,
            Some(window_vibrancy::NSVisualEffectState::Active),
            None,
        );
    }

    // Linux：效果由桌面合成器决定，库不支持，什么都不做
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let _ = (window, mode);
}
