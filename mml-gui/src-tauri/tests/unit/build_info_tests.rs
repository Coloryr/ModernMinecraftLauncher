use super::*;

/// 少写一段的版本号要补齐：`1.0` 不能拼成 `1.0.20261028`
#[test]
fn base_version_is_three_parts() {
    assert_eq!(base_version("1"), "1.0.0");
    assert_eq!(base_version("1.0"), "1.0.0");
    assert_eq!(base_version("1.0.0"), "1.0.0");
    assert_eq!(base_version("2.3.4"), "2.3.4");
}

/// 正式版不带日期，测试版与预发布版带 yyyymmdd
#[test]
fn version_follows_channel() {
    let base = base_version(env!("CARGO_PKG_VERSION"));
    assert_eq!(version_of(BuildChannel::Release), base);

    for channel in [BuildChannel::Debug, BuildChannel::Prerelease] {
        let version = version_of(channel);
        let date = version
            .strip_prefix(&format!("{base}."))
            .unwrap_or_else(|| panic!("版本号应以 {base}. 开头：{version}"));
        assert_eq!(date.len(), 8, "日期应为 yyyymmdd：{date}");
        assert!(
            date.chars().all(|c| c.is_ascii_digit()),
            "日期只能是数字：{date}"
        );
    }
}

