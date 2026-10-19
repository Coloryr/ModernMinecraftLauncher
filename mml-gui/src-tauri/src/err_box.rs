//! 致命错误弹窗

use mml_names::{i18, i18_items::error_type::ErrorType};

/// 弹系统错误框并退出程序，用于核心初始化失败等致命场景
///
/// - `e`: 错误类型，经 i18n 转成当前语言的文本后显示
///
/// 本函数不返回：用户关掉弹窗后进程以 1 退出
pub fn fatal_error(e: ErrorType) -> ! {
    let msg = i18::get_error(e);
    eprintln!("{msg}");

    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title(
            crate::log_text::LogText::FatalInitFailed.text_in(&mml_names::get_lang_type()),
        )
        .set_description(msg.as_str())
        .show();

    std::process::exit(1)
}

/// 弹系统错误框并退出程序，错误文本已经是成品
///
/// - `text`: 直接显示的错误文本，不再经 i18n
///
/// 本函数不返回：用户关掉弹窗后进程以 1 退出
pub fn fatal_error_text(text: &str) -> ! {
    eprintln!("{text}");

    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title(
            crate::log_text::LogText::FatalRunFailed.text_in(&mml_names::get_lang_type()),
        )
        .set_description(text)
        .show();

    std::process::exit(1)
}
