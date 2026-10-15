//! 范围检查器
//! 支持maven和fabric的范围检查
use std::cmp::Ordering;
use std::fmt;

use mml_names::i18_items::error_type::{ArgEmptyData, CoreResult, ErrorType};

use crate::serialize_tools::MiniJsonObj;

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Num(u64),
    Str(String),
}

fn tokenize(s: &str) -> Vec<Token> {
    s.split(|c| c == '.' || c == '-' || c == '_')
        .filter(|p| !p.is_empty())
        .map(|p| {
            if let Ok(n) = p.parse::<u64>() {
                Token::Num(n)
            } else {
                Token::Str(p.to_ascii_lowercase())
            }
        })
        .collect()
}

fn compare_token(a: &Token, b: &Token) -> Ordering {
    match (a, b) {
        (Token::Num(x), Token::Num(y)) => x.cmp(y),
        (Token::Num(_), Token::Str(_)) => Ordering::Greater,
        (Token::Str(_), Token::Num(_)) => Ordering::Less,
        (Token::Str(x), Token::Str(y)) => x.cmp(y),
    }
}

/// 简化版 Maven 版本比较。
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let ta = tokenize(a);
    let tb = tokenize(b);
    let len = ta.len().max(tb.len());
    for i in 0..len {
        let ord = match (ta.get(i), tb.get(i)) {
            (None, None) => Ordering::Equal,
            (None, Some(t)) => match t {
                Token::Num(0) => Ordering::Equal,
                Token::Num(_) => Ordering::Less,
                Token::Str(_) => Ordering::Greater,
            },
            (Some(t), None) => match t {
                Token::Num(0) => Ordering::Equal,
                Token::Num(_) => Ordering::Greater,
                Token::Str(_) => Ordering::Less,
            },
            (Some(x), Some(y)) => compare_token(x, y),
        };
        if ord != Ordering::Equal {
            return ord;
        }
    }
    Ordering::Equal
}

#[derive(Debug, Clone, PartialEq)]
pub enum VersionRange {
    /// 任意版本（`*`）
    Any,
    /// 精确版本（`1.0.0`）
    Exact(String),
    /// 单区间（`[1.0.0,2.0.0)`）
    Interval(Interval),
    /// 多区间联合（`1.0 || 2.0` 或 `(,1.0),[1.1,)`）
    Union(Vec<VersionRange>),
    /// AND：所有子条件都必须满足（新增，用于 JSON 数组）
    All(Vec<VersionRange>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Interval {
    pub lower: Option<String>,
    pub lower_inclusive: bool,
    pub upper: Option<String>,
    pub upper_inclusive: bool,
}

impl fmt::Display for VersionRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VersionRange::Any => write!(f, "*"),
            VersionRange::Exact(v) => write!(f, "{}", v),
            VersionRange::Interval(i) => write!(f, "{}", i),
            VersionRange::Union(rs) => {
                let parts: Vec<String> = rs.iter().map(|r| r.to_string()).collect();
                write!(f, "{}", parts.join(" || "))
            }
            VersionRange::All(rs) => {
                let parts: Vec<String> = rs.iter().map(|r| r.to_string()).collect();
                write!(f, "{}", parts.join(" && "))
            }
        }
    }
}

impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lb = if self.lower_inclusive { '[' } else { '(' };
        let ub = if self.upper_inclusive { ']' } else { ')' };
        let lower = self.lower.as_deref().unwrap_or("");
        let upper = self.upper.as_deref().unwrap_or("");
        write!(f, "{}{},{}{}", lb, lower, upper, ub)
    }
}

impl Interval {
    pub fn contains(&self, version: &str) -> bool {
        if let Some(lower) = &self.lower {
            let ord = compare_versions(version, lower);
            let ok = if self.lower_inclusive {
                ord != Ordering::Less
            } else {
                ord == Ordering::Greater
            };
            if !ok {
                return false;
            }
        }
        if let Some(upper) = &self.upper {
            let ord = compare_versions(version, upper);
            let ok = if self.upper_inclusive {
                ord != Ordering::Greater
            } else {
                ord == Ordering::Less
            };
            if !ok {
                return false;
            }
        }
        true
    }
}

impl VersionRange {
    pub fn contains(&self, version: &str) -> bool {
        match self {
            VersionRange::Any => true,
            VersionRange::Exact(v) => compare_versions(version, v) == Ordering::Equal,
            VersionRange::Interval(i) => i.contains(version),
            VersionRange::Union(rs) => rs.iter().any(|r| r.contains(version)),
            VersionRange::All(rs) => rs.iter().all(|r| r.contains(version)),
        }
    }
}

