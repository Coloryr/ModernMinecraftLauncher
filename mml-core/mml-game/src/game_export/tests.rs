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
        file: base.join(relative), size: 3, pack: ExportPackType::Modrinth,
        url: "https://example.invalid/mod.jar".into(),
        hash: FileHash::Sha1Sha512("sha1".into(), "sha512".into()), info,
    };
    let output = mml_testutil::temp_path(format!("modrinth-export-{}.mrpack", uuid::Uuid::new_v4()));
    game.export(ExportArg {
        file: output.clone(), pack: ExportPackType::Modrinth, archive: ArchiveType::Zip,
        mods: vec![online("mods/线上.jar", Some(OnlineInfoObj { modid: "project".into(), fileid: "version".into(), ..Default::default() }))],
        files: vec![online("mods/本地.jar", None)], select: vec![config], unselect: vec![],
        name: "整合包".into(), version: "1".into(), author: "author".into(), summary: "summary".into(), gui: None,
    }).await.unwrap();
    let mut zip = zip::ZipArchive::new(fs::File::open(&output).unwrap()).unwrap();
    let index: serde_json::Value = serde_json::from_reader(zip.by_name(names::MODRINTH_FILE).unwrap()).unwrap();
    assert_eq!(index["files"][0]["path"], "mods/线上.jar");
    assert_eq!(index["files"][1]["path"], "mods/本地.jar");
    assert_eq!(index["files"][0]["hashes"]["sha1"], "sha1");
    assert_eq!(index["files"][0]["downloads"][0], "https://example.invalid/mod.jar");
    assert_eq!(index["dependencies"]["minecraft"], "1.21.1");
    let mut text = String::new();
    zip.by_name("overrides/config/中文.toml").unwrap().read_to_string(&mut text).unwrap();
    assert_eq!(text, "enabled=true");
    drop(zip);
    fs::remove_file(output).unwrap();
    fs::remove_dir_all(root).unwrap();
}
