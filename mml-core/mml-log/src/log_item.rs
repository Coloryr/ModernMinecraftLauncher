//! 日志条目数据结构
//!
//! 定义日志系统中的日志级别枚举和单条日志条目结构。

use std::time::SystemTime;

use chrono::{DateTime, Datelike, Local, Timelike};

/// 日志级别
///
/// 从低到高依次为：Info → Warn → Error → Fault
pub(crate) enum LogLevel {
    /// 一般信息
    Info,
    /// 警告
    Warn,
    /// 错误
    Error,
    /// 严重错误/崩溃
    Fault,
}

/// 单条日志条目
///
/// 包含日志内容、级别和记录时间。
pub(crate) struct LogItem {
    /// 日志文本内容
    pub log: String,
    /// 日志级别
    level: LogLevel,
    /// 日志记录时间，系统时间
    time: SystemTime,
}

impl LogItem {
    /// 创建一条日志条目
    ///
    /// - `text`: 日志内容
    /// - `level`: 日志级别
    ///
    /// # 返回值
    ///
    /// 新建的日志条目，时间取当前系统时间
    pub fn new(text: String, level: LogLevel) -> Self {
        LogItem {
            log: text,
            level,
            time: SystemTime::now(),
        }
    }

    /// 获取格式化的时间字符串
    ///
    /// 形如 `2026-1-1 12:0:0`，月 / 日 / 时 / 分 / 秒均不补零
    ///
    /// # 返回值
    ///
    /// 格式化后的时间串，用的是系统本地时区
    pub fn get_time(&self) -> String {
        let time: DateTime<Local> = self.time.into();

        format!(
            "{}-{}-{} {}:{}:{}",
            time.year(),
            time.month(),
            time.day(),
            time.hour(),
            time.minute(),
            time.second()
        )
        .to_string()
    }

    /// 获取日志级别的字符串表示
    ///
    /// # 返回值
    ///
    /// 级别对应的固定字符串，日志输出用
    pub fn get_level(&self) -> &str {
        match self.level {
            LogLevel::Info => "Info",
            LogLevel::Warn => "Warn",
            LogLevel::Error => "Error",
            LogLevel::Fault => "Fault",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 日志级别字符串映射
    #[test]
    fn level_names() {
        assert_eq!(
            LogItem::new(String::from("a"), LogLevel::Info).get_level(),
            "Info"
        );
        assert_eq!(
            LogItem::new(String::from("a"), LogLevel::Warn).get_level(),
            "Warn"
        );
        assert_eq!(
            LogItem::new(String::from("a"), LogLevel::Error).get_level(),
            "Error"
        );
        assert_eq!(
            LogItem::new(String::from("a"), LogLevel::Fault).get_level(),
            "Fault"
        );
    }

    /// 日志条目应保留原文
    #[test]
    fn keep_text() {
        let item = LogItem::new(String::from("测试内容"), LogLevel::Warn);
        assert_eq!(item.log, "测试内容");
    }

    /// 时间格式为 `年-月-日 时:分:秒`，月 / 日 / 时分秒不补零，长度在 14~19 之间
    #[test]
    fn time_format() {
        let item = LogItem::new(String::from("t"), LogLevel::Info);
        let time = item.get_time();
        assert!((14..=19).contains(&time.len()), "时间长度异常: {time:?}");
        assert_eq!(time.matches('-').count(), 2, "应有 2 个 '-': {time:?}");
        assert_eq!(time.matches(':').count(), 2, "应有 2 个 ':': {time:?}");
        assert_eq!(
            time.matches(' ').count(),
            1,
            "日期与时间应以空格分隔: {time:?}"
        );
        // 年份是 4 位数字
        let year: String = time.chars().take(4).collect();
        assert!(
            year.chars().all(|c| c.is_ascii_digit()),
            "年份应为数字: {year:?}"
        );
    }
}
