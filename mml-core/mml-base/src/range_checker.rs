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
    // SemVer 的构建元数据不参与版本比较：`9.5.1+1.21.1` 就是 `9.5.1`。
    // 不切掉 `+` 后面那段会解析出 "1+1" 这种非数字记号，比 `[9.5.1,)` 时判成"小了"
    let s = s.split('+').next().unwrap_or(s);
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
        // 边界不是干净的版本号时比不准：`[1.21,1.21z)`、`[1.21-3.1.0c,)` 这类写法
        // 各家实现给出的结论都不一样，而上游加载器是放行的 —— 这里也一律当"符合"，
        // 免得报出一堆没有意义的版本不符
        if self.lower.as_deref().is_some_and(|b| !is_plain_version(b))
            || self.upper.as_deref().is_some_and(|b| !is_plain_version(b))
        {
            return true;
        }

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
/// 支持的写法：
/// - 区间：`[1.0.0,2.0.0)`、`(,1.0.0)`、`[7.0.0,)`
/// - 精确版本：`[1.0.0]`
/// - 多段：`[1.0],[2.0]` 是两段精确版本的并集；`[1.21,1.21.1,1.21.2]` 是作者
///   把允许的版本逐个列了出来，同样按并集处理
///
/// 解析失败返回 `Err`，调用方按"不限版本"处理。
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

/// 是不是"干净的版本号"：去掉构建元数据后全是点分数字
fn is_plain_version(version: &str) -> bool {
    let core = version.split('+').next().unwrap_or(version);
    !core.is_empty()
        && core
            .split('.')
            .all(|part| part.parse::<u64>().is_ok())
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
    // 裸版本号（没有任何运算符）按"这一档版本"处理：`1.21` 也匹配 `1.21.1`。
    // 要严格精确匹配请写 `=1.21`，那会走下面那条 Exact 分支
    if is_bare_token(tok)
        && let Some(range) = prefix_range(tok)
    {
        return Ok(range);
    }

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

/// 是不是裸版本号：没有任何运算符前缀
fn is_bare_token(tok: &str) -> bool {
    const OPS: [&str; 8] = [">=", "<=", ">", "<", "=", "~", "^", "*"];
    !OPS.iter().any(|op| tok.starts_with(op))
}

/// 裸版本号对应的区间：`[版本, 下一档)`
///
/// `1.21` → `[1.21,1.22)`、`1.21.1` → `[1.21.1,1.21.2)` —— 装补丁版也算符合。
/// 只认纯数字点分的版本；带字母的（`1.0.0-beta`）返回 `None`，交给精确匹配那条路，
/// 免得"下一档"算错。
fn prefix_range(version: &str) -> Option<VersionRange> {
    let mut parts: Vec<String> = version.split('.').map(str::to_string).collect();
    if parts.iter().any(|part| part.parse::<u64>().is_err()) {
        return None;
    }
    let last = parts.last_mut()?;
    *last = (last.parse::<u64>().ok()? + 1).to_string();

    Some(VersionRange::Interval(Interval {
        lower: Some(version.to_string()),
        lower_inclusive: true,
        upper: Some(parts.join(".")),
        upper_inclusive: false,
    }))
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

/// 版本范围字符串的写法
///
/// 两种写法对**裸版本号**（没有运算符、没有方括号）的解释不同，解析入口因此要分开：
/// Maven 把它当"推荐版本"、不限制任何版本，semver 则按精确匹配处理。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeSyntax {
    /// Maven：`[1.0,2.0)`；裸版本不限制（Forge / NeoForge 的 `versionRange`、1.12.2 的 `@版本`）
    Maven,
    /// semver：`>=1.0`、`~1.2`；裸版本精确匹配（Fabric / Quilt）
    Semver,
}

/// 按指定写法解析版本范围
///
/// - `s`: 范围字符串；空串报错，`*` 是不限版本
/// - `syntax`: 见 [`RangeSyntax`]
///
/// # 返回值
///
/// 见 [`VersionRange`]
pub fn parse_version_range_with(s: &str, syntax: RangeSyntax) -> CoreResult<VersionRange> {
    let s = s.trim();
    if s.is_empty() {
        return Err(ErrorType::ArgEmpty(ArgEmptyData::Name));
    }
    if s == "*" {
        return Ok(VersionRange::Any);
    }
    match syntax {
        RangeSyntax::Maven => {
            if s.starts_with('[') || s.starts_with('(') {
                parse_maven_version_range(s)
            } else {
                // 裸版本号在 Maven 里是"推荐版本"，不限制任何版本；当成精确匹配会把
                // 装了更新版本的实例全报成"版本不符"
                Ok(VersionRange::Any)
            }
        }
        // 方括号是 Maven 独有的写法：出现在 semver 那边也按 Maven 解析，
        // 别把一个 `[1.21,1.22)` 当成"带方括号的版本号"
        RangeSyntax::Semver => {
            if s.starts_with('[') || s.starts_with('(') {
                parse_maven_version_range(s)
            } else {
                parse_fabric_version_range(s)
            }
        }
    }
}

/// 自动识别语法并解析为 [`VersionRange`]。
///
/// - 以 `[` 或 `(` 开头 → Maven 语法
/// - 其他 → semver 语法（裸版本号会变成 `Exact`）
///
/// 裸版本号两种写法都有，**知道来源时请用 [`parse_version_range_with`]**，别依赖这里猜。
pub fn parse_version_range(s: &str) -> CoreResult<VersionRange> {
    let s = s.trim();
    if s.starts_with('[') || s.starts_with('(') {
        parse_version_range_with(s, RangeSyntax::Maven)
    } else {
        parse_version_range_with(s, RangeSyntax::Semver)
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
        assert!(!parse_maven_version_range("[1.21]").unwrap().contains("1.21.1"));
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
}
