use mml_base::version_parse::{
    is_game_version_117, is_game_version_120, is_game_version_1202, is_game_version_greater,
    is_game_version_greater_equal, parse_game_version,
};

/// 校验 1.20.2 门槛判断，含新格式、快照与预发布
#[test]
fn test_is_game_version_1202() {
    // 低于 1.20.2
    assert_eq!(is_game_version_1202("1.20.1"), false);
    assert_eq!(is_game_version_1202("1.20.0"), false);
    assert_eq!(is_game_version_1202("1.19.4"), false);
    assert_eq!(is_game_version_1202("1.7.10"), false);
    assert_eq!(is_game_version_1202("a1.0.16"), false);
    assert_eq!(is_game_version_1202("b1.7.3"), false);

    // 等于 1.20.2
    assert_eq!(is_game_version_1202("1.20.2"), true);

    // 高于 1.20.2
    assert_eq!(is_game_version_1202("1.20.3"), true);
    assert_eq!(is_game_version_1202("1.20.4"), true);
    assert_eq!(is_game_version_1202("1.21"), true);
    assert_eq!(is_game_version_1202("1.21.1"), true);

    // 新格式
    assert_eq!(is_game_version_1202("26.1"), true);
    assert_eq!(is_game_version_1202("26.1.1"), true);

    // 快照和预发布
    assert_eq!(is_game_version_1202("24w13a"), false); // 1.20.4 的快照，但时间线上早于 1.20.2
    assert_eq!(is_game_version_1202("1.20.2-pre1"), false); // 预发布 < 正式版
    assert_eq!(is_game_version_1202("1.20.2-rc1"), false); // RC < 正式版
}

/// 校验 1.20 门槛判断，含新格式、快照与预发布
#[test]
fn test_is_game_version_120() {
    // 低于 1.20
    assert_eq!(is_game_version_120("1.19.2"), false);
    assert_eq!(is_game_version_120("1.19.4"), false);
    assert_eq!(is_game_version_120("1.7.10"), false);
    assert_eq!(is_game_version_120("a1.0.16"), false);
    assert_eq!(is_game_version_120("b1.7.3"), false);

    // 等于 1.20
    assert_eq!(is_game_version_120("1.20.0"), true);

    // 高于 1.20
    assert_eq!(is_game_version_120("1.20.3"), true);
    assert_eq!(is_game_version_120("1.20.4"), true);
    assert_eq!(is_game_version_120("1.21"), true);
    assert_eq!(is_game_version_120("1.21.1"), true);

    // 新格式
    assert_eq!(is_game_version_120("26.1"), true);
    assert_eq!(is_game_version_120("26.1.1"), true);

    // 快照和预发布
    assert_eq!(is_game_version_120("24w13a"), false); // 1.20.4 的快照，但时间线上早于 1.20.2
    assert_eq!(is_game_version_120("1.20-pre1"), false); // 预发布 < 正式版
    assert_eq!(is_game_version_120("1.20-rc1"), false); // RC < 正式版
}

/// 校验 1.17 门槛判断，含新格式、快照与预发布
#[test]
fn test_is_game_version_117() {
    // 低于 1.17
    assert_eq!(is_game_version_117("1.16.5"), false);
    assert_eq!(is_game_version_117("1.7.10"), false);
    assert_eq!(is_game_version_117("a1.0.16"), false);
    assert_eq!(is_game_version_117("b1.7.3"), false);

    // 等于 1.17
    assert_eq!(is_game_version_117("1.17.0"), true);

    // 高于 1.17
    assert_eq!(is_game_version_117("1.20.3"), true);
    assert_eq!(is_game_version_117("1.20.4"), true);
    assert_eq!(is_game_version_117("1.21"), true);
    assert_eq!(is_game_version_117("1.21.1"), true);

    // 新格式
    assert_eq!(is_game_version_117("26.1"), true);
    assert_eq!(is_game_version_117("26.1.1"), true);

    // 快照和预发布
    assert_eq!(is_game_version_117("24w13a"), false); // 1.20.4 的快照，但时间线上早于 1.20.2
    assert_eq!(is_game_version_117("1.20-pre1"), false); // 预发布 < 正式版
    assert_eq!(is_game_version_117("1.20-rc1"), false); // RC < 正式版
}

/// 校验版本号严格大于比较，含新格式各类型与无法解析的输入
#[test]
fn test_version_comparison() {
    // 正式版比较
    assert!(is_game_version_greater("1.20.4", "1.20.2"));
    assert!(is_game_version_greater("1.21", "1.20.4"));
    assert!(!is_game_version_greater("1.20.1", "1.20.2"));

    // 新格式比较
    assert!(is_game_version_greater("26.1", "1.21.1"));
    assert!(is_game_version_greater("26.1.1", "26.1"));

    // 快照与正式版
    assert!(!is_game_version_greater("24w13a", "1.20.2"));

    // 预发布与正式版
    assert!(!is_game_version_greater("1.20.2-pre1", "1.20.2"));

    // 新格式各类型
    assert!(is_game_version_greater("26.1", "26.1-snapshot-1"));
    assert!(is_game_version_greater("26.1-pre-1", "26.1-snapshot-1"));
    assert!(is_game_version_greater("26.1-rc-1", "26.1-snapshot-1"));
    assert!(is_game_version_greater("26.1", "26.1-rc-1"));

    // 无法解析时返回 false，而不是 panic
    assert!(!is_game_version_greater("unknown", "1.20.2"));
    assert!(!is_game_version_greater("1.20.2", "unknown"));
}