/// 解析 Maven 风格的版本范围字符串。
///
/// 例：`[7.0.0,)`、`[1.0.0,2.0.0)`、`(,1.0.0),[1.1.0,)`、`1.0.0`
pub fn parse_maven_version_range(s: &str) -> CoreResult<VersionRange> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(VersionRange::Any);
    }
    if !s.starts_with('[') && !s.starts_with('(') {
        return Ok(VersionRange::Exact(s.to_string()));
    }

    let mut ranges = Vec::new();
    let mut rest = s;
    while !rest.is_empty() {
        let (interval, remaining) = parse_maven_interval(rest)?;
        ranges.push(VersionRange::Interval(interval));
        rest = remaining.trim_start();
        if rest.is_empty() {
            break;
        }
        if let Some(r) = rest.strip_prefix(',') {
            rest = r.trim_start();
        } else {
            return Err(ErrorType::InvalidOperation);
        }
    }
    if ranges.len() == 1 {
        Ok(ranges.pop().unwrap())
    } else {
        Ok(VersionRange::Union(ranges))
    }
}

/// 解析 JSON 字符串数组 `[ ">=1.18.1", "<1.19.3" ]`，每个元素单独解析后组合成 AND。
pub fn parse_json_array_range(s: &MiniJsonObj) -> CoreResult<VersionRange> {
    if !s.is_list() {
        return Err(ErrorType::InvalidOperation);
    }
    let list = s.as_list().unwrap_or_default();
    if list.is_empty() {
        return Ok(VersionRange::Any);
    }
    let ranges: Result<Vec<_>, _> = list
        .iter()
        .map(|it| parse_version_range(&it.as_string().unwrap_or_default()))
        .collect();
    let ranges = ranges?;
    if ranges.len() == 1 {
        Ok(ranges.into_iter().next().unwrap())
    } else {
        Ok(VersionRange::All(ranges))
    }
}

fn parse_maven_interval(s: &str) -> CoreResult<(Interval, &str)> {
    let s = s.trim_start();
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return Err(ErrorType::ArgEmpty(ArgEmptyData::Name));
    }
    let lower_inclusive = match bytes[0] {
        b'[' => true,
        b'(' => false,
        _ => return Err(ErrorType::InvalidOperation),
    };

    let comma_pos = s.find(',').ok_or_else(|| ErrorType::InvalidOperation)?;
    let lower_str = s[1..comma_pos].trim();
    let lower = if lower_str.is_empty() {
        None
    } else {
        Some(lower_str.to_string())
    };

    let after_comma = &s[comma_pos + 1..];
    let end_pos = after_comma
        .find(|c| c == ']' || c == ')')
        .ok_or_else(|| ErrorType::InvalidOperation)?;
    let upper_str = after_comma[..end_pos].trim();
    let upper = if upper_str.is_empty() {
        None
    } else {
        Some(upper_str.to_string())
    };
    let upper_inclusive = after_comma.as_bytes()[end_pos] == b']';

    let consumed = comma_pos + 1 + end_pos + 1;
    let remaining = &s[consumed..];

    Ok((
        Interval {
            lower,
            lower_inclusive,
            upper,
            upper_inclusive,
        },
        remaining,
    ))
}

/// 解析 Fabric 风格的版本范围字符串。
///
/// 例：`*`、`>=0.19.5`、`~1.2.3`、`^1.2.3`、`>=1.0 <2.0`、`1.0 || 2.0`
pub fn parse_fabric_version_range(s: &str) -> CoreResult<VersionRange> {
    let s = s.trim();
    if s.is_empty() || s == "*" {
        return Ok(VersionRange::Any);
    }

    let or_parts: Vec<&str> = s.split("||").map(|p| p.trim()).collect();
    if or_parts.len() > 1 {
        let ranges: Result<Vec<_>, _> = or_parts
            .iter()
            .map(|p| parse_fabric_conjunction(p))
            .collect();
        return Ok(VersionRange::Union(ranges?));
    }

    parse_fabric_conjunction(s)
}

fn parse_fabric_conjunction(s: &str) -> CoreResult<VersionRange> {
    let tokens: Vec<&str> = s.split_whitespace().collect();
    if tokens.is_empty() {
        return Ok(VersionRange::Any);
    }
    if tokens.len() == 1 {
        return parse_fabric_single(tokens[0]);
    }

    let mut lower: Option<(String, bool)> = None;
    let mut upper: Option<(String, bool)> = None;

    for tok in tokens {
        let (lo, up) = token_to_bounds(tok)?;
        if let Some((v, incl)) = lo {
            lower = match lower {
                None => Some((v, incl)),
                Some((lv, li)) => match compare_versions(&v, &lv) {
                    Ordering::Greater => Some((v, incl)),
                    Ordering::Equal => Some((v, li && incl)),
                    Ordering::Less => Some((lv, li)),
                },
            };
        }
        if let Some((v, incl)) = up {
            upper = match upper {
                None => Some((v, incl)),
                Some((uv, ui)) => match compare_versions(&v, &uv) {
                    Ordering::Less => Some((v, incl)),
                    Ordering::Equal => Some((v, ui && incl)),
                    Ordering::Greater => Some((uv, ui)),
                },
            };
        }
    }

    Ok(VersionRange::Interval(Interval {
        lower: lower.as_ref().map(|(v, _)| v.clone()),
        lower_inclusive: lower.as_ref().map(|(_, i)| *i).unwrap_or(false),
        upper: upper.as_ref().map(|(v, _)| v.clone()),
        upper_inclusive: upper.as_ref().map(|(_, i)| *i).unwrap_or(false),
    }))
}

