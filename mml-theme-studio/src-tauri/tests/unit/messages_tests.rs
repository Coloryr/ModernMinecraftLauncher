use super::*;

/// 中英两套都要有，且占位符能被替换
#[test]
fn renders_both_locales() {
    let zh = render(
        Locale::ZhCn,
        MessageKey::Created,
        &[("path", "D:/themes/a".to_string())],
    );
    let en = render(
        Locale::EnUs,
        MessageKey::Created,
        &[("path", "D:/themes/a".to_string())],
    );
    assert!(zh.contains("D:/themes/a"), "{zh}");
    assert!(en.contains("D:/themes/a"), "{en}");
    assert_ne!(zh, en);
    assert!(!zh.contains("{path}"));
}

/// 原样透传的错误不乱翻
#[test]
fn keeps_raw_errors() {
    let error = StudioError::raw("access denied");
    assert_eq!(error.text(Locale::ZhCn), "access denied");
}