/// 校验版本号大于等于比较，含无法解析的输入
#[test]
fn test_version_comparison_equal() {
    // >=
    assert!(is_game_version_greater_equal("1.20.2", "1.20.2"));
    assert!(is_game_version_greater_equal("1.20.3", "1.20.2"));
    assert!(!is_game_version_greater_equal("1.20.1", "1.20.2"));

    // 新格式
    assert!(is_game_version_greater_equal("26.1", "26.1"));
    assert!(is_game_version_greater_equal("26.1", "1.21.1"));

    // 无法解析时返回 false，而不是 panic
    assert!(!is_game_version_greater_equal("unknown", "1.20.2"));
    assert!(!is_game_version_greater_equal("1.20.2", "unknown"));
}

/// 版本号解析：覆盖正式版、快照、预发布、RC、新格式、远古版本。
#[test]
fn parse_game_version_formats() {
    // 旧格式正式版
    assert_eq!(parse_game_version("1.20.4"), Some(vec![1, 20, 4]));
    assert_eq!(parse_game_version("1.20"), Some(vec![1, 20, 0]));
    assert_eq!(parse_game_version("1.7.10"), Some(vec![1, 7, 10]));

    // 新格式正式版，编码为 100 + 年份
    assert_eq!(parse_game_version("26.1"), Some(vec![126, 1, 0]));
    assert_eq!(parse_game_version("26.1.1"), Some(vec![126, 1, 1]));

    // 新格式快照 / 预发布 / RC
    assert_eq!(
        parse_game_version("26.1-snapshot-11"),
        Some(vec![10, 126, 1, 11])
    );
    assert_eq!(parse_game_version("26.1-pre-1"), Some(vec![30, 126, 1, 1]));
    assert_eq!(parse_game_version("26.1-rc-2"), Some(vec![20, 126, 1, 2]));

    // 旧格式快照
    assert_eq!(parse_game_version("24w13a"), Some(vec![-10, 2024, 13, 1]));

    // 旧格式预发布 / RC
    assert_eq!(
        parse_game_version("1.20.4-pre1"),
        Some(vec![-5, 1, 20, 4, 1])
    );
    assert_eq!(
        parse_game_version("1.20.4-rc1"),
        Some(vec![-3, 1, 20, 4, 1])
    );

    // 远古版本
    assert_eq!(parse_game_version("a1.0.16"), Some(vec![-30, 1, 0, 16]));
    assert_eq!(parse_game_version("b1.7.3"), Some(vec![-20, 1, 7, 3]));
    assert_eq!(parse_game_version("rd-160052"), Some(vec![-40, 160052]));

    // 无法解析
    assert_eq!(parse_game_version(""), None);
    assert_eq!(parse_game_version("latest"), None);
    assert_eq!(parse_game_version("snapshot"), None);
    // 非数字段被过滤：首位数字成为主版本，属历史行为
    assert_eq!(parse_game_version("1.x"), Some(vec![1, 0, 0]));
    assert_eq!(parse_game_version("x.1.2"), Some(vec![1, 2, 0]));
    // 主版本既非旧格式的 1，也不满足新格式的 >=25，无法解析
    assert_eq!(parse_game_version("2.0"), None);
    assert_eq!(parse_game_version("20.1"), None);
}

/// 版本排序关系：正式版 > RC > 预发布 > 快照，旧格式早于新格式。
#[test]
fn parse_game_version_ordering() {
    // 旧格式：补丁号决定顺序
    assert!(parse_game_version("1.20.4") > parse_game_version("1.20.2"));
    assert!(parse_game_version("1.21") > parse_game_version("1.20.4"));
    assert!(parse_game_version("1.20.1") < parse_game_version("1.20.2"));

    // 新格式整体晚于旧格式，年份 >= 25 时编码为 100 + 年份
    assert!(parse_game_version("26.1") > parse_game_version("1.21.1"));

    // 新格式内部：正式版 > 预发布 > RC > 快照
    let release = parse_game_version("26.1").unwrap();
    let pre = parse_game_version("26.1-pre-1").unwrap();
    let rc = parse_game_version("26.1-rc-2").unwrap();
    let snap = parse_game_version("26.1-snapshot-11").unwrap();
    assert!(pre > rc, "预发布应晚于 RC");
    assert!(rc > snap, "RC 应晚于快照");
    assert!(release > pre, "正式版应晚于预发布");

    // 旧格式快照早于旧格式正式版
    assert!(parse_game_version("24w13a") < parse_game_version("1.20.2"));
}
