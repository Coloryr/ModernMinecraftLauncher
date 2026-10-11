//! Minecraft 版本号解析模块
//! 支持所有 Minecraft 版本格式：
//! - 旧格式正式版: 1.20.4, 1.7.10
//! - 旧格式快照: 24w13a, 25w08a
//! - 旧格式预发布: 1.20.4-pre1, 1.20.4-pre4
//! - 旧格式 RC: 1.20.4-rc1
//! - 新格式正式版: 26.1, 26.1.1，2026 年起
//! - 新格式快照: 26.1-snapshot-1
//! - 新格式预发布: 26.1-pre-1
//! - 新格式 RC: 26.1-rc-2
//! - 远古版本: a1.0.16, b1.7.3, rd-160052

/// 将 Minecraft 版本号转换为可比较的整数向量，按格式依次尝试各解析分支
///
/// - `version`: 版本号字符串，首尾空白会被忽略
///
/// # 返回值
///
/// 返回可比较的整数向量；所有分支都匹配不上时返回 `None`
pub fn parse_game_version(version: &str) -> Option<Vec<i32>> {
    let v = version.trim();

    // 1. 新格式快照: 26.1-snapshot-11
    if let Some(result) = parse_new_snapshot(v) {
        return Some(result);
    }

    // 2. 新格式预发布: 26.1-pre-1
    if let Some(result) = parse_new_pre_release(v) {
        return Some(result);
    }

    // 3. 新格式 RC: 26.1-rc-2
    if let Some(result) = parse_new_rc(v) {
        return Some(result);
    }

    // 4. 旧格式快照: 24w13a
    if let Some(result) = parse_old_snapshot(v) {
        return Some(result);
    }

    // 5. 旧格式预发布/RC: 1.20.4-pre1, 1.20.4-rc1
    if let Some(result) = parse_old_prerelease(v) {
        return Some(result);
    }

    // 6. 远古版本: a1.0.16, b1.7.3
    if let Some(result) = parse_ancient_version(v) {
        return Some(result);
    }

    // 7. 新格式正式版: 26.1, 26.1.1，年份 >= 25
    if let Some(result) = parse_new_release(v) {
        return Some(result);
    }

    // 8. 旧格式正式版: 1.20.4, 1.20, 1.20.0
    if let Some(result) = parse_old_release(v) {
        return Some(result);
    }

    None
}

/// 解析旧格式正式版并标准化为三段式 `[major, minor, patch]`
///
/// 例如 `1.20` 得到 `[1, 20, 0]`，`1.7.10` 得到 `[1, 7, 10]`
///
/// - `s`: 版本号字符串
///
/// # 返回值
///
/// 首段不是 1 或完全无法解析时返回 `None`
fn parse_old_release(s: &str) -> Option<Vec<i32>> {
    let parts: Vec<i32> = s.split('.').filter_map(|p| p.parse().ok()).collect();

    if parts.is_empty() {
        return None;
    }

    // 旧格式以 1 开头
    if parts[0] != 1 {
        return None;
    }

    // 标准化为三段式: [major, minor, patch]
    let major = parts[0];
    let minor = if parts.len() >= 2 { parts[1] } else { 0 };
    let patch = if parts.len() >= 3 { parts[2] } else { 0 };

    Some(vec![major, minor, patch])
}

/// 解析新格式正式版，输出 `[100 + year, release_num, hotfix_num]`
///
/// - `s`: 版本号字符串
///
/// # 返回值
///
/// 不足两段或年份小于 25 时返回 `None`
fn parse_new_release(s: &str) -> Option<Vec<i32>> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() < 2 {
        return None;
    }

    let year = parts[0].parse::<i32>().ok()?;
    if year < 25 {
        return None;
    }

    let release_num = parts[1].parse::<i32>().ok()?;
    let hotfix = if parts.len() >= 3 {
        parts[2].parse::<i32>().ok()?
    } else {
        0
    };

    Some(vec![100 + year, release_num, hotfix])
}

