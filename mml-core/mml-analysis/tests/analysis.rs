//! 崩溃分析规则的集成测试

use mml_analysis::{
    Confidence, CrashReport, FindingId, LoaderKind, LogKind, Severity, analyze, is_supported_path,
};

const FABRIC_MISSING: &str = r#"---- Minecraft Crash Report ----
// Don't be sad, have a hug! <3

Time: 2026-10-20 21:15:32
Description: Mod loading

net.fabricmc.loader.impl.FormattedException: Mod resolution encountered an error. Please try again with a different set of mods.
	at net.fabricmc.loader.impl.FabricLoaderImpl.load(FabricLoaderImpl.java:220)
Caused by: net.fabricmc.loader.impl.discovery.ModResolutionException: Some of your mods are incompatible with the game or each other!
	at net.fabricmc.loader.impl.discovery.ModResolver.resolve(ModResolver.java:45)
	Mod 'sodium' (sodium) 0.5.8 requires version 0.15.0 or later of mod 'fabric-api', which is missing!

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

const FORGE_MISSING: &str = r#"---- Minecraft Crash Report ----
// Surprise! Haha. Well, this is awkward.

Time: 2026-10-20 21:20:00
Description: Mod loading

net.minecraftforge.fml.ModLoadingException: Missing or unsupported mandatory dependencies:
	Mod ID: 'fabric_api', Requested by: 'sodium', Expected range: '[0.15,)', Actual version: '[MISSING]'
	at net.minecraftforge.fml.ModLoader.waitForTransition(ModLoader.java:1)

-- System Details --
Details:
	Minecraft Version: 1.20.1
	Java Version: 17.0.8, Microsoft
	Mod List: 
		sodium-1.20.1.jar |Sodium |sodium |0.5.8 |COMMON_SET|Manifest: a:b
		forge-1.20.1-47.2.0.jar |Forge |forge |47.2.0 |COMMON_SET|Manifest: c:d
"#;

const OOM: &str = r#"---- Minecraft Crash Report ----
// Why did you do that?

Time: 2026-10-20 21:25:00
Description: Ticking entity

java.lang.OutOfMemoryError: Java heap space
	at java.base/java.util.Arrays.copyOf(Arrays.java:3537)
	at net.minecraft.world.level.chunk.LevelChunk.<init>(LevelChunk.java:1)

-- System Details --
Details:
	Minecraft Version: 1.19.2
	Java Version: 17.0.8, Eclipse Adoptium
	Memory: 123456789 bytes (117 MiB) / 1073741824 bytes (1024 MiB) up to 4294967296 bytes (4096 MiB)
"#;

const MIXIN: &str = r#"---- Minecraft Crash Report ----
// This doesn't make any sense!

Time: 2026-10-20 21:30:00
Description: Initializing game

java.lang.RuntimeException: Mixin transformation of net.minecraft.client.Minecraft failed
	at org.spongepowered.asm.mixin.transformer.MixinTransformer.transformClassBytes(MixinTransformer.java:1)
Caused by: org.spongepowered.asm.mixin.transformer.throwables.MixinTransformerError: An unexpected critical error was encountered
	at org.spongepowered.asm.mixin.transformer.MixinTransformer.transformClassBytes(MixinTransformer.java:2)
Caused by: org.spongepowered.asm.mixin.injection.throwables.InvalidInjectionException: Critical injection failure: ...
	at examplemod.mixins.json.CommonMixin.inject(CommonMixin.java:10)
	... from mod examplemod
"#;

const MIXIN_FORGE: &str = r#"---- Minecraft Crash Report ----
// This doesn't make any sense!

Time: 2026-10-20 21:30:00
Description: Initializing game

org.spongepowered.asm.mixin.transformer.throwables.MixinTransformerError: An unexpected critical error was encountered
	at org.spongepowered.asm.mixin.transformer.MixinTransformer.transformClassBytes(MixinTransformer.java:1)
Caused by: org.spongepowered.asm.mixin.injection.throwables.InvalidInjectionException: Critical injection failure: ...
	Mixin apply failed examplemod.mixins.json:ExampleMixin from mod examplemod
	at com.examplemod.mixin.ExampleMixin.inject(ExampleMixin.java:42)
"#;

const MIXIN_FABRIC: &str = r#"---- Minecraft Crash Report ----
// This doesn't make any sense!

Time: 2026-10-20 21:31:00
Description: Initializing game

net.fabricmc.loader.impl.FormattedException: java.lang.RuntimeException: Mixin apply for mod examplemod failed examplemod.mixins.json:ExampleMixin from mod examplemod
	at net.fabricmc.loader.impl.launch.knot.Knot.init(Knot.java:1)
"#;

const MIXIN_CONFIG_ONLY: &str = r#"---- Minecraft Crash Report ----
// This doesn't make any sense!

Time: 2026-10-20 21:32:00
Description: Initializing game

org.spongepowered.asm.mixin.transformer.throwables.InvalidMixinException: Mixin config examplemod.mixins.json does not specify "minVersion"
	at org.spongepowered.asm.mixin.transformer.MixinConfig.postInit(MixinConfig.java:1)
