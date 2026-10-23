//! 本次故障与附带历史日志、线程转储的隔离。

use mml_analysis::{FindingId, analyze};

#[test]
fn appended_log_cannot_supply_causes_environment_or_suspects() {
    let result = analyze(
        r#"---- Minecraft Crash Report ----
Description: Unexpected error
java.lang.StackOverflowError
    at example.loop.Call.run(Call.java:12)
-- System Details --
    Minecraft Version: 1.7.10
    Fabric Mods:
        unrelated: Unrelated 1.0
[12:00:00] [main/ERROR]: Earlier startup failure
Caused by: java.util.zip.ZipException: invalid LOC header
    at unrelated.Mod.load(Mod.java:12)
java.nio.file.AccessDeniedException: old-file
    Java Version: 8
"#,
    );
    assert!(result.causes.is_empty());
    assert!(result.environment.java_version.is_none());
    assert!(result.suspected_mods.is_empty());
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.primary().unwrap().id, FindingId::StackOverflow);
}

#[test]
fn watchdog_ignores_healthy_background_threads() {
    let result = analyze(
        r#"---- Minecraft Crash Report ----
Description: Watching Server
java.lang.Error: Watchdog
    at net.minecraft.server.MinecraftServer.tick(MinecraftServer.java:10)
-- Thread Dump --
"VoiceChat" RUNNABLE
    at voicechat.Worker.wait(Worker.java:11)
"Profiler" RUNNABLE
    at spark.Worker.wait(Worker.java:11)
-- System Details --
    Fabric Mods:
        voicechat: Simple Voice Chat 2.0
        spark: Spark 1.0
"#,
    );
    assert_eq!(result.primary().unwrap().id, FindingId::WatchdogTimeout);
    assert!(result.suspected_mods.is_empty());
    assert!(
        !result
            .findings
            .iter()
            .flat_map(|f| &f.evidence)
            .any(|line| line.contains("Worker"))
    );
}

#[test]
fn suppressed_cleanup_error_is_not_a_root_cause() {
    let result = analyze(
        "---- Minecraft Crash Report ----\njava.lang.StackOverflowError\n\tat example.Call.run(Call.java:1)\n\tSuppressed: java.util.zip.ZipException: invalid LOC header\n\t\tat example.Cleanup.run(Cleanup.java:2)\nCaused by: java.lang.StackOverflowError\n\tat example.Call.run(Call.java:3)\n",
    );
    assert_eq!(result.causes.len(), 1);
    assert_eq!(result.causes[0].class, "java.lang.StackOverflowError");
}
