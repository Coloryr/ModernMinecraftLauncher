//! 致命错误弹窗
//!
//! 用在进程必须立刻终止的场合。日志系统可能还没起来（核心 init 失败时就是这样），
//! 所以同时写 stderr 与运行目录的 `logs.log`：release 没有控制台，只写 stderr 等于没写。

use std::io::Write;
use std::path::Path;

use mml_names::{i18, i18_items::error_type::ErrorType, names};

/// 把致命错误追加进运行目录的 `logs.log`
///
/// 不走 `mml_log`：日志系统正是可能还没起来的那一环。写不进去就只留 stderr，
/// 不能因为记日志再崩一次。
///
/// - `dir`: 运行目录
/// - `msg`: 已经翻好的错误文本
fn append_log(dir: &Path, msg: &str) {
    let file = dir.join(names::LOG_FILE);
    let line = format!(
        "[{}][Error][fatal] {msg}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)
    {
        let _ = file.write_all(line.as_bytes());
    }
}

/// 弹系统错误框并退出程序，用于核心初始化失败等致命场景
///
/// - `dir`: 运行目录，错误同时追加进它的 `logs.log`
/// - `e`: 错误类型，经 i18n 转成当前语言的文本后显示
///
/// 本函数不返回：用户关掉弹窗后进程以 1 退出
pub fn fatal_error(dir: &Path, e: ErrorType) -> ! {
    let msg = i18::get_error(e);
    append_log(dir, &msg);
    eprintln!("{msg}");

    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title(crate::log_text::LogText::FatalInitFailed.text_in(&mml_names::get_lang_type()))
        .set_description(msg.as_str())
        .show();

    std::process::exit(1)
}

/// 弹系统错误框并退出程序，错误文本已经是成品
///
/// - `dir`: 运行目录，错误同时追加进它的 `logs.log`
/// - `text`: 直接显示的错误文本，不再经 i18n
///
/// 本函数不返回：用户关掉弹窗后进程以 1 退出
pub fn fatal_error_text(dir: &Path, text: &str) -> ! {
    append_log(dir, text);
    eprintln!("{text}");

    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title(crate::log_text::LogText::FatalRunFailed.text_in(&mml_names::get_lang_type()))
        .set_description(text)
        .show();

    std::process::exit(1)
}