/// 解析新格式快照，输出 `[10, 100 + year, release_num, snapshot_num]`
///
/// - `s`: 版本号字符串
///
/// # 返回值
///
/// 形如 `26.1-snapshot-11` 才解析成功，其余返回 `None`
fn parse_new_snapshot(s: &str) -> Option<Vec<i32>> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    if parts[1] != "snapshot" {
        return None;
    }

    let base_parts: Vec<&str> = parts[0].split('.').collect();
    if base_parts.len() < 2 {
        return None;
    }

    let year = base_parts[0].parse::<i32>().ok()?;
    if year < 25 {
        return None;
    }

    let release_num = base_parts[1].parse::<i32>().ok()?;
    let snapshot_num = parts[2].parse::<i32>().ok()?;

    // 快照类型 = 10，排在预发布 30 与正式版 100+ 之前
    Some(vec![10, 100 + year, release_num, snapshot_num])
}

/// 解析新格式预发布，输出 `[30, 100 + year, release_num, pre_num]`
///
/// - `s`: 版本号字符串
///
/// # 返回值
///
/// 形如 `26.1-pre-1` 才解析成功，其余返回 `None`
fn parse_new_pre_release(s: &str) -> Option<Vec<i32>> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    if parts[1] != "pre" {
        return None;
    }

    let base_parts: Vec<&str> = parts[0].split('.').collect();
    if base_parts.len() < 2 {
        return None;
    }

    let year = base_parts[0].parse::<i32>().ok()?;
    if year < 25 {
        return None;
    }

    let release_num = base_parts[1].parse::<i32>().ok()?;
    let pre_num = parts[2].parse::<i32>().ok()?;

    // 预发布类型 = 30，排序为快照 10 < 预发布 30 < 正式版 100+
    Some(vec![30, 100 + year, release_num, pre_num])
}

/// 解析新格式 RC，输出 `[20, 100 + year, release_num, rc_num]`
///
/// - `s`: 版本号字符串
///
/// # 返回值
///
/// 形如 `26.1-rc-2` 才解析成功，其余返回 `None`
fn parse_new_rc(s: &str) -> Option<Vec<i32>> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    if parts[1] != "rc" {
        return None;
    }

    let base_parts: Vec<&str> = parts[0].split('.').collect();
    if base_parts.len() < 2 {
        return None;
    }

    let year = base_parts[0].parse::<i32>().ok()?;
    if year < 25 {
        return None;
    }

    let release_num = base_parts[1].parse::<i32>().ok()?;
    let rc_num = parts[2].parse::<i32>().ok()?;

    // RC 类型 = 20，排序为快照 10 < RC 20 < 预发布 30 < 正式版 100+
    Some(vec![20, 100 + year, release_num, rc_num])
}

/// 解析旧格式快照，输出 `[-10, 2000 + year, week, sub]`
///
/// - `s`: 版本号字符串
///
/// # 返回值
///
/// 形如 `24w13a` 才解析成功，其余返回 `None`
fn parse_old_snapshot(s: &str) -> Option<Vec<i32>> {
    // 格式: YYwWWx
    let chars: Vec<char> = s.chars().collect();
    if chars.len() < 5 {
        return None;
    }
    if chars[2] != 'w' {
        return None;
    }

    let year_str: String = chars[0..2].iter().collect();
    let week_str: String = chars[3..5].iter().collect();

    let year: i32 = year_str.parse().ok()?;
    let week: i32 = week_str.parse().ok()?;
    let sub = if chars.len() > 5 {
        (chars[5] as i32) - ('a' as i32) + 1
    } else {
        0
    };

    // 旧快照类型 = -10，排在正式版 1.x 之前
    Some(vec![-10, 2000 + year, week, sub])
}

/// 解析旧格式预发布与 RC，输出 `[tag, major, minor, patch, num]`
///
/// - `s`: 版本号字符串
///
/// # 返回值
///
/// 不含 `-pre` / `-rc` 后缀时返回 `None`
fn parse_old_prerelease(s: &str) -> Option<Vec<i32>> {
    let (base_part, tag, num) = if let Some(idx) = s.find("-pre") {
        let base = &s[..idx];
        let num_str = &s[idx + 4..];
        let num = num_str.parse::<i32>().ok()?;
        (base, -5, num)
    } else if let Some(idx) = s.find("-rc") {
        let base = &s[..idx];
        let num_str = &s[idx + 3..];
        let num = num_str.parse::<i32>().ok()?;
        (base, -3, num)
    } else {
        return None;
    };

    // 解析基础版本并标准化为三段式
    let base_parts: Vec<i32> = base_part
        .split('.')
        .filter_map(|p| p.parse().ok())
        .collect();
    if base_parts.is_empty() {
        return None;
    }

    let major = base_parts[0];
    let minor = if base_parts.len() >= 2 {
        base_parts[1]
    } else {
        0
    };
    let patch = if base_parts.len() >= 3 {
        base_parts[2]
    } else {
        0
    };

    Some(vec![tag, major, minor, patch, num])
}