"#;

const MIXIN_STACK_ONLY: &str = r#"---- Minecraft Crash Report ----
// This doesn't make any sense!

Time: 2026-10-20 21:33:00
Description: Initializing game

org.spongepowered.asm.mixin.transformer.throwables.MixinTransformerError: An unexpected critical error was encountered
	at com.examplemod.mixin.FooMixin.bar(FooMixin.java:1)

-- System Details --
Details:
	Fabric Mods: 
		examplemod: Example Mod 1.0
"#;

const JAVA_TOO_OLD: &str = r#"---- Minecraft Crash Report ----
// Hi

Time: 2026-10-20 21:35:00
Description: Loading mod

java.lang.UnsupportedClassVersionError: com/example/mod/ExampleMod has been compiled by a more recent version of the Java Runtime (class file version 65.0), this version of the Java Runtime only recognizes class file versions up to 61.0
	at java.base/java.lang.ClassLoader.defineClass1(Native Method)

-- System Details --
Details:
	Minecraft Version: 1.20.1
	Java Version: 17.0.8, Microsoft
"#;

const GRAPHICS: &str = r#"---- Minecraft Crash Report ----
// Oh dear. Something went wrong.

Time: 2026-10-20 21:40:00
Description: Initializing game

org.lwjgl.LWJGLException: Pixel format not accelerated
	at org.lwjgl.opengl.WindowsPeerInfo.nChoosePixelFormat(Native Method)
	at org.lwjgl.opengl.WindowsDisplay.createWindow(WindowsDisplay.java:1)

-- System Details --
Details:
	Minecraft Version: 1.7.10
	OpenGL: ATI Radeon HD 5700 Series GL version 4.5, ...
"#;

const DUPLICATE: &str = r#"---- Minecraft Crash Report ----
// Hi

Time: 2026-10-20 21:45:00
Description: Mod loading

java.lang.RuntimeException: Duplicate mods found: [create, create]
	at net.minecraftforge.fml.ModLoader.load(ModLoader.java:1)
"#;

const JVM_FATAL: &str = r#"#
# A fatal error has been detected by the Java Runtime Environment:
#
#  EXCEPTION_ACCESS_VIOLATION (0xc0000005) at pc=0x00007ff6a1b2c3d4, pid=1234, tid=5678
#
# Problematic frame:
# C  [ig9icd64.dll+0x1234]
"#;

const GAME_LOG: &str = r#"[21:00:00] [main/INFO]: Loading Minecraft 1.20.1 with Fabric Loader 0.15.11
[21:00:05] [main/ERROR]: java.lang.NoSuchMethodError: 'void net.minecraft.class_310.method_1551()'
[21:00:05] [main/ERROR]: 	at com.example.mod.Example.onInitialize(Example.java:42)
"#;

const UNKNOWN: &str = r#"---- Minecraft Crash Report ----
// Hi

Time: 2026-10-20 21:50:00
Description: Rendering screen

java.lang.IllegalStateException: Something odd happened
	at com.example.mod.Renderer.render(Renderer.java:12)

-- System Details --
Details:
	Minecraft Version: 1.20.1
	OpenGL: NVIDIA GeForce RTX 4090/PCIe/SSE2, 4.6.0
"#;

const PLAIN_LOG: &str = r#"[12:00:00] [main/INFO]: Done (2.345s)!
"#;

const RANDOM_TEXT: &str = r#"java.lang.NullPointerException: Something odd
	at com.example.mod.Renderer.render(Renderer.java:12)
"#;

#[test]
fn fabric_missing_dependency() {
    let result = analyze(FABRIC_MISSING);

    assert_eq!(result.kind, LogKind::CrashReport);
    assert_eq!(result.description.as_deref(), Some("Mod loading"));
    assert_eq!(result.environment.loader, Some(LoaderKind::Fabric));
    assert_eq!(
        result.environment.minecraft_version.as_deref(),
        Some("1.20.1")
    );

    let primary = result.primary().expect("应命中缺前置");
    assert_eq!(primary.id, FindingId::MissingDependency);
    assert_eq!(primary.severity, Severity::Error);
    assert!(primary.mods.iter().any(|m| m == "sodium"));
    assert!(primary.mods.iter().any(|m| m == "fabric-api"));
    assert!(result.suspected_mods.iter().any(|m| m == "sodium"));
}

#[test]
fn forge_missing_dependency() {
    let result = analyze(FORGE_MISSING);

    assert_eq!(result.environment.loader, Some(LoaderKind::Forge));
    assert_eq!(result.environment.loader_version.as_deref(), Some("47.2.0"));

    let primary = result.primary().expect("应命中缺前置");
    assert_eq!(primary.id, FindingId::MissingDependency);
    assert!(primary.mods.iter().any(|m| m == "fabric_api"));
    assert!(primary.mods.iter().any(|m| m == "sodium"));
}

