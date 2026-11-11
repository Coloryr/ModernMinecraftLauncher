use super::*;

/// 删掉的「平铺」模式不能让旧配置读不出来：grid 并入 group，认不出的值落回 list
#[test]
fn view_mode_reads_legacy_grid_as_group() {
    for (raw, want) in [
        ("list", ViewMode::List),
        ("group", ViewMode::Group),
        ("grid", ViewMode::Group),
        ("whatever", ViewMode::List),
    ] {
        let cfg: MainWindowConfig =
            serde_json::from_str(&format!("{{\"ViewMode\":\"{raw}\"}}")).expect("旧配置要能读");
        assert_eq!(cfg.view_mode, want, "raw={raw}");
    }
}

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

/// 首次启动按系统语言选界面语言：只有中文与英文两套，其余语言落英文
#[test]
fn system_locale_maps_to_supported_languages() {
    for (raw, want) in [
        ("zh-CN", Lang::zh_cn),
        ("zh-TW", Lang::zh_cn),
        ("ZH-HANS", Lang::zh_cn),
        ("en-US", Lang::en_us),
        ("ja-JP", Lang::en_us),
        ("", Lang::en_us),
    ] {
        assert_eq!(lang_of_locale(raw), want, "raw={raw}");
    }
}
