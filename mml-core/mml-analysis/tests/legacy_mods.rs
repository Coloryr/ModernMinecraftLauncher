//! 旧版 Forge 报告中的模组表与调用栈关联。

use mml_analysis::{Confidence, FindingId, LoaderKind, analyze};

#[test]
fn forge_17_mods_preserve_id_name_and_version() {
    let result = analyze(
        r#"---- Minecraft Crash Report ----
java.lang.StackOverflowError
    at crazypants.enderio.conduit.ItemConduit.update(ItemConduit.java:480)
-- System Details --
    UCHIJA Forge{10.13.4.1614} [Minecraft Forge] (forge.jar)
    UCHIJA EnderIO{2.9.1} [Ender IO] (enderio.jar)
    UCHIJA mcp{9.05} [Minecraft Coder Pack] (minecraft.jar)
"#,
    );
    let entry = result.environment.mod_of("enderio").unwrap();
    assert_eq!(entry.id, "EnderIO");
    assert_eq!(entry.name, "Ender IO");
    assert_eq!(entry.version, "2.9.1");
    assert_eq!(result.environment.loader, Some(LoaderKind::Forge));
    assert_eq!(
        result.environment.loader_version.as_deref(),
        Some("10.13.4.1614")
    );
    assert_eq!(result.suspected_mods, ["EnderIO"]);
}

#[test]
fn forge_17_errored_mods_are_named_as_the_cause() {
    let result = analyze(
        "---- Minecraft Crash Report ----\njava.lang.NullPointerException\n    at cpw.mods.fml.common.eventhandler.EventBus.post(EventBus.java:1)\n-- System Details --\n\tUCE\tchromatictooltips{1.0.36-GTNH} [ChromaticTooltips] (chromatictooltips-1.0.36-GTNH.jar) \n\tUCE\tchromatictooltipscompat{1.0.40-GTNH} [ChromaticTooltipsCompat] (chromatictooltipscompat-1.0.40-GTNH.jar) \n\tUCHIJAAAA\tForge{10.13.4.1614} [Minecraft Forge] (forge.jar) \n",
    );
    let finding = result
        .findings
        .iter()
        .find(|finding| finding.id == FindingId::ModErrored)
        .expect("errored mods 未命中");
    assert_eq!(
        finding.mods,
        ["chromatictooltips", "chromatictooltipscompat"]
    );
    assert_eq!(finding.confidence, Confidence::High);
    assert_eq!(result.findings[0].id, FindingId::ModErrored);
    assert_eq!(result.suspected_mods.len(), 2);
    assert_eq!(
        result
            .environment
            .mod_of("chromatictooltips")
            .unwrap()
            .states,
        "UCE"
    );
}

#[test]
fn forge_112_state_table_does_not_parse_headers_as_mods() {
    let result = analyze(
        r#"---- Minecraft Crash Report ----
java.lang.StackOverflowError
    at bre.smoothfont.FontRendererHook.wrap(FontRendererHook.java:1716)
-- System Details --
    | State | ID | Version | Source |
    |:----- |:-- |:------- |:------ |
    | LCHIJA | forge | 14.23.5.2847 | forge.jar |
    | LCHIJA | smoothfont | 2.1.4 | SmoothFont.jar |
"#,
    );
    assert_eq!(result.environment.mods.len(), 2);
    assert_eq!(
        result.environment.mod_of("smoothfont").unwrap().version,
        "2.1.4"
    );
    assert_eq!(result.suspected_mods, ["smoothfont"]);
}
