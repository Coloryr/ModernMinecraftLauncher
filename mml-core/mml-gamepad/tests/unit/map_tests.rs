use super::*;

use std::path::Path;

/// 界面上列出的动作名都能被解析
#[test]
fn action_names_parse() {
    let names = action_names();
    assert!(names.len() > 40);
    for name in names {
        assert!(parse_action(&name).is_ok(), "{name}");
    }
}

/// 内置默认映射可编译，关键绑定落在预期位置
#[test]
fn default_map_compiles() {
    let map = GamepadConfig::default().compile().unwrap();
    assert!(!map.buttons.is_empty());
    assert_eq!(map.mouse.deadzone, 0.15);

    let south = button_index(Button::South).unwrap();
    assert!(
        map.buttons
            .iter()
            .any(|rule| rule.index == south && rule.combo.steps.contains(&Action::Key(32)))
    );
}

/// 键名、鼠标键、滚轮都能解析
#[test]
fn parse_actions() {
    assert_eq!(parse_action("W"), Ok(Action::Key(87)));
    assert_eq!(parse_action("space"), Ok(Action::Key(32)));
    assert_eq!(parse_action("F5"), Ok(Action::Key(294)));
    assert_eq!(parse_action("LeftShift"), Ok(Action::Key(340)));
    assert_eq!(parse_action("MouseLeft"), Ok(Action::Mouse(0)));
    assert_eq!(parse_action("MouseRight"), Ok(Action::Mouse(1)));
    assert_eq!(parse_action("ScrollUp"), Ok(Action::Scroll(1)));
    assert_eq!(parse_action("ScrollDown"), Ok(Action::Scroll(-1)));
    assert!(parse_action("Nope").is_err());
    assert!(parse_action("Mouse9").is_err());
}

/// 区间判定不要求起点小于终点
#[test]
fn range_hit() {
    assert!(in_range(-0.5, -1.0, -0.35));
    assert!(!in_range(-0.2, -1.0, -0.35));
    assert!(in_range(-0.5, -0.35, -1.0));
    assert!(!in_range(0.0, 0.35, 1.0));
}

/// 默认配置写出的 JSON 用大驼峰，读回来还能编译
#[test]
fn config_roundtrip() {
    let text = serde_json::to_string_pretty(&GamepadConfig::default()).unwrap();
    assert!(text.contains("\"Selected\""));
    assert!(text.contains("\"Buttons\""));
    assert!(text.contains("\"CursorStick\""));

    let config: GamepadConfig = serde_json::from_str(&text).unwrap();
    assert_eq!(config.buttons.len(), GamepadConfig::default().buttons.len());
    assert!(config.compile().is_ok());
}

/// 修饰键不能和按钮相同
#[test]
fn modifier_cannot_match_button() {
    let mut config = GamepadConfig::default();
    config.buttons[0].modifier = Some(Button::South);
    assert!(config.compile().is_err());
}

/// 修饰键为空时不写进 JSON，设置后能读回来
#[test]
fn modifier_roundtrip() {
    let mut config = GamepadConfig::default();
    config.buttons[0].modifier = Some(Button::LeftTrigger);
    let text = serde_json::to_string_pretty(&config).unwrap();
    assert!(text.contains("\"Modifier\": \"leftTrigger\""));
    let back: GamepadConfig = serde_json::from_str(&text).unwrap();
    assert_eq!(back.buttons[0].modifier, Some(Button::LeftTrigger));

    let plain = GamepadConfig::default();
    let text = serde_json::to_string_pretty(&plain).unwrap();
    assert!(!text.contains("\"Modifier\""));
}

/// 动作写错时整份配置编译失败
#[test]
fn invalid_action_is_rejected() {
    let mut config = GamepadConfig::default();
    config.buttons.push(ButtonRule {
        button: Button::South,
        modifier: None,
        action: String::from("Nope"),
        mode: ButtonMode::Normal,
    });
    assert!(config.compile().is_err());
}

/// 写一份配置到文件
fn write_config(path: &Path, config: &GamepadConfig) {
    let text = serde_json::to_string_pretty(config).unwrap();
    std::fs::write(path, text).unwrap();
}

/// 配置名拦住路径分隔符与隐藏文件名
#[test]
fn names_are_validated() {
    assert!(valid_name("default"));
    assert!(valid_name("我的配置"));
    assert!(!valid_name(""));
    assert!(!valid_name("../x"));
    assert!(!valid_name("a/b"));
    assert!(!valid_name(".hidden"));
}

