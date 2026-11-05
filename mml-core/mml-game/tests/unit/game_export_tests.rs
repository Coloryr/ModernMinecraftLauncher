//! 直接读取实际生成的整合包，验证清单路径与内容。

use super::*;
use std::{fs, io::Read};

#[tokio::test]
async fn modrinth_archive_uses_portable_paths_for_both_download_sources() {
    let run = crate::test_support::boot();
    crate::launcher_path::instance_path::init(&run).unwrap();
    let game: InstanceSettingObj = serde_json::from_value(serde_json::json!({
        "Name": "导出测试", "DirName": format!("export-{}", uuid::Uuid::new_v4()), "Version": "1.21.1"
    })).unwrap();
    let root = game.get_base_path();
    let base = game.get_game_path();
    fs::create_dir_all(base.join("config")).unwrap();
    let config = base.join("config/中文.toml");
    fs::write(&config, "enabled=true").unwrap();
    let online = |relative: &str, info| OnlineFileExport {
        file: base.join(relative),
        size: 3,
        pack: ExportPackType::Modrinth,
        url: "https://example.invalid/mod.jar".into(),
        hash: FileHash::Sha1Sha512("sha1".into(), "sha512".into()),
        info,
    };
    let output =
        mml_testutil::temp_path(format!("modrinth-export-{}.mrpack", uuid::Uuid::new_v4()));
    game.export(ExportArg {
        file: output.clone(),
        pack: ExportPackType::Modrinth,
        archive: ArchiveType::Zip,
        mods: vec![online(
            "mods/线上.jar",
            Some(OnlineInfoObj {
                modid: "project".into(),
                fileid: "version".into(),
                ..Default::default()
            }),
        )],
        files: vec![online("mods/本地.jar", None)],
        select: vec![config],
        unselect: vec![],
        cancel: None,
        name: "整合包".into(),
        version: "1".into(),
        author: "author".into(),
        summary: "summary".into(),
        gui: None,
    })
    .await
    .unwrap();
    let mut zip = zip::ZipArchive::new(fs::File::open(&output).unwrap()).unwrap();
    let index: serde_json::Value =
        serde_json::from_reader(zip.by_name(names::MODRINTH_FILE).unwrap()).unwrap();
    // Modrinth 规范要求有 game 字段，缺了导入方会拒绝
    assert_eq!(index["game"], "minecraft");
    assert_eq!(index["formatVersion"], 1);
    assert_eq!(index["files"][0]["path"], "mods/线上.jar");
    assert_eq!(index["files"][1]["path"], "mods/本地.jar");
    assert_eq!(index["files"][0]["hashes"]["sha1"], "sha1");
    assert_eq!(
        index["files"][0]["downloads"][0],
        "https://example.invalid/mod.jar"
    );
    assert_eq!(index["dependencies"]["minecraft"], "1.21.1");
    let mut text = String::new();
    zip.by_name("overrides/config/中文.toml")
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    assert_eq!(text, "enabled=true");
    drop(zip);
    fs::remove_file(output).unwrap();
    fs::remove_dir_all(root).unwrap();
}
/// MMC 的 instance.cfg 要把自定义 JVM 参数写进去，占位符换成 MMC 变量
#[test]
fn mmc_instance_cfg_converts_jvm_placeholders() {
    let game = InstanceSettingObj {
        name: "mmc-args".into(),
        version: "1.20.1".into(),
        jvm_arg: Some(mml_config::config_obj::RunArgObj {
            max_memory: Some(4096),
            min_memory: Some(1024),
            jvm_args: Some(
                "-Dfoo=1\n-javaagent:%GAME_BASE_DIR%/mml-server-sync.jar=https://a/server.json".into(),
            ),
            ..Default::default()
        }),
        ..Default::default()
    };

    let args = mmc_jvm_args(game.jvm_arg.as_ref());
    let text = mmc_instance_cfg(&game.name, Some("gtnh_icon"), game.jvm_arg.as_ref(), &args);
    assert!(text.contains("[General]"), "{text}");
    assert!(text.contains("ConfigVersion=1.2"), "{text}");
    assert!(text.contains("OverrideJavaArgs=true"), "{text}");
    assert!(text.contains("OverrideMemory=true"), "{text}");
    assert!(text.contains("iconKey=gtnh_icon"), "{text}");
    assert!(text.contains("MaxMemAlloc=4096"), "{text}");
    assert!(text.contains("MinMemAlloc=1024"), "{text}");
    assert!(
        text.contains("JvmArgs=-Dfoo=1 -javaagent:$INST_DIR/mml-server-sync.jar=https://a/server.json"),
        "{text}"
    );
    assert!(!text.contains("%GAME_BASE_DIR%"), "{text}");
}