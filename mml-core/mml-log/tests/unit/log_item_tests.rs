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

