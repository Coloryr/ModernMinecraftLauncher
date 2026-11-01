use super::*;
use mml_config::config_obj::RunArgObj;
use mml_game::launcher::instance_setting_obj::ProxyHostObj;
use serde_json::Value;

/// 测试用的环境取值
fn env() -> ReportEnv {
    ReportEnv {
        ui_lang: "zh_cn".into(),
        global_isolate: true,
    }
}

/// 没有在线来源
fn no_online() -> HashMap<String, OnlineInfoObj> {
    HashMap::new()
}

#[test]
fn report_lists_mods_with_state_and_source() {
    let dir = crate::test_support::temp_dir("instance-report-mods");
    fs::create_dir_all(dir.join("mods/sub")).unwrap();
    for name in [
        "z.jar",
        "中文.JAR",
        "a.jar.disable",
        "b.DISABLED",
        "readme.txt",
    ] {
        fs::write(dir.join("mods").join(name), "sample").unwrap();
    }
    fs::write(dir.join("mods/sub/nested.jar"), "nested").unwrap();
    fs::create_dir(dir.join("mods/directory.jar")).unwrap();

    let online = HashMap::from([
        (
            "z.jar".to_string(),
            OnlineInfoObj {
                file: "z.jar".into(),
                modid: "123".into(),
                fileid: "456".into(),
                url: "https://example.invalid/z.jar".into(),
                ..Default::default()
            },
        ),
        (
            "a.jar".to_string(),
            OnlineInfoObj {
                file: "a.jar".into(),
                modid: "789".into(),
                fileid: "012".into(),
                ..Default::default()
            },
        ),
    ]);
    let report = report(&InstanceSettingObj::default(), &dir, &env(), &online).unwrap();

    let find = |path: &str| {
        report
            .mods
            .iter()
            .find(|item| item.path == path)
            .unwrap_or_else(|| panic!("缺少 {path}"))
    };
    assert_eq!(report.mods.len(), 6);
    let z = find("mods/z.jar");
    assert!(z.enabled);
    assert_eq!(z.project_id.as_deref(), Some("123"));
    assert_eq!(z.file_id.as_deref(), Some("456"));
    assert_eq!(z.url.as_deref(), Some("https://example.invalid/z.jar"));
    // 每个模组都算 SHA1，不论有没有在线来源
    let sample = mml_base::hash_helper::gen_hash(HashType::Sha1, b"sample");
    assert_eq!(z.sha1.as_deref(), Some(sample.as_str()));
    assert_eq!(find("mods/中文.JAR").sha1.as_deref(), Some(sample.as_str()));

    // 停用的模组按去掉后缀的名字找在线来源，来源与 SHA1 都不能丢
    let disabled = find("mods/a.jar.disable");
    assert!(!disabled.enabled);
    assert_eq!(disabled.project_id.as_deref(), Some("789"));
    assert_eq!(disabled.file_id.as_deref(), Some("012"));
    assert_eq!(disabled.sha1.as_deref(), Some(sample.as_str()));
    assert!(!find("mods/b.DISABLED").enabled);
    assert!(find("mods/中文.JAR").enabled);
    assert!(find("mods/sub/nested.jar").enabled);

    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn report_counts_resources_and_saves() {
    let dir = crate::test_support::temp_dir("instance-report-resources");
    fs::create_dir_all(dir.join("resourcepacks")).unwrap();
    fs::write(dir.join("resourcepacks/pack.zip"), "pack").unwrap();
    fs::create_dir_all(dir.join("shaderpacks")).unwrap();
    fs::write(dir.join("shaderpacks/shader.zip"), "shader").unwrap();
    fs::create_dir_all(dir.join("schematics")).unwrap();
    fs::write(dir.join("schematics/house.schem"), "schem").unwrap();
    fs::create_dir_all(dir.join("screenshots")).unwrap();
    fs::write(dir.join("screenshots/a.png"), "png").unwrap();
    fs::write(dir.join("screenshots/b.png"), "png").unwrap();
    fs::create_dir_all(dir.join("saves/world/datapacks")).unwrap();
    fs::write(dir.join("saves/world/level.dat"), "nbt").unwrap();
    fs::write(dir.join("saves/world/datapacks/pack.zip"), "pack").unwrap();

    let report = report(&InstanceSettingObj::default(), &dir, &env(), &no_online()).unwrap();

    // 资源包 / 光影 / 结构文件不算哈希，只给大小
    assert_eq!(report.resource_packs.len(), 1);
    assert!(report.resource_packs[0].sha1.is_none());
    assert_eq!(report.resource_packs[0].size, 4);
    assert_eq!(report.shader_packs.len(), 1);
    assert_eq!(report.schematics.len(), 1);
    assert_eq!(report.screenshots, 2);

    assert_eq!(report.saves.len(), 1);
    assert_eq!(report.saves[0].directory, "world");
    assert_eq!(report.saves[0].data_packs, ["pack.zip"]);
    assert_eq!(report.saves[0].size, 7);

    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn report_carries_launch_settings_and_environment() {
    let dir = crate::test_support::temp_dir("instance-report-launch");
    let mut game = InstanceSettingObj {
        name: "中文实例".into(),
        version: "1.21.1".into(),
        loader: mml_game::loader::LoaderType::Forge,
        loader_version: Some("52.0.1".into()),
        ..Default::default()
    };
    game.jvm_arg = Some(RunArgObj {
        min_memory: Some(1024),
        max_memory: Some(8192),
        jvm_args: Some("-XX:+UseG1GC\n-XX:MaxGCPauseMillis=50".into()),
        game_args: Some("--width 1920".into()),
        isolate_home: Some(false),
        gc_mode: Some(GCType::G1GC),
        ..Default::default()
    });
    game.jvm_name = Some("Java 21".into());
    game.window = Some(mml_config::config_obj::WindowSettingObj {
        width: Some(1920),
        height: Some(1080),
        full_screen: Some(true),
        ..Default::default()
    });

    let report = report(&game, &dir, &env(), &no_online()).unwrap();

    assert_eq!(report.minecraft, "1.21.1");
    assert_eq!(report.loader, "forge");
    assert_eq!(report.loader_version.as_deref(), Some("52.0.1"));
    assert_eq!(report.launch.min_memory, Some(1024));
    assert_eq!(report.launch.max_memory, Some(8192));
    assert_eq!(report.launch.gc, "g1gc");
    assert_eq!(report.launch.jvm_args.len(), 2);
    assert_eq!(report.launch.game_args, ["--width 1920"]);
    assert!(!report.launch.isolate_home, "实例写了 false 就不跟全局默认");
    assert_eq!(report.launch.java_name.as_deref(), Some("Java 21"));
    assert_eq!(report.launch.window_width, Some(1920));
    assert!(report.launch.fullscreen);
    assert_eq!(report.locale.ui, "zh_cn");
    assert_eq!(report.format, "mml-instance-info-v2");
    assert!(!report.launcher.version.is_empty());
    assert!(!report.system.os.is_empty());

    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn report_never_writes_private_fields() {
    let dir = crate::test_support::temp_dir("instance-report-private");
    let mut game = InstanceSettingObj {
        name: "实例".into(),
        ..Default::default()
    };
    game.jvm_local = Some("C:/private/java/bin/java.exe".into());
    game.server_url = Some("https://private.example/server".into());
    game.proxy_host = Some(ProxyHostObj {
        ip: Some("10.0.0.1".into()),
        port: Some(1080),
        user: Some("private-user".into()),
        password: Some("private-pass".into()),
    });
    game.jvm_arg = Some(RunArgObj {
        pre_run_arg: Some("private-command".into()),
        post_run_arg: Some("private-command".into()),
        jvm_env: Some("SECRET=private".into()),
        ..Default::default()
    });

    let text =
        serde_json::to_string(&report(&game, &dir, &env(), &no_online()).unwrap()).unwrap();
    assert!(!text.contains("private"), "{text}");
    assert!(!text.contains("10.0.0.1"), "{text}");

    let value: Value = serde_json::from_str(&text).unwrap();
    for key in [
        "Format",
        "Launcher",
        "System",
        "Locale",
        "Modpack",
        "Launch",
        "Mods",
        "ResourcePacks",
        "ShaderPacks",
        "Saves",
        "Schematics",
        "Screenshots",
    ] {
        assert!(value.get(key).is_some(), "缺少 {key}");
    }

    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn report_handles_empty_instance() {
    let dir = crate::test_support::temp_dir("instance-report-empty");
    let report = report(&InstanceSettingObj::default(), &dir, &env(), &no_online()).unwrap();
    assert!(report.mods.is_empty());
    assert!(report.saves.is_empty());
    assert_eq!(report.screenshots, 0);
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn report_propagates_unreadable_mod_directory() {
    let dir = crate::test_support::temp_dir("instance-report-invalid");
    fs::write(dir.join("mods"), "not a directory").unwrap();
    assert!(report(&InstanceSettingObj::default(), &dir, &env(), &no_online()).is_err());
    crate::test_support::cleanup(&[&dir]);
}