#[test]
fn out_of_memory_uses_memory_limit() {
    let result = analyze(OOM);

    let primary = result.primary().expect("应命中 OOM");
    assert_eq!(primary.id, FindingId::OutOfMemoryHeap);
    assert_eq!(primary.severity, Severity::Fatal);
    assert_eq!(primary.confidence, Confidence::High);
    assert!(primary.details.iter().any(|d| d == "memory_max_mb=4096"));
}

#[test]
fn mixin_failure_names_config() {
    let result = analyze(MIXIN);

    let primary = result.primary().expect("应命中 Mixin");
    assert_eq!(primary.id, FindingId::MixinFailure);
    assert!(
        primary
            .details
            .iter()
            .any(|d| d == "mixin_config=examplemod.mixins.json")
    );
    assert!(primary.mods.iter().any(|m| m == "examplemod"));
}

#[test]
fn mixin_failure_names_offending_mod() {
    for (name, text) in [
        ("forge", MIXIN_FORGE),
        ("fabric", MIXIN_FABRIC),
        ("config", MIXIN_CONFIG_ONLY),
    ] {
        let result = analyze(text);
        let primary = result
            .primary()
            .unwrap_or_else(|| panic!("{name} 应命中 Mixin"));
        assert_eq!(primary.id, FindingId::MixinFailure, "{name}");
        assert_eq!(
            primary.mods.first().map(String::as_str),
            Some("examplemod"),
            "{name}"
        );
        assert!(
            primary
                .details
                .iter()
                .any(|d| d == "mixin_config=examplemod.mixins.json"),
            "{name}"
        );
    }
}

#[test]
fn mixin_failure_falls_back_to_stack_package() {
    let result = analyze(MIXIN_STACK_ONLY);

    let primary = result.primary().expect("应命中 Mixin");
    assert_eq!(primary.id, FindingId::MixinFailure);
    assert_eq!(primary.mods.first().map(String::as_str), Some("examplemod"));
}

#[test]
fn java_version_too_old() {
    let result = analyze(JAVA_TOO_OLD);

    let primary = result.primary().expect("应命中 Java 版本");
    assert_eq!(primary.id, FindingId::JavaVersionTooOld);
    assert!(primary.details.iter().any(|d| d == "required_java=21"));
}

#[test]
fn graphics_driver_without_duplicate_finding() {
    let result = analyze(GRAPHICS);

    let primary = result.primary().expect("应命中图形驱动");
    assert_eq!(primary.id, FindingId::GraphicsDriver);
    assert_eq!(
        result
            .findings
            .iter()
            .filter(|f| f.id == FindingId::GraphicsDriver)
            .count(),
        1
    );
}

#[test]
fn duplicate_mods() {
    let result = analyze(DUPLICATE);

    let primary = result.primary().expect("应命中重复模组");
    assert_eq!(primary.id, FindingId::DuplicateMod);
}

#[test]
fn jvm_fatal_is_not_analyzed_yet() {
    let result = analyze(JVM_FATAL);

    assert_eq!(result.kind, LogKind::JvmFatal);
    assert!(result.findings.is_empty(), "JVM 致命错误暂不分析");
}

#[test]
fn unknown_text_is_not_analyzed() {
    let result = analyze(RANDOM_TEXT);

    assert_eq!(result.kind, LogKind::Unknown);
    assert!(result.findings.is_empty());
    assert!(result.suspected_mods.is_empty());
}

#[test]
fn supported_paths_are_logs_or_crash_reports() {
    assert!(is_supported_path("latest.log"));
    assert!(is_supported_path("crash-2026-10-20_21.15.32-client.txt"));
    assert!(!is_supported_path("notes.txt"));
    assert!(!is_supported_path("hs_err_pid123.log"));
}

#[test]
fn game_log_missing_method() {
    let result = analyze(GAME_LOG);

    assert_eq!(result.kind, LogKind::GameLog);
    let primary = result.primary().expect("应命中缺方法");
    assert_eq!(primary.id, FindingId::MissingMethod);
}

#[test]
fn unknown_crash_is_unclassified() {
    let result = analyze(UNKNOWN);

    assert_eq!(
        result.primary().map(|f| f.id),
        Some(FindingId::Unclassified)
    );
    assert!(
        !result
            .findings
            .iter()
            .any(|f| f.id == FindingId::GraphicsDriver),
        "系统信息里的 OpenGL 行不应被当成驱动错误"
    );
}

#[test]
fn plain_log_has_no_findings() {
    let result = analyze(PLAIN_LOG);

    assert_eq!(result.kind, LogKind::GameLog);
    assert!(result.findings.is_empty());
    assert!(result.suspected_mods.is_empty());
}

#[test]
fn parse_exposes_exception_chain() {
    let report = CrashReport::parse(MIXIN);

    assert_eq!(
        report.exception.as_ref().map(|e| e.class.as_str()),
        Some("java.lang.RuntimeException")
    );
    assert_eq!(report.causes.len(), 2);
    assert!(report.raw().contains("Mixin transformation"));
}
