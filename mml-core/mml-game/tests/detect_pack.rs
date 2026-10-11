//! 整合包类型检测测试：在临时目录动态构造 zip，验证 `detect_pack` 的类型判定与名字提取。
//!
//! 纯离线测试，不依赖全局初始化；压缩包放在 mml_testutil 的临时目录下，测完删除。

use std::io::Write;
use std::path::PathBuf;

use mml_game::add_game::{PackType, detect_pack};
use uuid::Uuid;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

mod common;
use common::{ZipEntry, entry};

/// 测试压缩包输出目录，mml_testutil 临时目录下的唯一子目录
fn work_dir() -> PathBuf {
    let dir = mml_testutil::temp_dir()
        .join("mml-detect-pack-test")
        .join(Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dir).expect("创建测试临时目录失败");
    dir
}

/// 程序化生成一个 zip 压缩包
///
/// - `dir`: 输出目录
/// - `stem`: 压缩包文件名，不带扩展名
/// - `files`: 条目路径与原始内容
///
/// # 返回值
///
/// 生成的压缩包路径
fn make_zip(dir: &PathBuf, stem: &str, files: &[ZipEntry]) -> PathBuf {
    let zip_path = dir.join(format!("{stem}.zip"));
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

/// CurseForge：根目录 `manifest.json`，名字取 `name` 字段。
#[test]
fn detect_curseforge() {
    let dir = work_dir();
    let manifest = br#"{"manifestType":"minecraftModpack","name":"Stone Block","version":"1.0.0"}"#;
    let zip = make_zip(&dir, "some-pack", &[entry("manifest.json", manifest)]);

    let detected = detect_pack(&zip).expect("检测失败");
    assert!(matches!(detected.pack_type, PackType::CurseForge));
    assert_eq!(detected.name, "Stone Block");

    let _ = std::fs::remove_file(&zip);
    let _ = std::fs::remove_dir(&dir);
}

/// CurseForge：`manifest.json` 位于 overrides 层级的子目录时也能命中。
#[test]
fn detect_curseforge_nested() {
    let dir = work_dir();
    let manifest = br#"{"name":"Nested Pack"}"#;
    let zip = make_zip(&dir, "nested", &[entry("pack/manifest.json", manifest)]);

    let detected = detect_pack(&zip).expect("检测失败");
    assert!(matches!(detected.pack_type, PackType::CurseForge));
    assert_eq!(detected.name, "Nested Pack");

    let _ = std::fs::remove_file(&zip);
    let _ = std::fs::remove_dir(&dir);
}

/// Modrinth：`modrinth.index.json`，名字取 `name` 字段。
#[test]
fn detect_modrinth() {
    let dir = work_dir();
    let index = br#"{"formatVersion":1,"name":"Fabulously Optimized","versionId":"1.0"}"#;
    let zip = make_zip(&dir, "fo-pack", &[entry("modrinth.index.json", index)]);

    let detected = detect_pack(&zip).expect("检测失败");
    assert!(matches!(detected.pack_type, PackType::Modrinth));
    assert_eq!(detected.name, "Fabulously Optimized");

    let _ = std::fs::remove_file(&zip);
    let _ = std::fs::remove_dir(&dir);
}

/// HMCL：`mcbbs.packmeta`，名字取 `name` 字段。
#[test]
fn detect_hmcl() {
    let dir = work_dir();
    let meta = br#"{"name":"HMCL Pack","version":"1.20.1"}"#;
    let zip = make_zip(&dir, "hmcl-pack", &[entry("mcbbs.packmeta", meta)]);

    let detected = detect_pack(&zip).expect("检测失败");
    assert!(matches!(detected.pack_type, PackType::HMCL));
    assert_eq!(detected.name, "HMCL Pack");

    let _ = std::fs::remove_file(&zip);
    let _ = std::fs::remove_dir(&dir);
}

/// HMCL 优先级高于 CurseForge：HMCL 导出包可能同时带两种元数据。
#[test]
fn detect_hmcl_wins_over_curseforge() {
    let dir = work_dir();
    let zip = make_zip(
        &dir,
        "mixed",
        &[
            entry("mcbbs.packmeta", br#"{"name":"HMCL Wins"}"#),
            entry("manifest.json", br#"{"name":"CF Name"}"#),
        ],
    );

    let detected = detect_pack(&zip).expect("检测失败");
    assert!(matches!(detected.pack_type, PackType::HMCL));
    assert_eq!(detected.name, "HMCL Wins");

    let _ = std::fs::remove_file(&zip);
    let _ = std::fs::remove_dir(&dir);
}

/// MMC：`mmc-pack.json` 无 `name` 字段时，从 `instance.cfg` 读 `name=`。
#[test]
fn detect_mmc_name_from_instance_cfg() {
    let dir = work_dir();
    let zip = make_zip(
        &dir,
        "mmc-pack",
        &[
            entry("mmc-pack.json", br#"{"formatVersion":1,"components":[]}"#),
            entry("mmc-root/instance.cfg", b"InstanceType=OneSix\nname=My MMC Inst\n"),
        ],
    );

    let detected = detect_pack(&zip).expect("检测失败");
    assert!(matches!(detected.pack_type, PackType::MMC));
    assert_eq!(detected.name, "My MMC Inst");

    let _ = std::fs::remove_file(&zip);
    let _ = std::fs::remove_dir(&dir);
}

/// MMC：`mmc-pack.json` 自带 `name` 字段时直接使用。
#[test]
fn detect_mmc_name_from_json() {
    let dir = work_dir();
    let zip = make_zip(
        &dir,
        "mmc-json",
        &[entry("mmc-pack.json", br#"{"name":"Json Name"}"#)],
    );

    let detected = detect_pack(&zip).expect("检测失败");
    assert!(matches!(detected.pack_type, PackType::MMC));
    assert_eq!(detected.name, "Json Name");

    let _ = std::fs::remove_file(&zip);
    let _ = std::fs::remove_dir(&dir);
}

/// HMCL 服务器包：`server-manifest.json`。
#[test]
fn detect_hmcl_server() {
    let dir = work_dir();
    let manifest = br#"{"name":"Server Pack","version":"1.0"}"#;
    let zip = make_zip(&dir, "server", &[entry("server-manifest.json", manifest)]);

    let detected = detect_pack(&zip).expect("检测失败");
    assert!(matches!(detected.pack_type, PackType::HMCLServer));
    assert_eq!(detected.name, "Server Pack");

    let _ = std::fs::remove_file(&zip);
    let _ = std::fs::remove_dir(&dir);
}

/// 带 `.minecraft` 目录的无元数据压缩包 → 其他启动器导出包，名字用文件名。
#[test]
fn detect_launcher_pack() {
    let dir = work_dir();
    let zip = make_zip(
        &dir,
        "launcher-export",
        &[
            entry(".minecraft/mods/some-mod.jar", b"jar"),
            entry(".minecraft/options.txt", b"fov:0"),
        ],
    );

    let detected = detect_pack(&zip).expect("检测失败");
    assert!(matches!(detected.pack_type, PackType::LauncherPack));
    assert_eq!(detected.name, "launcher-export");

    let _ = std::fs::remove_file(&zip);
    let _ = std::fs::remove_dir(&dir);
}

/// `.minecraft` 包在子目录下，双层包装也能识别。
#[test]
fn detect_launcher_pack_wrapped() {
    let dir = work_dir();
    let zip = make_zip(
        &dir,
        "wrapped",
        &[entry("game/.minecraft/versions/1.20.1/1.20.1.jar", b"j")],
    );

    let detected = detect_pack(&zip).expect("检测失败");
    assert!(matches!(detected.pack_type, PackType::LauncherPack));

    let _ = std::fs::remove_file(&zip);
    let _ = std::fs::remove_dir(&dir);
}

/// 无元数据、无 `.minecraft` 的压缩包 → 直接解压，名字取 `game.json` 的 `name` 字段。
#[test]
fn detect_archive_pack_with_game_json() {
    let dir = work_dir();
    let zip = make_zip(
        &dir,
        "plain",
        &[
            entry("game.json", br#"{"name":"Plain Name"}"#),
            entry("mods/a.jar", b"a"),
        ],
    );

    let detected = detect_pack(&zip).expect("检测失败");
    assert!(matches!(detected.pack_type, PackType::ArchivePack));
    assert_eq!(detected.name, "Plain Name");

    let _ = std::fs::remove_file(&zip);
    let _ = std::fs::remove_dir(&dir);
}

/// 完全没有可识别元数据的压缩包 → 直接解压，名字用文件名去扩展名。
#[test]
fn detect_archive_pack_fallback_name() {
    let dir = work_dir();
    let zip = make_zip(
        &dir,
        "my-cool-pack",
        &[entry("mods/a.jar", b"a"), entry("config/a.toml", b"c")],
    );

    let detected = detect_pack(&zip).expect("检测失败");
    assert!(matches!(detected.pack_type, PackType::ArchivePack));
    assert_eq!(detected.name, "my-cool-pack");

    let _ = std::fs::remove_file(&zip);
    let _ = std::fs::remove_dir(&dir);
}

/// 名字字段为空的元数据 → 回退到文件名去扩展名。
#[test]
fn detect_empty_name_falls_back_to_stem() {
    let dir = work_dir();
    let zip = make_zip(&dir, "no-name", &[entry("modrinth.index.json", br#"{"name":""}"#)]);

    let detected = detect_pack(&zip).expect("检测失败");
    assert!(matches!(detected.pack_type, PackType::Modrinth));
    assert_eq!(detected.name, "no-name");

    let _ = std::fs::remove_file(&zip);
    let _ = std::fs::remove_dir(&dir);
}

/// 不是压缩包的文件 → 返回错误而不是 panic。
#[test]
fn detect_rejects_non_archive() {
    let dir = work_dir();
    let txt = dir.join("not-a-zip.txt");
    std::fs::write(&txt, b"plain text").unwrap();

    assert!(detect_pack(&txt).is_err());

    let _ = std::fs::remove_file(&txt);
}