fn parse_fabric_single(tok: &str) -> CoreResult<VersionRange> {
    let (lo, up) = token_to_bounds(tok)?;
    match (&lo, &up) {
        (Some((l, true)), Some((u, true))) if compare_versions(l, u) == Ordering::Equal => {
            Ok(VersionRange::Exact(l.clone()))
        }
        _ => Ok(VersionRange::Interval(Interval {
            lower: lo.clone().map(|(v, _)| v),
            lower_inclusive: lo.map(|(_, i)| i).unwrap_or(false),
            upper: up.clone().map(|(v, _)| v),
            upper_inclusive: up.map(|(_, i)| i).unwrap_or(false),
        })),
    }
}

fn token_to_bounds(tok: &str) -> CoreResult<(Option<(String, bool)>, Option<(String, bool)>)> {
    let tok = tok.trim();
    if tok.is_empty() || tok == "*" {
        return Ok((None, None));
    }

    for (prefix, kind) in [
        (">=", "ge"),
        ("<=", "le"),
        (">", "gt"),
        ("<", "lt"),
        ("=", "eq"),
        ("~", "tilde"),
        ("^", "caret"),
    ] {
        if let Some(rest) = tok.strip_prefix(prefix) {
            let v = rest.trim();
            if v.is_empty() || v == "*" {
                return Ok((None, None));
            }
            return match kind {
                "ge" => Ok((Some((v.to_string(), true)), None)),
                "gt" => Ok((Some((v.to_string(), false)), None)),
                "le" => Ok((None, Some((v.to_string(), true)))),
                "lt" => Ok((None, Some((v.to_string(), false)))),
                "eq" => Ok((Some((v.to_string(), true)), Some((v.to_string(), true)))),
                "tilde" => {
                    let (l, u) = expand_tilde(v)?;
                    Ok((Some((l, true)), Some((u, false))))
                }
                "caret" => {
                    let (l, u) = expand_caret(v)?;
                    Ok((Some((l, true)), Some((u, false))))
                }
                _ => unreachable!(),
            };
        }
    }

    Ok((Some((tok.to_string(), true)), Some((tok.to_string(), true))))
}

fn expand_tilde(v: &str) -> CoreResult<(String, String)> {
    let parts: Vec<&str> = v.split('.').collect();
    let major: u64 = parts[0].parse().map_err(|_| ErrorType::InvalidOperation)?;
    let upper = if parts.len() >= 2 {
        let minor: u64 = parts[1].parse().map_err(|_| ErrorType::InvalidOperation)?;
        format!("{}.{}", major, minor + 1)
    } else {
        format!("{}", major + 1)
    };
    Ok((v.to_string(), upper))
}

fn expand_caret(v: &str) -> CoreResult<(String, String)> {
    let parts: Vec<&str> = v.split('.').collect();
    let major: u64 = parts[0].parse().map_err(|_| ErrorType::InvalidOperation)?;
    let upper = if major > 0 {
        format!("{}", major + 1)
    } else if parts.len() >= 2 {
        let minor: u64 = parts[1].parse().map_err(|_| ErrorType::InvalidOperation)?;
        if minor > 0 {
            format!("0.{}", minor + 1)
        } else if parts.len() >= 3 {
            let patch: u64 = parts[2].parse().map_err(|_| ErrorType::InvalidOperation)?;
            format!("0.0.{}", patch + 1)
        } else {
            "0.1".to_string()
        }
    } else {
        "1".to_string()
    };
    Ok((v.to_string(), upper))
}

/// 自动识别语法并解析为 [`VersionRange`]。
///
/// - 以 `[` 或 `(` 开头 → Maven 语法
/// - 其他 → Fabric 语法（裸版本号会变成 `Exact`）
pub fn parse_version_range(s: &str) -> CoreResult<VersionRange> {
    let s = s.trim();
    if s.is_empty() {
        return Err(ErrorType::ArgEmpty(ArgEmptyData::Name));
    }
    if s == "*" {
        return Ok(VersionRange::Any);
    }
    if s.starts_with('[') || s.starts_with('(') {
        parse_maven_version_range(s)
    } else {
        parse_fabric_version_range(s)
    }
}

#[cfg(test)]
mod tests {
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
        assert_eq!(
            parse_version_range("1.0.0").unwrap(),
            VersionRange::Exact("1.0.0".into())
        );
    }
}
