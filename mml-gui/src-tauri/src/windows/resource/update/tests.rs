//! 模组更新检查与应用的测试

use super::check::*;
use super::newfile::*;
use super::prepare::*;
use super::query::*;

use std::collections::HashMap;

use std::path::{Path, PathBuf};

use mml_game::game_mods::{ModObj};
use mml_game::launcher::file_online_info_obj::OnlineInfoObj;
use mml_game::launcher::{ModPackType};
use mml_net::curseforge_api::{file_obj::CurseForgeFileDataObj};
use mml_net::modrinth_api::{version_obj::ModrinthVersionObj};

use crate::dtos::{ModUpdateApplyDto};


use mml_base::file_item::FileHash;
use mml_game::game_mods::ModItemObj;
use mml_net::modrinth_api::version_obj::ModrinthVersionFileObj;

/// 造一个顶层模组条目，只填判定用得到的字段
fn mod_item(sha1: &str, name: &str, version: &str) -> ModObj {
    ModObj {
        hash: FileHash::Sha1(sha1.to_string()),
        file: PathBuf::from(format!("{name}.jar")),
        info: vec![ModItemObj {
            name: name.to_string(),
            version: Some(version.to_string()),
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// 造一条在线信息
fn online(sha1: &str, modid: &str, fileid: &str) -> OnlineInfoObj {
    OnlineInfoObj {
        sha1: sha1.to_string(),
        modid: modid.to_string(),
        fileid: fileid.to_string(),
        ..Default::default()
    }
}

/// 造一个 CurseForge 文件
fn cf_file(id: u64, date: &str) -> CurseForgeFileDataObj {
    CurseForgeFileDataObj {
        id,
        display_name: format!("file {id}"),
        file_name: format!("file-{id}.jar"),
        file_date: date.to_string(),
        ..Default::default()
    }
}

/// 造一个 Modrinth 版本，文件列表按 `(文件名, 是否 primary)` 给
fn mr_version(id: &str, date: &str, files: &[(&str, bool)]) -> ModrinthVersionObj {
    ModrinthVersionObj {
        id: id.to_string(),
        version_number: format!("v-{id}"),
        date_published: date.to_string(),
        files: files
            .iter()
            .map(|(name, primary)| ModrinthVersionFileObj {
                filename: name.to_string(),
                primary: *primary,
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}

/// 只收有在线信息的模组：没在表里的、项目号或文件号缺一项的都要跳过
#[test]
fn collect_targets_keeps_online_only() {
    let list = vec![
        mod_item("aa", "sodium", "0.5.0"),
        mod_item("bb", "manual", "1.0"),
        mod_item("cc", "half", "1.0"),
    ];
    let mut table: HashMap<String, OnlineInfoObj> = HashMap::new();
    table.insert("100".into(), online("aa", "100", "200"));
    // bb 是手动放进去的，表里没有它的 SHA1
    // cc 只有项目号、没有文件号：认不出装的是哪个文件
    table.insert("101".into(), online("cc", "101", ""));

    let targets = collect_targets(&list, &table);
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].file, "sodium.jar");
    assert_eq!(targets[0].name, "sodium");
    assert_eq!(targets[0].version, "0.5.0");
    assert_eq!(targets[0].project_id, "100");
    assert_eq!(targets[0].file_id, "200");
    // 项目号与文件号都是纯数字 → CurseForge
    assert_eq!(targets[0].source, ModPackType::CurseForge);
}

/// 没算出 SHA1 的文件不参与检查
#[test]
fn collect_targets_skips_without_hash() {
    let mut item = mod_item("dd", "nohash", "1.0");
    item.hash = FileHash::None;
    let mut table: HashMap<String, OnlineInfoObj> = HashMap::new();
    table.insert("102".into(), online("", "102", "103"));

    assert!(collect_targets(&[item], &table).is_empty());
}

/// 造一条「下载更新」的申请，只填批量取文件号用得到的字段
fn apply_item(source: &str, project_id: &str, new_file_id: &str) -> ModUpdateApplyDto {
    ModUpdateApplyDto {
        uuid: String::new(),
        source: source.to_string(),
        project_id: project_id.to_string(),
        new_file_id: new_file_id.to_string(),
        name: String::new(),
        old_version: String::new(),
        new_version: String::new(),
    }
}

/// 批量取文件号：按来源分两组、各自去重；数字号解析不了的和陌生来源都跳过
#[test]
fn split_new_file_ids_groups_sources() {
    let items = vec![
        apply_item("curseforge", "100", "300"),
        apply_item("curseforge", "100", "300"),
        apply_item("curseforge", "101", "301"),
        // 数字号解析不了：留给逐条准备报 BAD_FILE_ID
        apply_item("curseforge", "102", "not-a-number"),
        apply_item("modrinth", "aaa", "xyz"),
        apply_item("modrinth", "aaa", "xyz"),
        // 陌生来源：不取
        apply_item("other", "1", "2"),
    ];

    let ids = split_new_file_ids(&items);
    assert_eq!(ids.curseforge, vec![300, 301]);
    assert_eq!(ids.modrinth, vec![String::from("xyz")]);
}

/// CurseForge：挑 `fileDate` 最晚的那个，不按列表顺序
#[test]
fn newest_curseforge_picks_latest_date() {
    let list = vec![
        cf_file(1, "2024-01-01T00:00:00Z"),
        cf_file(3, "2024-03-01T00:00:00Z"),
        cf_file(2, "2024-02-01T00:00:00Z"),
    ];

    let latest = newest_curseforge(list).unwrap();
    assert_eq!(latest.id, "3");
    assert_eq!(latest.file, "file-3.jar");
    assert_eq!(latest.version, "file 3");
    assert_eq!(latest.date, "2024-03-01T00:00:00Z");
}

/// CurseForge：空列表没有候选
#[test]
fn newest_curseforge_empty() {
    assert!(newest_curseforge(Vec::new()).is_none());
}

/// Modrinth：挑 `date_published` 最晚的那个，文件名取 primary 那一份
#[test]
fn newest_modrinth_picks_latest_date() {
    let list = vec![
        mr_version("a", "2024-01-01T00:00:00Z", &[("old.jar", true)]),
        mr_version(
            "c",
            "2024-03-01T00:00:00Z",
            &[("first.jar", false), ("new.jar", true)],
        ),
        mr_version("b", "2024-02-01T00:00:00Z", &[("mid.jar", true)]),
    ];

    let latest = newest_modrinth(list).unwrap();
    assert_eq!(latest.id, "c");
    assert_eq!(latest.version, "v-c");
    assert_eq!(latest.file, "new.jar");
    assert_eq!(latest.date, "2024-03-01T00:00:00Z");
}

/// Modrinth：没有 primary 时退回第一个文件，空列表没有候选
#[test]
fn newest_modrinth_without_primary() {
    let list = vec![mr_version(
        "a",
        "2024-01-01T00:00:00Z",
        &[("only.jar", false)],
    )];
    assert_eq!(newest_modrinth(list).unwrap().file, "only.jar");

    assert!(newest_modrinth(Vec::new()).is_none());
}

/// 新旧文件同名时不删旧文件：下载已经覆盖了同一个路径
#[test]
fn stale_file_skips_same_path() {
    let same = Path::new("mods/sodium.jar");
    assert!(stale_file(same, same).is_none());

    let old = Path::new("mods/sodium-0.5.jar");
    let new = Path::new("mods/sodium-0.6.jar");
    assert_eq!(stale_file(old, new), Some(old));
}

/// 只差大小写的两个名字在 Windows 上是同一个文件，同样不能删
#[test]
fn stale_file_skips_case_only_change() {
    let old = Path::new("mods/Sodium.jar");
    let new = Path::new("mods/sodium.jar");

    if cfg!(windows) {
        assert!(stale_file(old, new).is_none());
    } else {
        assert_eq!(stale_file(old, new), Some(old));
    }
}
