use super::*;

#[test]
fn version_compare_basics() {
    assert_eq!(compare_versions("1.0", "1.0.0"), Ordering::Equal);
    assert_eq!(compare_versions("1.0", "1.0-alpha"), Ordering::Greater);
    assert_eq!(compare_versions("1.10", "1.9"), Ordering::Greater);
    assert_eq!(compare_versions("2.0-SNAPSHOT", "2.0"), Ordering::Less);
}

#[test]
fn maven_returns_version_range_directly() {
    let r: VersionRange = parse_maven_version_range("[7.0.0,)").unwrap();
    assert!(r.contains("7.0.0"));
    assert!(r.contains("8.0.0"));
    assert!(!r.contains("6.9.9"));

    let r = parse_maven_version_range("[1.0.0,2.0.0)").unwrap();
    assert!(r.contains("1.0.0"));
    assert!(r.contains("1.9.9"));
    assert!(!r.contains("2.0.0"));

    let r = parse_maven_version_range("(,1.0.0),[1.1.0,)").unwrap();
    assert!(r.contains("0.9"));
    assert!(!r.contains("1.0.0"));
    assert!(r.contains("1.1.0"));
    assert!(r.contains("3.0.0"));
}

/// SemVer 的构建元数据（`+` 之后）不参与比较
#[test]
fn build_metadata_is_ignored() {
    let r = parse_maven_version_range("[9.5.1,)").unwrap();
    assert!(r.contains("9.5.1+1.21.1"));
    assert!(!r.contains("9.5.0+1.21.1"));
}

/// Maven 里裸版本号是"推荐版本"，不限制任何版本
#[test]
fn maven_bare_version_is_any() {
    assert_eq!(
        parse_version_range_with("4.7.5.1", RangeSyntax::Maven).unwrap(),
        VersionRange::Any
    );
    // semver 那边按"这一档"匹配，不是精确相等
    let r = parse_version_range_with("4.7.5.1", RangeSyntax::Semver).unwrap();
    assert!(r.contains("4.7.5.1"));
    assert!(!r.contains("4.7.6"));
}

/// 闭区间上界要能装下同版本
#[test]
fn inclusive_upper_bound_matches() {
    let r = parse_version_range_with("[1.21,1.21.1]", RangeSyntax::Maven).unwrap();
    assert!(r.contains("1.21.1"));
    assert!(r.contains("1.21"));
    assert!(!r.contains("1.21.2"));
}

/// Forge 里常见的两种畸形写法：逐个列版本、多段并集
#[test]
fn maven_bracket_lists() {
    // `[1.21,1.21.1,1.21.2]`：三个候选版本
    let r = parse_maven_version_range("[1.21,1.21.1,1.21.2]").unwrap();
    assert!(r.contains("1.21.1"));
    assert!(!r.contains("1.21.3"));

    // `[1.21],[1.21.1]`：两段精确版本的并集
    let r = parse_maven_version_range("[1.21],[1.21.1]").unwrap();
    assert!(r.contains("1.21"));
    assert!(r.contains("1.21.1"));
    assert!(!r.contains("1.22"));
}

/// 边界不是干净版本号时不比版本（上游加载器放行，别误报）
#[test]
fn odd_bounds_are_not_judged() {
    assert!(
        parse_maven_version_range("[1.21,1.21z)")
            .unwrap()
            .contains("1.21.1")
    );
    assert!(
        parse_maven_version_range("[1.21-3.1.0c,)")
            .unwrap()
            .contains("1.21.1-3.6.8")
    );
}

/// 不带逗号的 `[1.20.1]` 是精确版本，不能因为"解析不出区间"就退回不限版本
#[test]
fn maven_exact_bracket_range() {
    let r = parse_maven_version_range("[1.20.1]").unwrap();
    assert_eq!(r, VersionRange::Exact("1.20.1".into()));
    assert!(r.contains("1.20.1"));
    assert!(r.contains("1.20.1.0"));
    assert!(!r.contains("1.20.2"));
    // 精确就是精确：`[1.21]` 不吃 1.21.1；游戏版本那一档的放宽在 game_mods 里
    assert!(
        !parse_maven_version_range("[1.21]")
            .unwrap()
            .contains("1.21.1")
    );
}

#[test]
fn fabric_returns_version_range_directly() {
    let r: VersionRange = parse_fabric_version_range(">=0.19.5").unwrap();
    assert!(r.contains("0.19.5"));
    assert!(!r.contains("0.19.4"));

    let r = parse_fabric_version_range("*").unwrap();
    assert_eq!(r, VersionRange::Any);
    assert!(r.contains("任何东西"));

    let r = parse_fabric_version_range("~1.2.3").unwrap();
    assert!(r.contains("1.2.9"));
    assert!(!r.contains("1.3.0"));

    let r = parse_fabric_version_range("^1.2.3").unwrap();
    assert!(r.contains("1.9.0"));
    assert!(!r.contains("2.0.0"));

    let r = parse_fabric_version_range("^0.2.3").unwrap();
    assert!(r.contains("0.2.9"));
    assert!(!r.contains("0.3.0"));

    let r = parse_fabric_version_range(">=1.0.0 <2.0.0").unwrap();
    assert!(r.contains("1.5.0"));
    assert!(!r.contains("2.0.0"));

    let r = parse_fabric_version_range("1.0.0 || 2.0.0").unwrap();
    assert!(r.contains("2.0.0"));
    assert!(!r.contains("1.5.0"));
}

#[test]
fn parse_version_range_auto_detect() {
    assert!(matches!(
        parse_version_range("[1.0,)").unwrap(),
        VersionRange::Interval(_)
    ));
    assert!(matches!(
        parse_version_range(">=1.0").unwrap(),
        VersionRange::Interval(_)
    ));
    assert_eq!(parse_version_range("*").unwrap(), VersionRange::Any);
    // 裸版本号走"这一档"：1.0.0 也匹配 1.0.0.1，但不吃 1.0.1
    let r = parse_version_range("1.0.0").unwrap();
    assert!(r.contains("1.0.0"));
    assert!(r.contains("1.0.0.1"));
    assert!(!r.contains("1.0.1"));
}

/// 裸版本号按"这一档"匹配：`1.21` 要能吃下 `1.21.1`
#[test]
fn bare_semver_matches_next_patch() {
    let r = parse_version_range_with("1.21", RangeSyntax::Semver).unwrap();
    assert!(r.contains("1.21"));
    assert!(r.contains("1.21.1"));
    assert!(!r.contains("1.22"));

    // 显式精确匹配不受影响
    assert_eq!(
        parse_version_range_with("=1.21", RangeSyntax::Semver).unwrap(),
        VersionRange::Exact("1.21".into())
    );
}

