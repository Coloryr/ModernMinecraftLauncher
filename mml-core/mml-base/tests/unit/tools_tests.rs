use super::*;

/// 检查 check_is_not_number 的各种输入
#[test]
fn test_check_is_not_number() {
    // 空串 / 空白串视为含非数字
    assert!(check_is_not_number(""));
    assert!(check_is_not_number("   "));
    // 纯数字，无任何空白
    assert!(!check_is_not_number("123"));
    // 注意现状：trim 仅用于空串判断，首尾空白本身按非数字字符处理
    assert!(check_is_not_number(" 123 "));
    // 含非数字字符
    assert!(check_is_not_number("12a3"));
    assert!(check_is_not_number("1.5"));
    assert!(check_is_not_number("-1"));
}

/// 检查 check_is_word 的各种输入
#[test]
fn test_check_is_word() {
    assert!(check_is_word("abc123"));
    assert!(check_is_word("ABC"));
    assert!(check_is_word("123"));
    assert!(!check_is_word(""));
    assert!(!check_is_word("a-b"));
    assert!(!check_is_word("中文"));
    assert!(!check_is_word("a b"));
}

/// get_string 截取 start 与 end 之间的内容
#[test]
fn test_get_string() {
    assert_eq!(get_string("hello world", "l", "o"), "l");
    assert_eq!(get_string("a=1;b=2;", "=", ";"), "1");
    // start 不存在时返回原串
    assert_eq!(get_string("abc", "x", "y"), "abc");
    // end 不存在时返回原串
    assert_eq!(get_string("abc", "a", "z"), "abc");
}

/// build_vec_string 用平台换行符拼接
#[test]
fn test_build_vec_string() {
    let ending = mml_names::get_line_ending();
    let vec = vec!["a".to_string(), "b".to_string()];
    assert_eq!(build_vec_string(&vec), format!("a{ending}b{ending}"));
    // 空列表
    assert_eq!(build_vec_string(&Vec::new()), "");
}

/// get_path_part 拆分目录与文件名
#[test]
fn test_get_path_part() {
    let part = get_path_part("home/user/text.txt");
    assert_eq!(part.parent.replace('\\', "/"), "home/user");
    assert_eq!(part.file, "text.txt");

    // 只有文件名时父目录为空
    let part = get_path_part("text.txt");
    assert_eq!(part.parent, "");
    assert_eq!(part.file, "text.txt");
}

/// arg_parse 基本分词
#[test]
fn test_arg_parse_simple() {
    assert_eq!(arg_parse("a b c"), vec!["a", "b", "c"]);
    // 多余空白
    assert_eq!(arg_parse("  a   b  "), vec!["a", "b"]);
    // 空串
    assert!(arg_parse("").is_empty());
    assert!(arg_parse("   ").is_empty());
}

/// arg_parse 引号内空白保留
#[test]
fn test_arg_parse_quote() {
    assert_eq!(arg_parse("a \"b c\" d"), vec!["a", "b c", "d"]);
    // 引号在参数中间
    assert_eq!(arg_parse("pre\"x y\"post"), vec!["prex ypost"]);
}

/// arg_parse 转义
#[test]
fn test_arg_parse_escape() {
    // 现状记录，疑似 bug：转义字符后的字符虽被追加，但计数被清零，
    // 若转义后紧跟参数结尾，整个参数会被丢弃
    assert_eq!(arg_parse(r"a\b"), Vec::<String>::new());
    // 转义后还有后续字符时参数保留，内容为 a + 反斜杠 + bc
    assert_eq!(arg_parse(r"a\bc"), vec![r"a\bc"]);
    // 引号前的反斜杠转义引号
    assert_eq!(arg_parse("a\\\"b"), vec!["a\"b"]);
}

