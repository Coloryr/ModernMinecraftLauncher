//! 从真实故障缩减的异常链，省略用户路径、服务器地址和整份模组清单。

use mml_analysis::{FindingId, analyze};

#[test]
fn lwjgl_verification_is_not_file_corruption() {
    let result = analyze(
        r#"---- Minecraft Crash Report ----
Description: Initializing game
java.lang.InternalError: java.lang.VerifyError: Bad type on operand stack
Exception Details:
  Location:
    org/lwjgl/system/libffi/LibFFI.ffi_prep_cif_var()I @17: invokestatic
  Reason:
    Type 'org/lwjgl/PointerBuffer' is not assignable to 'org/lwjgl/system/CustomBuffer'
    at java.lang.invoke.InnerClassLambdaMetafactory.generateInnerClass(InnerClassLambdaMetafactory.java:358)
-- System Details --
    Java Version: 25.0.1, Example
"#,
    );
    let primary = result.primary().unwrap();
    assert_eq!(primary.id, FindingId::BytecodeVerification);
    assert!(
        primary
            .details
            .iter()
            .any(|line| line == "hint=lwjgl_type_conflict")
    );
    assert!(
        primary
            .evidence
            .iter()
            .any(|line| line.contains("is not assignable"))
    );
    assert!(
        primary
            .details
            .iter()
            .any(|line| line.starts_with("bytecode_location=org/lwjgl/system/libffi/LibFFI"))
    );
    assert!(
        !result
            .findings
            .iter()
            .any(|f| matches!(f.id, FindingId::CorruptFile | FindingId::JavaVersionTooOld))
    );
}

#[test]
fn bytecode_errors_are_separate_from_zip_integrity_errors() {
    for (error, expected) in [
        (
            "java.lang.VerifyError: Inconsistent stackmap frames",
            FindingId::BytecodeVerification,
        ),
        (
            "java.lang.ClassFormatError: Duplicate method name",
            FindingId::ClassFormat,
        ),
        (
            "java.util.zip.ZipException: invalid LOC header",
            FindingId::CorruptFile,
        ),
    ] {
        let result = analyze(&format!("---- Minecraft Crash Report ----\n{error}\n"));
        assert_eq!(result.primary().unwrap().id, expected);
        assert!(
            !result
                .primary()
                .unwrap()
                .details
                .iter()
                .any(|line| line == "hint=lwjgl_type_conflict")
        );
    }
}

#[test]
fn failed_static_initializer_is_not_a_missing_class() {
    let result = analyze(
        r#"---- Minecraft Crash Report ----
Description: Rendering screen
java.lang.NoClassDefFoundError: Could not initialize class com.gtnewhorizons.angelica.client.gui.FontConfigScreen
    at example.Menu.open(Menu.java:12)
Caused by: java.lang.ExceptionInInitializerError: Exception java.lang.ExceptionInInitializerError [in thread "Client thread"]
    at com.gtnewhorizons.angelica.client.gui.FontConfigScreen.<clinit>(FontConfigScreen.java:25)
"#,
    );
    assert_eq!(result.primary().unwrap().id, FindingId::ClassInitialization);
    assert!(
        !result
            .findings
            .iter()
            .any(|f| f.id == FindingId::MissingClass)
    );
}

#[test]
fn concrete_cause_wins_over_initialization_and_render_wrappers() {
    for wrapper in [
        "java.lang.ExceptionInInitializerError",
        "java.lang.RuntimeException: Chunk build failed",
    ] {
        let result = analyze(&format!(
            "---- Minecraft Crash Report ----\n{wrapper}\nCaused by: java.lang.ClassCastException: class net.minecraft.client.Minecraft cannot be cast to class com.gtnewhorizons.angelica.mixins.interfaces.ResourceAccessor\n    at com.gtnewhorizons.angelica.client.font.FontStrategist.<clinit>(FontStrategist.java:68)\n"
        ));
        assert_eq!(result.primary().unwrap().id, FindingId::ClassCast);
        assert!(
            result
                .primary()
                .unwrap()
                .details
                .iter()
                .any(|line| line.contains("FontStrategist.java:68"))
        );
    }
}

#[test]
fn missing_method_wins_over_entrypoint_failure() {
    let result = analyze(
        r#"---- Minecraft Crash Report ----
Description: Initializing game
java.lang.RuntimeException: Could not execute entrypoint stage 'main' due to errors, provided by 'voicechat'!
    at loader.Entry.run(Entry.java:2)
Caused by: java.lang.NoSuchMethodError: 'VoicechatClientApiImpl VoicechatClientApiImpl.instance()'
    at de.maxhenkel.replayvoicechat.ReplayVoicechatPlugin.<clinit>(ReplayVoicechatPlugin.java:15)
"#,
    );
    assert_eq!(result.primary().unwrap().id, FindingId::MissingMethod);
    assert!(
        result
            .primary()
            .unwrap()
            .details
            .iter()
            .any(|line| line.contains("ReplayVoicechatPlugin.java:15"))
    );
    assert!(
        result
            .findings
            .iter()
            .any(|f| f.id == FindingId::ModEntrypointFailure)
    );
}

