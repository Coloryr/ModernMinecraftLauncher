use super::*;

/// 旧格式正式版解析
#[test]
fn test_parse_old_release() {
    assert_eq!(parse_game_version("1.20.4"), Some(vec![1, 20, 4]));
    assert_eq!(parse_game_version("1.20"), Some(vec![1, 20, 0]));
    assert_eq!(parse_game_version("1.20.0"), Some(vec![1, 20, 0]));
    assert_eq!(parse_game_version("1.7.10"), Some(vec![1, 7, 10]));
    assert_eq!(parse_game_version(" 1.21 "), Some(vec![1, 21, 0]));
}

/// 新格式正式版解析，2026 年起
#[test]
fn test_parse_new_release() {
    assert_eq!(parse_game_version("26.1"), Some(vec![126, 1, 0]));
    assert_eq!(parse_game_version("26.1.1"), Some(vec![126, 1, 1]));
    // 年份过小不算新格式
    assert_eq!(parse_game_version("24.1"), None);
}

/// 新格式快照 / 预发布 / RC
#[test]
fn test_parse_new_snapshot_pre_rc() {
    assert_eq!(
        parse_game_version("26.1-snapshot-1"),
        Some(vec![10, 126, 1, 1])
    );
    assert_eq!(parse_game_version("26.1-pre-1"), Some(vec![30, 126, 1, 1]));
    assert_eq!(parse_game_version("26.1-rc-2"), Some(vec![20, 126, 1, 2]));
}

/// 旧格式快照 24w13a
#[test]
fn test_parse_old_snapshot() {
    assert_eq!(parse_game_version("24w13a"), Some(vec![-10, 2024, 13, 1]));
    assert_eq!(parse_game_version("24w13"), Some(vec![-10, 2024, 13, 0]));
    // 周编号小于 10
    assert_eq!(parse_game_version("25w03a"), Some(vec![-10, 2025, 3, 1]));
}

/// 旧格式预发布 / RC
#[test]
fn test_parse_old_prerelease() {
    assert_eq!(
        parse_game_version("1.20.4-pre1"),
        Some(vec![-5, 1, 20, 4, 1])
    );
    assert_eq!(
        parse_game_version("1.20.4-rc1"),
        Some(vec![-3, 1, 20, 4, 1])
    );
    // 现状记录，疑似 bug：pre/rc 缺编号时该分支返回 None，随后由 parse_old_release
    // 的 filter_map 兜底解析成正式版 1.20.0，预发布标记被静默丢弃
    assert_eq!(parse_game_version("1.20.4-pre"), Some(vec![1, 20, 0]));
}

/// 远古版本
#[test]
fn test_parse_ancient_version() {
    assert_eq!(parse_game_version("a1.0.16"), Some(vec![-30, 1, 0, 16]));
    assert_eq!(parse_game_version("b1.7.3"), Some(vec![-20, 1, 7, 3]));
    assert_eq!(parse_game_version("rd-160052"), Some(vec![-40, 160052]));
}

/// 无法解析的版本
#[test]
fn test_parse_invalid() {
    assert_eq!(parse_game_version(""), None);
    assert_eq!(parse_game_version("abc"), None);
    assert_eq!(parse_game_version("foo"), None);
}

/// 版本号大小比较
#[test]
fn test_is_game_version_greater() {
    assert!(is_game_version_greater("1.20.4", "1.20.3"));
    assert!(is_game_version_greater("1.21", "1.20.9"));
    // 新格式大于一切旧格式
    assert!(is_game_version_greater("26.1", "1.21.4"));
    // 快照小于正式版
    assert!(!is_game_version_greater("24w13a", "1.20.4"));
    assert!(!is_game_version_greater("1.20.4", "1.20.4"));
    // 解析失败时返回 false
    assert!(!is_game_version_greater("abc", "1.20"));
    assert!(!is_game_version_greater("1.20", "abc"));
}

/// 相同版本判定：解析结果相等为真，不等或任一无法解析为假
#[test]
fn test_is_game_version_equal() {
    assert!(is_game_version_equal("1.20", "1.20"));
    assert!(is_game_version_equal("1.20.4", "1.20.4"));
    assert!(is_game_version_equal("a1.0.16", "a1.0.16"));
    assert!(!is_game_version_equal("1.20", "1.21"));
    assert!(!is_game_version_equal("1.21", "1.20"));
    // 解析失败时返回 false
    assert!(!is_game_version_equal("abc", "1.20"));
    assert!(!is_game_version_equal("1.20", "abc"));
}

/// is_game_version_greater_equal 与版本门槛判断
#[test]
fn test_is_game_version_greater_equal_and_marks() {
    assert!(is_game_version_greater_equal("1.17", "1.17"));
    assert!(is_game_version_greater_equal("1.18", "1.17"));
    assert!(!is_game_version_greater_equal("1.16.5", "1.17"));

    assert!(!is_game_version_117("1.16.5"));
    assert!(is_game_version_117("1.17"));
    assert!(is_game_version_120("1.20"));
    assert!(!is_game_version_120("1.19.4"));
    assert!(is_game_version_1202("1.20.2"));
    assert!(!is_game_version_1202("1.20.1"));
}

