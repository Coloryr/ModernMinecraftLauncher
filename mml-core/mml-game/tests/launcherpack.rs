//! LauncherPack 的检测逻辑测试：导入其他启动器压缩包。
//!
//! 覆盖 `.minecraft` 定位、版本隔离判定、版本扫描与主版本选择。
//! 这些测试不依赖实例列表、路径缓存等全局状态，可以独立运行。
//! 压缩包为程序化生成，二进制样本不提交进 git。

use std::io::Write;
use std::path::PathBuf;

use mml_base::archives::BaseArchive;
use mml_game::add_game::{
    find_minecraft_prefix, pick_primary, scan_versions, version_folder_has_data,
};
use uuid::Uuid;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

mod common;
use common::{ZipEntry, entry};

/// 程序化生成一个 zip 压缩包
///
/// - `files`: 条目路径与原始内容
///
/// # 返回值
///
/// 生成的压缩包路径，落在测试临时目录
fn make_zip(files: &[ZipEntry]) -> PathBuf {
    let zip_path = mml_testutil::temp_dir().join(format!("mml-launcherpack-{}.zip", Uuid::new_v4()));
    let file = std::fs::File::create(&zip_path).expect("创建测试压缩包失败");
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    for e in files {
        writer.start_file(e.name, options).unwrap();
        writer.write_all(e.data).unwrap();
    }
    writer.finish().unwrap();
    zip_path
}

/// 生成一个最小可解析的官方版本 json
///
/// - `id`: 版本 ID
///
/// # 返回值
///
/// 只含 `id` 字段的 JSON 字节
fn version_json(id: &str) -> Vec<u8> {
    format!(r#"{{"id":"{id}"}}"#).into_bytes()
}

/// `.minecraft` 在压缩包根目录时能定位到
#[test]
fn find_minecraft_prefix_finds_root() {
    let zip = make_zip(&[
        entry(".minecraft/mods/a.jar", b"a"),
        entry(".minecraft/options.txt", b"x"),
    ]);
    let archive = BaseArchive::open(&zip).unwrap();
    assert_eq!(
        find_minecraft_prefix(archive.entries()).as_deref(),
        Some(".minecraft/")
    );
}

/// `.minecraft` 被一层目录包住时也能定位到
#[test]
fn find_minecraft_prefix_finds_wrapped() {
    let zip = make_zip(&[entry("mygame/.minecraft/mods/a.jar", b"a")]);
    let archive = BaseArchive::open(&zip).unwrap();
    assert_eq!(
        find_minecraft_prefix(archive.entries()).as_deref(),
        Some("mygame/.minecraft/")
    );
}

/// 压缩包里没有 `.minecraft` 时返回 `None`
#[test]
fn find_minecraft_prefix_none() {
    let zip = make_zip(&[entry("some/other/file.txt", b"a")]);
    let archive = BaseArchive::open(&zip).unwrap();
    assert!(find_minecraft_prefix(archive.entries()).is_none());
}

/// 版本文件夹内除 json / jar 外还有资源时判为隔离
#[test]
fn version_folder_has_data_detects_isolation() {
    // 版本文件夹内除 json/jar 外还有游戏资源 → 视为隔离
    let json = version_json("1.21.1");
    let zip = make_zip(&[
        entry(".minecraft/versions/1.21.1/1.21.1.json", json.as_slice()),
        entry(".minecraft/versions/1.21.1/1.21.1.jar", b"jar"),
        entry(".minecraft/versions/1.21.1/config/example.toml", b"c"),
    ]);
    let archive = BaseArchive::open(&zip).unwrap();
    assert!(version_folder_has_data(&archive, ".minecraft/", "1.21.1"));

    // 只有 json/jar → 常规布局，未隔离
    let json = version_json("1.21.1");
    let zip = make_zip(&[
        entry(".minecraft/versions/1.21.1/1.21.1.json", json.as_slice()),
        entry(".minecraft/versions/1.21.1/1.21.1.jar", b"jar"),
    ]);
    let archive = BaseArchive::open(&zip).unwrap();
    assert!(!version_folder_has_data(&archive, ".minecraft/", "1.21.1"));
}

/// 版本扫描同时认出根目录版本与隔离版本
#[test]
fn scan_versions_detects_isolation_and_name() {
    let root = version_json("1.20.1");
    let isolated = version_json("1.21.1");
    let zip = make_zip(&[
        entry(".minecraft/1.20.1.json", root.as_slice()),
        entry(
            ".minecraft/versions/1.21.1/1.21.1.json",
            isolated.as_slice(),
        ),
    ]);
    let archive = BaseArchive::open(&zip).unwrap();
    let versions = scan_versions(&archive, ".minecraft/");
    assert_eq!(versions.len(), 2);

    let non_isolated = versions
        .iter()
        .find(|version| version.version_name == "1.20.1")
        .unwrap();
    assert!(!non_isolated.isolated);
    assert_eq!(non_isolated.obj.id, "1.20.1");

    let isolated = versions
        .iter()
        .find(|version| version.version_name == "1.21.1")
        .unwrap();
    assert!(isolated.isolated);
    assert_eq!(isolated.obj.id, "1.21.1");
}

/// 主版本优先取 `launcher_profiles.json` 里的 `lastVersionId`
#[test]
fn pick_primary_prefers_launcher_profiles() {
    let v1 = version_json("1.20.1");
    let v2 = version_json("1.21.1");
    let zip = make_zip(&[
        entry(".minecraft/1.20.1.json", v1.as_slice()),
        entry(".minecraft/1.21.1.json", v2.as_slice()),
        entry(
            ".minecraft/launcher_profiles.json",
            br#"{"profiles":{"a":{"lastVersionId":"1.21.1"}}}"#,
        ),
    ]);
    let archive = BaseArchive::open(&zip).unwrap();
    let versions = scan_versions(&archive, ".minecraft/");
    let index = pick_primary(&archive, ".minecraft/", &versions).unwrap();
    assert_eq!(versions[index].obj.id, "1.21.1");
}

/// 没有 profiles 时优先取带数据的隔离版本
#[test]
fn pick_primary_prefers_isolated_with_data() {
    let v1 = version_json("1.20.1");
    let v2 = version_json("1.21.1");
    let zip = make_zip(&[
        entry(".minecraft/1.20.1.json", v1.as_slice()),
        entry(".minecraft/versions/1.21.1/1.21.1.json", v2.as_slice()),
        entry(".minecraft/versions/1.21.1/config/example.toml", b"c"),
    ]);
    let archive = BaseArchive::open(&zip).unwrap();
    let versions = scan_versions(&archive, ".minecraft/");
    let index = pick_primary(&archive, ".minecraft/", &versions).unwrap();
    assert_eq!(versions[index].obj.id, "1.21.1");
}

/// 只有一个版本时直接选它
#[test]
fn pick_primary_single_version() {
    let v1 = version_json("1.20.1");
    let zip = make_zip(&[entry(".minecraft/1.20.1.json", v1.as_slice())]);
    let archive = BaseArchive::open(&zip).unwrap();
    let versions = scan_versions(&archive, ".minecraft/");
    assert_eq!(pick_primary(&archive, ".minecraft/", &versions), Some(0));
}
