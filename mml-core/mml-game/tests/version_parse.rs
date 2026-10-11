//! Mojang 版本 JSON 解析测试：版本清单、版本详情、V1/V2 启动参数格式、版本号判断。
//!
//! 纯离线测试，不依赖全局初始化：不创建实例目录、不发网络请求。

use mml_game::launcher::instance_setting_obj::InstanceSettingObj;
use mml_game::mojang::VersionType;
use mml_game::mojang::game_arg_obj::{ArgValue, Argument, GameArgObj};
use mml_game::mojang::version_obj::VersionObj;
use serde_json::json;

/// 版本清单 version_manifest_v2.json 解析
#[test]
fn version_manifest_parse() {
    let json = json!({
        "latest": { "release": "1.21.1", "snapshot": "24w40a" },
        "versions": [
            {
                "id": "1.21.1",
                "type": "release",
                "url": "https://piston-meta.mojang.com/v1/packages/xxx/1.21.1.json",
                "sha1": "aaaabbbbccccdddd"
            },
            {
                "id": "24w40a",
                "type": "snapshot",
                "url": "https://piston-meta.mojang.com/v1/packages/yyy/24w40a.json",
                "sha1": "1111222233334444"
            }
        ]
    });

    let obj: VersionObj = serde_json::from_value(json).unwrap();

    assert_eq!(obj.latest.release, "1.21.1");
    assert_eq!(obj.versions.len(), 2);

    let release = &obj.versions[0];
    assert_eq!(release.id, "1.21.1");
    assert_eq!(release.version_type, "release");
    assert_eq!(
        release.url,
        "https://piston-meta.mojang.com/v1/packages/xxx/1.21.1.json"
    );
    assert_eq!(release.sha1, "aaaabbbbccccdddd");

    let snapshot = &obj.versions[1];
    assert_eq!(snapshot.id, "24w40a");
    assert_eq!(snapshot.version_type, "snapshot");

    // 序列化往返
    let data = serde_json::to_string(&obj).unwrap();
    let restored: VersionObj = serde_json::from_str(&data).unwrap();
    assert_eq!(restored.latest.release, "1.21.1");
    assert_eq!(restored.versions.len(), 2);
}

