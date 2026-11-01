use super::*;

/// 首版的小驼峰键仍然能读，非法取值回落到默认
#[test]
fn reads_legacy_keys() {
    let dir = mml_testutil::temp_path(format!("theme-studio-config-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("studio.json");
    std::fs::write(
        &path,
        r#"{"locale":"en-US","recent":["D:/a","D:/b"],"theme":"Light"}"#,
    )
    .unwrap();
    let config = load(&path);
    assert_eq!(config.locale, Locale::EnUs);
    assert_eq!(config.theme, Theme::Light);
    assert_eq!(config.recent, vec!["D:/a".to_string(), "D:/b".to_string()]);

    std::fs::write(&path, r#"{"Locale":"xx","Theme":"invalid"}"#).unwrap();
    let config = load(&path);
    assert_eq!(config.locale, Locale::ZhCn);
    assert_eq!(config.theme, Theme::Dark);
}

