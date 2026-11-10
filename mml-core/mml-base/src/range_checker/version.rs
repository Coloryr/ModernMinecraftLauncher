//! 版本号记号切分与比较

use super::*;

/// 版本号切出来的记号，数字段与字符串段分开比较
#[derive(Debug, Clone, PartialEq)]
enum Token {
    /// 纯数字段
    Num(u64),
    /// 非数字段，已转小写
    Str(String),
}

/// 把版本号切成可比较的记号，按 `.` `-` `_` 分段，纯数字段转数字
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

/// 比较两个记号的大小，数字段排在字符串段前面
fn compare_token(a: &Token, b: &Token) -> Ordering {
    match (a, b) {
        (Token::Num(x), Token::Num(y)) => x.cmp(y),
        (Token::Num(_), Token::Str(_)) => Ordering::Greater,
        (Token::Str(_), Token::Num(_)) => Ordering::Less,
        (Token::Str(x), Token::Str(y)) => x.cmp(y),
    }
}

/// 简化版 Maven 版本比较。
///
/// - `a`: 左操作数版本串
/// - `b`: 右操作数版本串
///
/// # 返回值
///
/// `a` 相对 `b` 的先后，逐段比较；`a` 小返回 `Ordering::Less`
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

/// 是不是"干净的版本号"：去掉构建元数据后全是点分数字
pub(crate) fn is_plain_version(version: &str) -> bool {
    let core = version.split('+').next().unwrap_or(version);
    !core.is_empty() && core.split('.').all(|part| part.parse::<u64>().is_ok())
}
