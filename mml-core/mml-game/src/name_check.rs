//! 名字校验：实例名会落成目录名、分组名会进分组表，创建前统一挡一次
//!
//! 只校验**用户输入**。从整合包 / 目录派生出来的名字不走这里，仍由
//! `path_helper::replace_path_name` 兜底 —— 那条路要能把任意包名装进来，
//! 不该被长度或保留名卡住。

use mml_names::i18_items::error_type::{CoreResult, ErrorType, NameInvalidData, NameInvalidReason};

/// 名字最多多少个字符
pub const MAX_NAME_CHARS: usize = 64;

/// Windows 保留设备名，不能作为文件 / 目录名
const RESERVED_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// 名字里不允许出现的字符：Windows 文件名非法字符与控制字符
///
/// - `c`: 待检查的字符
///
/// # 返回值
///
/// 不允许出现返回 `true`
fn is_illegal_char(c: char) -> bool {
    c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
}

/// 检查名字是否可用
///
/// 空名、超过 [`MAX_NAME_CHARS`]、含非法字符、以点结尾、Windows 保留名都会拒绝；
/// 首尾空白不算错误，调用方按 `trim` 后的名字使用。
///
/// - `name`: 用户输入的名字
///
/// # 返回值
///
/// 可用返回 `Ok(())`；不可用返回 [`ErrorType::NameInvalid`]，带具体原因
pub fn check_name(name: &str) -> CoreResult<()> {
    let trimmed = name.trim();
    let invalid = |reason| {
        ErrorType::NameInvalid(NameInvalidData {
            name: name.to_string(),
            reason,
        })
    };

    if trimmed.is_empty() {
        return Err(invalid(NameInvalidReason::Empty));
    }

    let len = trimmed.chars().count();
    if len > MAX_NAME_CHARS {
        return Err(invalid(NameInvalidReason::TooLong {
            len,
            max: MAX_NAME_CHARS,
        }));
    }

    if let Some(c) = trimmed.chars().find(|c| is_illegal_char(*c)) {
        return Err(invalid(NameInvalidReason::IllegalChar(c)));
    }

    // Windows 会吃掉结尾的点，`存档.` 与 `存档` 会落到同一个目录
    if trimmed.ends_with('.') {
        return Err(invalid(NameInvalidReason::TrailingDot));
    }

    // 保留名按第一个点之前的部分判断：`CON` 与 `CON.txt` 都不能用
    let stem = trimmed.split('.').next().unwrap_or(trimmed);
    if RESERVED_NAMES
        .iter()
        .any(|item| item.eq_ignore_ascii_case(stem))
    {
        return Err(invalid(NameInvalidReason::Reserved));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 正常名字，含中英文、点与首尾空白
    #[test]
    fn accepts_normal_names() {
        for name in [
            "我的实例 1.21",
            "26.2-Fabric-0.19.3",
            "a.b",
            "  padded  ",
            "草稿_v2",
        ] {
            check_name(name).unwrap_or_else(|err| panic!("{name} 应可用：{err}"));
        }
    }

    /// 空名与非法字符：路径分隔符、Windows 通配符、控制字符
    #[test]
    fn rejects_empty_and_illegal_chars() {
        assert!(matches!(reason("   "), NameInvalidReason::Empty));
        for name in [
            "a/b", "a\\b", "a:b", "a*b", "a?b", "a|b", "a<b", "a>b", "a\"b", "a\nb", "a\tb",
        ] {
            assert!(
                matches!(reason(name), NameInvalidReason::IllegalChar(_)),
                "{name} 应因非法字符被拒"
            );
        }
    }

    /// 超长、结尾点、Windows 保留名
    #[test]
    fn rejects_long_trailing_dot_and_reserved() {
        let long = "名".repeat(MAX_NAME_CHARS + 1);
        assert!(matches!(
            reason(&long),
            NameInvalidReason::TooLong {
                len,
                max: MAX_NAME_CHARS
            } if len == MAX_NAME_CHARS + 1
        ));

        assert!(matches!(reason("存档."), NameInvalidReason::TrailingDot));
        for name in ["CON", "con", "NUL.txt", "COM1", "lpt9.log"] {
            assert!(
                matches!(reason(name), NameInvalidReason::Reserved),
                "{name} 应被当成保留名"
            );
        }
    }

    /// 取一个名字被拒的原因；没被拒说明用例写错了
    fn reason(name: &str) -> NameInvalidReason {
        match check_name(name) {
            Err(ErrorType::NameInvalid(data)) => data.reason,
            Ok(()) => panic!("{name} 应被拒绝"),
            Err(err) => panic!("{name} 的错误类型不对：{err}"),
        }
    }
}
