use mml_base::tools::{check_is_not_number, check_is_word, get_string};

/// 校验 check_is_not_number 对纯数字串与非数字串的判断
#[test]
fn check_number() {
    let input = "123";

    let input1 = "123abc";

    assert!(!check_is_not_number(input));
    assert!(check_is_not_number(input1));
}

/// 校验 check_is_word 对英文数字串与非 ASCII 串的判断
#[test]
fn check_word() {
    let input = "123";
    let input1 = "123abc";
    let input2 = "123abc测试";

    assert!(check_is_word(input));
    assert!(check_is_word(input1));
    assert!(!check_is_word(input2));
}

/// 校验 get_string 用起止标记截取中间内容，含中文场景
#[test]
fn string() {
    let s1 = get_string("abcXYZdef", "abc", "def");
    assert_eq!("XYZ", s1);

    let s2 = get_string("你好世界abc", "你好", "abc");
    assert_eq!("世界", s2);
}
