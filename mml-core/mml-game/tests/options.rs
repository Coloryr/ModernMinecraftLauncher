//! `game_options::read_options` 单元测试：解析 OptiFine 风格的 `options.txt` 配置。

use std::io::Cursor;

use mml_game::game_options::{InstanceCfg, read_options, read_options_from_file};

/// 解析 `key:value` 形式的基础键值对
#[test]
fn parse_basic_key_value() {
    let text = "fov:0\nrenderDistance:12\ngamma:0.5\n";
    let data = read_options(Cursor::new(text), None).unwrap();

    assert_eq!(data.len(), 3);
    assert_eq!(data.get("fov").map(String::as_str), Some("0"));
    assert_eq!(data.get("renderDistance").map(String::as_str), Some("12"));
    assert_eq!(data.get("gamma").map(String::as_str), Some("0.5"));
}

/// 自定义分隔符也能解析
#[test]
fn parse_custom_separator() {
    let text = "fov=0\nrenderDistance=12\n";
    let data = read_options(Cursor::new(text), Some('=')).unwrap();

    assert_eq!(data.len(), 2);
    assert_eq!(data.get("fov").map(String::as_str), Some("0"));
    assert_eq!(data.get("renderDistance").map(String::as_str), Some("12"));
}

/// 整行注释、空行与行内注释都被跳过
#[test]
fn skip_comments_and_blank_lines() {
    let text = "# 整行注释\n\n   \nfov:0 # 行内注释\nrenderDistance:12\n";
    let data = read_options(Cursor::new(text), None).unwrap();

    assert_eq!(data.len(), 2);
    assert_eq!(data.get("fov").map(String::as_str), Some("0"));
    assert_eq!(data.get("renderDistance").map(String::as_str), Some("12"));
}

/// 只有键没有值时取空字符串
#[test]
fn key_without_value_gets_empty_string() {
    let text = "noValueKey:\n";
    let data = read_options(Cursor::new(text), None).unwrap();

    assert_eq!(data.len(), 1);
    assert_eq!(data.get("noValueKey").map(String::as_str), Some(""));
}

/// 空输入与只有注释的输入都得到空表
#[test]
fn empty_input_returns_empty_map() {
    let data = read_options(Cursor::new(""), None).unwrap();
    assert!(data.is_empty());

    let data = read_options(Cursor::new("# 只有注释\n"), None).unwrap();
    assert!(data.is_empty());
}

/// 从文件读配置，结果与从 reader 读一致
#[test]
fn read_options_from_file_works() {
    let file = mml_testutil::temp_dir().join(format!("mml-options-test-{}.txt", std::process::id()));
    std::fs::write(&file, "width:1920\nheight:1080\n").unwrap();

    let data: InstanceCfg = read_options_from_file(&file, None).unwrap();
    assert_eq!(data.get("width").map(String::as_str), Some("1920"));
    assert_eq!(data.get("height").map(String::as_str), Some("1080"));

    let _ = std::fs::remove_file(&file);
}
