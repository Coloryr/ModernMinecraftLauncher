//! Maven 写法的版本范围解析，即方括号/圆括号区间

use super::*;

/// 解析 Maven 风格的版本范围字符串。
///
/// 支持的写法：
/// - 区间：`[1.0.0,2.0.0)`、`(,1.0.0)`、`[7.0.0,)`
/// - 精确版本：`[1.0.0]`
/// - 多段：`[1.0],[2.0]` 是两段精确版本的并集；`[1.21,1.21.1,1.21.2]` 是作者
///   把允许的版本逐个列了出来，同样按并集处理
///
/// 解析失败返回 `Err`，调用方按"不限版本"处理。
///
/// - `s`: Maven 版本范围串，前后空白会被去掉
///
/// # 返回值
///
/// 解析后的范围；空串返回 [`VersionRange::Any`]，格式错误返回 `ErrorType::InvalidOperation`
pub fn parse_maven_version_range(s: &str) -> CoreResult<VersionRange> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(VersionRange::Any);
    }
    if !s.starts_with('[') && !s.starts_with('(') {
        return Ok(VersionRange::Exact(s.to_string()));
    }

    let mut ranges: Vec<VersionRange> = Vec::new();
    let mut rest = s;
    while !rest.is_empty() {
        let bytes = rest.as_bytes();
        if bytes[0] != b'[' && bytes[0] != b'(' {
            return Err(ErrorType::InvalidOperation);
        }
        let close = rest
            .find(|c| c == ']' || c == ')')
            .ok_or(ErrorType::InvalidOperation)?;
        let parts: Vec<&str> = rest[1..close].split(',').map(str::trim).collect();

        match parts.len() {
            // `[1.0.0]`：Maven 的精确版本写法
            1 => {
                if let Some(version) = bound_of(parts[0]) {
                    ranges.push(VersionRange::Exact(version));
                }
            }
            // `[1.0.0,2.0.0)`：一个区间
            2 => ranges.push(VersionRange::Interval(Interval {
                lower: bound_of(parts[0]),
                lower_inclusive: bytes[0] == b'[',
                upper: bound_of(parts[1]),
                upper_inclusive: bytes[close] == b']',
            })),
            // 三个及以上：逐个列的允许版本，按并集处理
            _ => ranges.extend(
                parts
                    .iter()
                    .filter_map(|part| bound_of(part))
                    .map(VersionRange::Exact),
            ),
        }

        rest = rest[close + 1..].trim_start();
        if rest.is_empty() {
            break;
        }
        rest = rest
            .strip_prefix(',')
            .ok_or(ErrorType::InvalidOperation)?
            .trim_start();
    }

    match ranges.len() {
        0 => Ok(VersionRange::Any),
        1 => Ok(ranges.pop().unwrap()),
        _ => Ok(VersionRange::Union(ranges)),
    }
}

/// 区间的一段：空串表示这一侧不限
fn bound_of(part: &str) -> Option<String> {
    if part.is_empty() {
        None
    } else {
        Some(part.to_string())
    }
}