#[test]
fn deeper_linkage_error_wins_over_outer_runtime_error() {
    let result = analyze(
        "---- Minecraft Crash Report ----\njava.lang.NullPointerException: wrapper\nCaused by: java.lang.NoSuchMethodError: example.Api.call()\n",
    );
    assert_eq!(result.primary().unwrap().id, FindingId::MissingMethod);
}

#[test]
fn render_failure_includes_mod_call_after_framework_frames() {
    let result = analyze(
        r#"---- Minecraft Crash Report ----
Description: Unexpected error
java.lang.IllegalStateException: Not building!
    at knot//net.minecraft.class_287.method_60802(class_287.java:76)
    at knot//icyllis.modernui.mc.TooltipRenderer.drawRoundedBackground(TooltipRenderer.java:768)
-- System Details --
    Fabric Mods:
        modernui: Modern UI 3.12
"#,
    );
    let primary = result.primary().unwrap();
    assert_eq!(primary.id, FindingId::RenderFailure);
    assert!(
        primary
            .details
            .iter()
            .any(|line| line.starts_with("related_location=")
                && line.contains("TooltipRenderer.java:768"))
    );
    assert!(
        primary
            .evidence
            .iter()
            .any(|line| line.contains("TooltipRenderer.java:768"))
    );
}

#[test]
fn runtime_programming_errors_keep_the_exact_failure_site() {
    for (error, expected, source) in [
        (
            "java.lang.NullPointerException: Cannot invoke getTime() because trainComponent is null",
            FindingId::NullPointer,
            "WheelBlockEntityRenderer.java:117",
        ),
        (
            "java.lang.ArithmeticException: / by zero",
            FindingId::Arithmetic,
            "GameUtils.java:908",
        ),
        (
            "java.lang.IndexOutOfBoundsException: Index: 1 Size: 1",
            FindingId::IndexOutOfBounds,
            "SREAbilityPlayerComponent.java:285",
        ),
        (
            "java.lang.IllegalStateException: Trying to return to in-game GUI during disconnection",
            FindingId::DisconnectedScreen,
            "Screen.java:207",
        ),
    ] {
        let result = analyze(&format!(
            "---- Minecraft Crash Report ----\n{error}\n    at io.wifi.starrailexpress.Action.run({source})\n"
        ));
        assert_eq!(result.primary().unwrap().id, expected);
        assert!(
            result
                .primary()
                .unwrap()
                .details
                .iter()
                .any(|line| line.contains(source))
        );
    }
}

#[test]
fn missing_client_class_requires_dedicated_server_and_known_mapping() {
    for (version, side, expected) in [
        ("1.21.1", "Dedicated Server", FindingId::ClientOnlyClass),
        ("1.21.1", "Client", FindingId::MissingClass),
        ("1.7.10", "Dedicated Server", FindingId::MissingClass),
    ] {
        let result = analyze(&format!(
            "---- Minecraft Crash Report ----\njava.lang.NoClassDefFoundError: net/minecraft/class_638\n    at net.minecraft.class_1937.wrapOperation$bnj000$ruokmod$wrapRandom(class_1937.java:7083)\nCaused by: java.lang.ClassNotFoundException: net.minecraft.class_638\n-- System Details --\n    Minecraft Version: {version}\n    Type: {side}\n    Fabric Mods:\n        ruokmod: R U OK 1.0\n"
        ));
        assert_eq!(result.primary().unwrap().id, expected);
        assert_eq!(result.suspected_mods, ["ruokmod"]);
    }
}

#[test]
fn watchdog_hints_are_from_the_stalled_stack() {
    for (frame, hint) in [
        (
            "org.apache.logging.log4j.core.appender.OutputStreamManager.writeBytes(OutputStreamManager.java:365)",
            "watchdog_log_output",
        ),
        (
            "net.minecraft.class_3131$class_3132.method_13669(class_3131.java:302)",
            "watchdog_spreadplayers",
        ),
    ] {
        let result = analyze(&format!(
            "---- Minecraft Crash Report ----\njava.lang.Error: Watchdog\n    at {frame}\n-- System Details --\n    Minecraft Version: 1.21.1\n"
        ));
        assert_eq!(result.primary().unwrap().id, FindingId::WatchdogTimeout);
        assert!(
            result
                .primary()
                .unwrap()
                .details
                .contains(&format!("hint={hint}"))
        );
        assert!(
            result
                .primary()
                .unwrap()
                .evidence
                .iter()
                .any(|line| line.contains(frame))
        );
    }
}

#[test]
fn dimension_and_repeated_recursion_are_actionable_details() {
    let dimension = analyze(
        "---- Minecraft Crash Report ----\njava.lang.IllegalArgumentException: Could not get provider type for dimension 191, does not exist\n",
    );
    assert!(
        dimension
            .primary()
            .unwrap()
            .details
            .contains(&"dimension=191".to_string())
    );
    let recursion = analyze(&format!(
        "---- Minecraft Crash Report ----\njava.lang.StackOverflowError\n{}",
        "    at bre.smoothfont.FontRendererHook.wrap(FontRendererHook.java:1716)\n".repeat(9)
    ));
    assert!(
        recursion
            .primary()
            .unwrap()
            .details
            .iter()
            .any(|line| line.starts_with("repeat_frame=") && line.ends_with("× 9"))
    );
}
