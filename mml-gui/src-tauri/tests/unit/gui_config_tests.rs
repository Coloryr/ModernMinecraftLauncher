use super::*;

/// 删掉的材质选项要收口，别让设置页那个下拉落到不存在的值上
#[test]
fn window_effect_normalizes_removed_options() {
    let mut config = GuiConfig::default();
    for (raw, want) in [
        ("auto", "none"),
        ("blur", "acrylic"),
        ("mica", "mica"),
        ("acrylic", "acrylic"),
        ("", "none"),
    ] {
        config.client.window_effect = raw.to_string();
        normalize_window_effect(&mut config);
        assert_eq!(config.client.window_effect, want, "raw={raw}");
    }
}

