//! Minecraft 崩溃日志分析
//!
//! 输入一份 crash-report 或 `latest.log` 文本，解析运行环境与异常链，再按规则给出
//! 结构化的结论。结论只带稳定的 [`FindingId`] 与机器可读细节，面向用户的文案由调用方
//! 按 id 取 i18n，本 crate 不产出句子。
//!
//! ```no_run
//! let report = mml_analysis::CrashReport::parse("---- Minecraft Crash Report ----");
//! let result = report.analyze();
//! if let Some(finding) = result.primary() {
//!     println!("{}", finding.id.as_str());
//! }
//! ```

use std::path::Path;

mod model;
mod parse;
mod rules;

pub use model::{
    Confidence, CrashAnalysis, CrashReport, Environment, ExceptionFrame, Finding, FindingId,
    LoaderKind, LogKind, ModEntry, Severity,
};

/// 分析一份崩溃报告或游戏日志
///
/// - `text`: 日志全文，UTF-8
///
/// # 返回值
///
/// 见 [`CrashAnalysis`]
pub fn analyze(text: &str) -> CrashAnalysis {
    CrashReport::parse(text).analyze()
}

/// 判断文件是否属于崩溃分析的范围
///
/// 只认游戏日志 `.log` 与 Minecraft 生成的 `crash-*.txt`；
/// JVM 致命错误日志 `hs_err_pid*.log` 暂不分析。
///
/// - `path`: 文件路径
///
/// # 返回值
///
/// 在范围内返回 `true`
pub fn is_supported_path(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let name = name.to_ascii_lowercase();

    if name.starts_with("hs_err_pid") {
        return false;
    }

    name.ends_with(".log") || (name.starts_with("crash-") && name.ends_with(".txt"))
}

/// 读取并分析一个日志文件
///
/// 只接受 [`is_supported_path`] 认的日志；文件按 UTF-8 解码，非法字节会被替换，
/// GBK 日志请调用方先解码再走 [`analyze`]。
///
/// - `path`: 日志文件路径
///
/// # 返回值
///
/// 分析结果；路径不在范围内或读取失败时返回 io 错误
pub fn analyze_file(path: impl AsRef<Path>) -> std::io::Result<CrashAnalysis> {
    let path = path.as_ref();
    if !is_supported_path(path) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "only .log or crash report files are analyzed",
        ));
    }

    let bytes = std::fs::read(path)?;
    let text = String::from_utf8_lossy(&bytes);
    Ok(analyze(&text))
}