/// 版本清单缺字段时使用默认值，serde default 生效
#[test]
fn version_manifest_defaults() {
    let obj: VersionObj = serde_json::from_str("{}").unwrap();
    assert_eq!(obj.latest.release, "");
    assert!(obj.versions.is_empty());

    // versions 内条目缺 sha1 / url 时也应能解析
    let obj: VersionObj = serde_json::from_str(r#"{"versions":[{"id":"1.20.4","type":"release"}]}"#)
        .unwrap();
    assert_eq!(obj.versions.len(), 1);
    assert_eq!(obj.versions[0].sha1, "");
    assert_eq!(obj.versions[0].url, "");
}

/// V1 版本详情：旧版 `minecraftArguments` 单字符串格式。
#[test]
fn game_arg_obj_v1_parse() {
    let json = json!({
        "id": "1.12.2",
        "mainClass": "net.minecraft.client.main.Main",
        "minecraftArguments": "--username ${auth_player_name} --version ${version_name}",
        "minimumLauncherVersion": 18,
        "releaseTime": "2017-09-18T08:39:46+00:00",
        "downloads": {
            "client": {
                "sha1": "0b8a76f11c41e6d15f1c3e2c9a9a1b2c",
                "url": "https://launcher.mojang.com/v1/objects/xxx/client.jar"
            }
        }
    });

    let obj: GameArgObj = serde_json::from_value(json).unwrap();

    assert_eq!(obj.id, "1.12.2");
    assert_eq!(obj.main_class, "net.minecraft.client.main.Main");
    assert_eq!(obj.minimum_launcher_version, 18);
    // minimumLauncherVersion <= 18 → V1
    assert!(!obj.is_game_version_v2());
    assert_eq!(
        obj.minecraft_arguments.as_deref(),
        Some("--username ${auth_player_name} --version ${version_name}")
    );
    assert_eq!(obj.downloads.client.sha1, "0b8a76f11c41e6d15f1c3e2c9a9a1b2c");
    // V1 格式没有 arguments 字段
    assert!(obj.arguments.is_none());
}

/// V2 版本详情：新版 `arguments` 列表格式，Plain 与带规则的 Conditional 混排
#[test]
fn game_arg_obj_v2_parse() {
    let json = json!({
        "id": "1.20.4",
        "mainClass": "net.minecraft.client.main.Main",
        "minimumLauncherVersion": 21,
        "releaseTime": "2023-12-07T12:34:56+00:00",
        "arguments": {
            "game": [
                "--username",
                "${auth_player_name}",
                { "rules": [{ "action": "allow", "os": { "name": "windows" } }], "value": ["--win-arg"] },
                { "rules": [{ "action": "allow" }], "value": "--single-value" }
            ],
            "jvm": [
                "-Djava.library.path=${natives_directory}",
                { "rules": [{ "action": "allow", "os": { "name": "osx" } }], "value": ["-XstartOnFirstThread"] }
            ]
        },
        "javaVersion": { "majorVersion": 17 },
        "assetIndex": { "id": "8", "url": "https://piston-meta.mojang.com/v1/packages/xxx/8.json" },
        "libraries": [
            {
                "name": "org.lwjgl:lwjgl:3.3.1",
                "url": "https://libraries.minecraft.net/",
                "downloads": {
                    "artifact": {
                        "path": "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1.jar",
                        "sha1": "libsha1",
                        "url": "https://libraries.minecraft.net/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1.jar"
                    },
                    "classifiers": {
                        "natives-windows": {
                            "path": "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows.jar",
                            "sha1": "nsha1",
                            "url": "https://libraries.minecraft.net/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows.jar"
                        }
                    }
                }
            }
        ]
    });

    let obj: GameArgObj = serde_json::from_value(json).unwrap();

    // minimumLauncherVersion > 18 → V2
    assert!(obj.is_game_version_v2());

    let args = obj.arguments.as_ref().unwrap();
    // game 参数：2 个 Plain + 2 个 Conditional
    assert_eq!(args.game.len(), 4);
    assert!(matches!(&args.game[0], Argument::Plain(s) if s == "--username"));
    assert!(matches!(&args.game[1], Argument::Plain(s) if s == "${auth_player_name}"));

    // 条件参数的 value 支持单值与多值两种形态
    let Argument::Conditional(cond) = &args.game[2] else {
        panic!("第 3 个参数应为 Conditional");
    };
    assert!(matches!(cond.value, ArgValue::Multi(ref v) if v.len() == 1));

    let Argument::Conditional(cond) = &args.game[3] else {
        panic!("第 4 个参数应为 Conditional");
    };
    assert!(matches!(cond.value, ArgValue::Single(ref v) if v == "--single-value"));

    // jvm 参数
    assert_eq!(args.jvm.len(), 2);

    // 版本详情其他字段
    assert_eq!(obj.java_version.as_ref().unwrap().major_version, 17);
    let index = obj.asset_index.as_ref().unwrap();
    assert_eq!(index.id, "8");
    let libs = obj.libraries.as_ref().unwrap();
    assert_eq!(libs.len(), 1);
    assert_eq!(libs[0].name, "org.lwjgl:lwjgl:3.3.1");
    assert_eq!(libs[0].downloads.artifact.sha1, "libsha1");
    assert_eq!(
        libs[0].downloads.classifiers.as_ref().unwrap().natives_windows.sha1,
        "nsha1"
    );
}

/// 版本详情缺字段时使用默认值，覆盖 V1 之前的老版本与残缺 JSON
#[test]
fn game_arg_obj_defaults() {
    let obj: GameArgObj = serde_json::from_str(r#"{"id":"a1.0.16","mainClass":"Main"}"#).unwrap();
    assert_eq!(obj.id, "a1.0.16");
    // 默认 minimum_launcher_version = 0 → 不是 V2
    assert!(!obj.is_game_version_v2());
    assert!(obj.libraries.is_none());
    assert!(obj.arguments.is_none());
    assert!(obj.minecraft_arguments.is_none());
    assert!(obj.asset_index.is_none());
    assert_eq!(obj.downloads.client.url, "");
}

/// 版本号判断，逻辑在 version_checker：V1/V2 与 1.17 / 1.20 / 1.20.2 阈值
#[test]
fn game_arg_version_checker() {
    let make = |id: &str, min: i32| {
        GameArgObj {
            id: id.to_string(),
            minimum_launcher_version: min,
            ..Default::default()
        }
    };

    // 1.17 阈值，含 1.17
    assert!(make("1.17", 21).is_game_version_117());
    assert!(make("1.20.4", 21).is_game_version_117());
    assert!(!make("1.16.5", 21).is_game_version_117());

    // 1.20 阈值，含 1.20
    assert!(make("1.20", 21).is_game_version_120());
    assert!(make("1.20.4", 21).is_game_version_120());
    assert!(!make("1.19.4", 21).is_game_version_120());

    // 1.20.2 阈值，含 1.20.2
    assert!(make("1.20.2", 21).is_game_version_1202());
    assert!(make("1.21", 21).is_game_version_1202());
    assert!(!make("1.20.1", 21).is_game_version_1202());

    // 新版本号格式 26.x 视为最新
    assert!(make("26.1", 21).is_game_version_1202());
}

/// 实例配置上的版本号判断，与版本详情同源
#[test]
fn instance_version_checker() {
    let make = |version: &str| InstanceSettingObj {
        version: version.to_string(),
        ..Default::default()
    };

    assert!(make("1.20.4").is_game_version_120());
    assert!(make("1.20.4").is_game_version_1202());
    assert!(!make("1.12.2").is_game_version_120());
    assert!(!make("1.12.2").is_game_version_1202());
    assert!(!make("1.16.5").is_game_version_117());
    assert!(!make("1.12.2").is_game_version_117());
}

/// 版本类型枚举与独立 ID 的双向转换
#[test]
fn version_type_ids() {
    assert_eq!(VersionType::Release.id(), "release");
    assert_eq!(VersionType::Snapshot.id(), "snapshot");
    assert_eq!(VersionType::Other.id(), "other");
    assert_eq!(VersionType::All.id(), "all");

    assert_eq!(VersionType::from_id("release"), VersionType::Release);
    assert_eq!(VersionType::from_id("snapshot"), VersionType::Snapshot);
    assert_eq!(VersionType::from_id("other"), VersionType::Other);
    assert_eq!(VersionType::from_id("all"), VersionType::All);
    // 未知 ID 回退 Release
    assert_eq!(VersionType::from_id("unknown"), VersionType::Release);
}
