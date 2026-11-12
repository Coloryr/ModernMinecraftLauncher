//! 前端错误转发：webview 里的 `console.error` 与未捕获异常写进启动器日志
//!
//! 前端没有落日志的能力，控制台里的东西用户贴不出来；用户反馈问题时贴的是 `logs.log`，
//! 所以这里给一条转发通道。级别统一 Error。

use crate::log_text::LogText;

/// 把前端报上来的错误写进日志
///
/// - `window`: 出错的窗口 kind，形如 `main` / `resource`；拿不到时是空串
/// - `message`: 前端拼好的错误文本，已带模块前缀
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub fn window_log_frontend(window: String, message: String) {
    mml_log::error(LogText::FrontendError { window, message }.text());
}