/// 解析远古版本，前缀决定排序标记
///
/// - `s`: 版本号字符串
///
/// # 返回值
///
/// `a` 前缀得 `-30`，`b` 前缀得 `-20`，`rd-` / `inf-` 得 `-40`；其余返回 `None`
fn parse_ancient_version(s: &str) -> Option<Vec<i32>> {
    // Alpha: a1.0.16
    if s.starts_with('a') {
        let without_prefix = &s[1..];
        let parts: Vec<i32> = without_prefix
            .split('.')
            .filter_map(|p| p.parse().ok())
            .collect();
        if !parts.is_empty() {
            let mut result = vec![-30]; // Alpha 标记
            result.extend(parts);
            return Some(result);
        }
    }

    // Beta: b1.7.3
    if s.starts_with('b') {
        let without_prefix = &s[1..];
        let parts: Vec<i32> = without_prefix
            .split('.')
            .filter_map(|p| p.parse().ok())
            .collect();
        if !parts.is_empty() {
            let mut result = vec![-20]; // Beta 标记
            result.extend(parts);
            return Some(result);
        }
    }

    // rd-160052 等远古版本
    if s.starts_with("rd-") || s.starts_with("inf-") {
        let parts: Vec<i32> = s.split('-').filter_map(|p| p.parse().ok()).collect();
        if !parts.is_empty() {
            return Some(vec![-40, parts[0]]);
        }
    }

    None
}

/// 判断第一个版本号是否大于第二个
///
/// - `v1`: 版本号字符串
/// - `v2`: 版本号字符串
///
/// # 返回值
///
/// `v1` 大于 `v2` 时为 `true`；任一无法解析时返回 `false`
pub fn is_game_version_greater(v1: &str, v2: &str) -> bool {
    let parts1 = parse_game_version(v1);
    let parts2 = parse_game_version(v2);

    match (parts1, parts2) {
        (Some(p1), Some(p2)) => p1 > p2,
        _ => false,
    }
}

/// 判断两个版本号是否相等
///
/// - `v1`: 版本号字符串
/// - `v2`: 版本号字符串
///
/// # 返回值
///
/// 任一无法解析时返回 `false`
pub fn is_game_version_equal(v1: &str, v2: &str) -> bool {
    let parts1 = parse_game_version(v1);
    let parts2 = parse_game_version(v2);

    match (parts1, parts2) {
        (Some(p1), Some(p2)) => p1 == p2,
        _ => false,
    }
}

/// 判断第一个版本号是否大于等于第二个
///
/// - `v1`: 版本号字符串
/// - `v2`: 版本号字符串
///
/// # 返回值
///
/// `v1` 不小于 `v2` 时为 `true`；任一无法解析时返回 `false`
pub fn is_game_version_greater_equal(v1: &str, v2: &str) -> bool {
    let parts1 = parse_game_version(v1);
    let parts2 = parse_game_version(v2);

    match (parts1, parts2) {
        (Some(p1), Some(p2)) => p1 >= p2,
        _ => false,
    }
}

/// 判断是否为 1.17 及以上版本
///
/// - `version`: 版本号字符串
///
/// # 返回值
///
/// 无法解析时返回 `false`
pub fn is_game_version_117(version: &str) -> bool {
    is_game_version_greater_equal(version, "1.17")
}

/// 判断是否为 1.20 及以上版本
///
/// - `version`: 版本号字符串
///
/// # 返回值
///
/// 无法解析时返回 `false`
pub fn is_game_version_120(version: &str) -> bool {
    is_game_version_greater_equal(version, "1.20")
}

/// 判断是否为 1.20.2 及以上版本
///
/// - `version`: 版本号字符串
///
/// # 返回值
///
/// 无法解析时返回 `false`
pub fn is_game_version_1202(version: &str) -> bool {
    is_game_version_greater_equal(version, "1.20.2")
}

#[cfg(test)]
mod tests {
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
}
