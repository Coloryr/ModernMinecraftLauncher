use super::*;

const FABRIC: &str = r#"---- Minecraft Crash Report ----
// Don't be sad, have a hug! <3

Time: 2026-10-20 21:15:32
Description: Mod loading

net.fabricmc.loader.impl.FormattedException: Mod resolution encountered an error. Please try again with a different set of mods.
	at net.fabricmc.loader.impl.FabricLoaderImpl.load(FabricLoaderImpl.java:220)
Caused by: net.fabricmc.loader.impl.discovery.ModResolutionException: Some of your mods are incompatible with the game or each other!
	... 3 more

-- System Details --
Details:
	Minecraft Version: 1.20.1
	Java Version: 17.0.8, Microsoft
	Memory: 123456789 bytes (117 MiB) / 536870912 bytes (512 MiB) up to 2147483648 bytes (2048 MiB)
	Fabric Mods: 
		fabricloader: Fabric Loader 0.15.11
		minecraft: Minecraft 1.20.1
		sodium: Sodium 0.5.8
"#;

#[test]
fn parses_crash_report_header_and_exception() {
    let report = parse(FABRIC);

    assert_eq!(report.kind, LogKind::CrashReport);
    assert_eq!(report.description.as_deref(), Some("Mod loading"));
    assert_eq!(report.time.as_deref(), Some("2026-10-20 21:15:32"));
    assert_eq!(
        report.exception.as_ref().map(|e| e.class.as_str()),
        Some("net.fabricmc.loader.impl.FormattedException")
    );
    assert_eq!(report.causes.len(), 1);
}

#[test]
fn parses_environment_and_mods() {
    let env = parse(FABRIC).environment;

    assert_eq!(env.minecraft_version.as_deref(), Some("1.20.1"));
    assert_eq!(env.java_version.as_deref(), Some("17.0.8"));
    assert_eq!(env.java_vendor.as_deref(), Some("Microsoft"));
    assert_eq!(env.memory_max_mb, Some(2048));
    assert_eq!(env.loader, Some(LoaderKind::Fabric));
    assert_eq!(env.loader_version.as_deref(), Some("0.15.11"));
    assert_eq!(env.mods.len(), 3);
    assert_eq!(
        env.mod_of("sodium").map(|m| m.version.as_str()),
        Some("0.5.8")
    );
}

#[test]
fn parses_forge_mod_table() {
    let text = r#"---- Minecraft Crash Report ----
Description: Mod loading

-- System Details --
Details:
	Minecraft Version: 1.20.1
	Mod List: 
		sodium-1.20.1.jar |Sodium |sodium |0.5.8 |COMMON_SET|Manifest: a:b
		forge-1.20.1-47.2.0.jar |Forge |forge |47.2.0 |COMMON_SET|Manifest: c:d
"#;
    let env = parse(text).environment;

    assert_eq!(env.loader, Some(LoaderKind::Forge));
    assert_eq!(env.loader_version.as_deref(), Some("47.2.0"));
    assert_eq!(
        env.mod_of("sodium").map(|m| m.name.as_str()),
        Some("Sodium")
    );
}

#[test]
fn parses_exception_from_game_log() {
    let text = r#"[21:00:00] [main/INFO]: Loading Minecraft 1.20.1
[21:00:05] [main/ERROR]: java.lang.NoSuchMethodError: 'void net.minecraft.class_310.method_1551()'
"#;
    let report = parse(text);

    assert_eq!(report.kind, LogKind::GameLog);
    assert_eq!(
        report.exception.as_ref().map(|e| e.class.as_str()),
        Some("java.lang.NoSuchMethodError")
    );
}

#[test]
fn ignores_plain_text() {
    let report = parse("just a line\nand another");

    assert_eq!(report.kind, LogKind::Unknown);
    assert!(report.exception.is_none());
    assert!(report.environment.mods.is_empty());
}

