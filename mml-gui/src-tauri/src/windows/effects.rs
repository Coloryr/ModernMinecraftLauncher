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
/// - `mode`: 客户端设置里的窗口材质，`auto` / `mica` / `acrylic` / `none`（老配置里的 `blur` 按亚克力铺）
pub fn apply(window: &WebviewWindow, mode: &str) {
    #[cfg(target_os = "windows")]
    {
        // 换材质要先清掉上一个：它们都是 DWM 的系统背景，直接叠加不会互相替换
        let _ = window_vibrancy::clear_mica(window);
        let _ = window_vibrancy::clear_acrylic(window);
        let _ = window_vibrancy::clear_blur(window);

        let applied = match mode {
            "none" => Ok(()),
            "mica" => window_vibrancy::apply_mica(window, None),
            "acrylic" => window_vibrancy::apply_acrylic(window, None),
            // 模糊是 Win11 22H1 的老路径，22H2 起铺上去就是整窗全黑（crate 的 README 标着
            // 22H1 only，我们在 build 26220 上实测过）。老配置里可能还留着 blur，按亚克力铺，
            // 别让它把窗口变黑
            "blur" => window_vibrancy::apply_acrylic(window, None),
            // auto：从新到旧逐个试。云母只有 Win11 认，亚克力要 Win10 1809+，都没有就没救了；
            // 不再退回 blur，理由同上。按库的返回值降级比先探测系统版本稳
            _ => window_vibrancy::apply_mica(window, None)
                .or_else(|_| window_vibrancy::apply_acrylic(window, None)),
        };
        // 铺不上就退回不透明底色：原因留给用户贴日志，光看界面分不清是系统不支持
        // 还是「透明效果」被关了
        if let Err(err) = applied {
            mml_log::error(
                crate::log_text::LogText::WindowEffectFailed {
                    label: window.label().to_string(),
                    mode: mode.to_string(),
                    error: err.to_string(),
                }
                .text(),
            );
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
