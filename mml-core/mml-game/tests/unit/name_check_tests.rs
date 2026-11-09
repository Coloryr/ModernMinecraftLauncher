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