/// 写出的默认配置带 Selected，能直接生效
#[test]
fn write_default_selects_profile() {
    let dir = mml_testutil::temp_dir().join(format!("gamepad_default_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let file = write_default(&dir).unwrap();
    let config = load_file(&file).unwrap();
    assert!(config.selected);
    assert!(config.compile().is_ok());
}

/// 目录里标记 Selected 的那份生效，坏文件只记错误
#[test]
fn scan_picks_selected_profile() {
    let dir = mml_testutil::temp_dir().join(format!("gamepad_scan_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mut first = GamepadConfig::default();
    first.selected = false;
    write_config(&dir.join("a.json"), &first);
    let mut second = GamepadConfig::default();
    second.selected = true;
    write_config(&dir.join("b.json"), &second);
    std::fs::write(dir.join("broken.json"), "{").unwrap();
    // 旧读取选项不能因 serde(default) 被当成第三份有效映射。
    std::fs::write(dir.join("reader.json"), r#"{"XInput":true}"#).unwrap();

    let scan = scan(&dir);
    assert_eq!(scan.profiles.len(), 2);
    assert_eq!(scan.errors.len(), 1);
    let picked = pick(&scan.profiles).unwrap();
    assert!(picked.path.ends_with("b.json"));
}

/// 都没标记 Selected 时取文件名排序第一份
#[test]
fn scan_falls_back_to_first_file() {
    let dir = mml_testutil::temp_dir().join(format!("gamepad_order_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mut config = GamepadConfig::default();
    config.selected = false;
    write_config(&dir.join("b.json"), &config);
    write_config(&dir.join("a.json"), &config);

    let scan = scan(&dir);
    let picked = pick(&scan.profiles).unwrap();
    assert!(picked.path.ends_with("a.json"));
}

/// 目录签名随文件修改时间变化
#[test]
fn signature_changes_with_file() {
    let dir = mml_testutil::temp_dir().join(format!("gamepad_sig_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    assert!(signature(&dir).is_empty());

    write_config(&dir.join("a.json"), &GamepadConfig::default());
    let first = signature(&dir);
    assert!(!first.is_empty());
    assert_eq!(first, signature(&dir));
}

/// 组合动作保留书写顺序，松开时反序
#[test]
fn combo_keeps_press_and_release_order() {
    let combo = parse_combo("LeftAlt+E").unwrap();
    assert_eq!(
        combo.press().collect::<Vec<_>>(),
        vec![Action::Key(342), Action::Key(69)]
    );
    assert_eq!(
        combo.release().collect::<Vec<_>>(),
        vec![Action::Key(69), Action::Key(342)]
    );
    // 空项与重复都要拒绝
    assert!(parse_combo("LeftAlt+").is_err());
    assert!(parse_combo("E+E").is_err());
}

/// 标点、CapsLock、Win、Menu 这些键也要能绑，列表里的名字必须全部可解析
#[test]
fn every_action_name_parses() {
    for name in action_names() {
        assert!(parse_action(&name).is_ok(), "{name} 解析失败");
    }
    assert_eq!(parse_action("Grave"), Ok(Action::Key(96)));
    assert_eq!(parse_action("CapsLock"), Ok(Action::Key(280)));
    assert_eq!(parse_action("LeftBracket"), Ok(Action::Key(91)));
    assert_eq!(parse_action("LeftSuper"), Ok(Action::Key(343)));
    assert_eq!(parse_action("Menu"), Ok(Action::Key(348)));
}

/// 旧配置里没有 Mode 字段时按普通处理
#[test]
fn missing_mode_defaults_to_normal() {
    let text = "{\"Button\":\"south\",\"Action\":\"Space\"}";
    let rule: ButtonRule = serde_json::from_str(text).unwrap();
    assert_eq!(rule.mode, ButtonMode::Normal);
}

/// 游戏内手柄默认关，编译后原样带给转发线程
#[test]
fn enabled_defaults_to_false() {
    let config = GamepadConfig::default();
    assert!(!config.enabled);
    assert!(!config.compile().unwrap().enabled);

    let mut on = GamepadConfig::default();
    on.enabled = true;
    assert!(on.compile().unwrap().enabled);
}
