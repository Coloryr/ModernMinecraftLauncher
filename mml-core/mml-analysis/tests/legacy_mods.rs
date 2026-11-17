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

const MODERN_LIST_WITH_AE2: &str = r#"---- Minecraft Crash Report ----
// This doesn't make any sense!

Time: 2026-11-17 20:40:06
Description: Rendering screen

java.lang.NullPointerException: Cannot invoke "appeng.api.stacks.AEKey.formatAmount(long, appeng.api.stacks.AmountFormat)" because the return value of "appeng.menu.me.crafting.CraftingStatusEntry.getWhat()" is null
	at appeng.client.gui.me.crafting.CraftingStatusTableRenderer.getEntryDescription(CraftingStatusTableRenderer.java:47) ~[appliedenergistics2-forge-15.4.10.jar%23238!/:?] {re:classloading}
	at appeng.client.gui.me.crafting.AbstractTableRenderer.render(AbstractTableRenderer.java:98) ~[appliedenergistics2-forge-15.4.10.jar%23238!/:?] {re:mixin,re:classloading}
	at net.minecraft.client.gui.screens.inventory.AbstractContainerScreen.m_88315_(AbstractContainerScreen.java:116) ~[client-1.20.1-20230612.114412-srg.jar%23306!/:?] {re:mixin,pl:mixin:APP:ae2.mixins.json:AbstractContainerScreenMixin}

-- System Details --
Details:
	Minecraft Version: 1.20.1
	Java Version: 25.0.1, Microsoft
	Mod List: 
		chloride-FORGE-mc1.20.1-v1.7.5.jar                |Chloride                      |chloride                      |1.7.5               |DONE      |Manifest: NOSIGNATURE
		ExtendedAE-1.20-1.4.12-forge.jar                  |ExtendedAE                    |expatternprovider             |1.20-1.4.12-forge   |DONE      |Manifest: NOSIGNATURE
		Patchouli-1.20.1-85-FORGE.jar                     |Patchouli                     |patchouli                     |1.20.1-85-FORGE     |DONE      |Manifest: NOSIGNATURE
		appliedenergistics2-forge-15.4.10.jar             |Applied Energistics 2         |ae2                           |15.4.10             |DONE      |Manifest: NOSIGNATURE
"#;

#[test]
fn modern_mod_list_file_name_is_not_a_loading_state() {
    let result = analyze(MODERN_LIST_WITH_AE2);

    assert!(
        result
            .findings
            .iter()
            .all(|finding| finding.id != FindingId::ModErrored),
        "jar 文件名里的 E 不是加载状态：{:?}",
        result
            .findings
            .iter()
            .map(|finding| finding.id.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        result.primary().map(|finding| finding.id),
        Some(FindingId::NullPointer)
    );
    assert_eq!(
        result.suspected_mods,
        ["ae2"],
        "模组 id 与包名不同，要靠栈帧末尾的 jar 文件名对上"
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
