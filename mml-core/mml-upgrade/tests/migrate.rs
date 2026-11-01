//! 旧 JSON 索引导入新库：条数、跳过项、留底改名

use std::fs;
use std::path::Path;

use mml_link::{IndexKind, Store};
use mml_names::names;
use mml_upgrade::migrate;

/// 启动内核日志：迁移失败时会走 `mml_log`，没启动就 panic
fn boot() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let dir = mml_testutil::temp_path("mml-upgrade-log");
        let _ = fs::create_dir_all(&dir);
        let _ = mml_log::start(&dir);
    });
}

/// 建一个只属于本用例的临时目录
fn case_dir(tag: &str) -> std::path::PathBuf {
    let dir = mml_testutil::temp_path(format!("mml-upgrade-{tag}-{}", uuid::Uuid::new_v4()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 写一份旧格式索引
fn write_legacy(root: &Path, file: &str, json: &str) {
    fs::write(root.join(file), json).unwrap();
}

#[test]
fn imports_legacy_mods_and_keeps_backup() {
    boot();
    let dir = case_dir("mods");
    // 两条有效记录 + 一条缺 hash 的记录
    write_legacy(
        &dir,
        names::MOD_DOWNLOAD_FILE,
        r#"{
            "p1": [
                {"pid":"p1","fid":"1","name":"A","hash":"AAAA","file":"a.jar","url":"https://x/a"},
                {"pid":"p1","fid":"2","name":"B","hash":"bbbb","file":"b.jar","url":""},
                {"pid":"p1","fid":"3","name":"C","hash":"","file":"c.jar","url":""}
            ]
        }"#,
    );

    let store = Store::open(&dir.join(names::DOWNLOAD_INDEX_DIR)).unwrap();
    let report = migrate(&dir, &store, |_, _| true);

    assert!(report.did_work());
    assert_eq!(report.mods, 2);
    assert_eq!(report.resourcepacks, 0);
    assert_eq!(report.failed, 1);

    // 旧文件改成 .bak，原文件不在了
    assert!(!dir.join(names::MOD_DOWNLOAD_FILE).exists());
    assert!(dir.join(format!("{}.bak", names::MOD_DOWNLOAD_FILE)).exists());

    // 记录进了库，且 hash 已转小写
    let rows = store.load(IndexKind::Mod).unwrap();
    assert_eq!(rows.len(), 2);
    let hashes: Vec<String> = rows
        .iter()
        .map(|(_, payload)| {
            let value: serde_json::Value = serde_json::from_slice(payload).unwrap();
            value["hash"].as_str().unwrap().to_string()
        })
        .collect();
    assert!(hashes.contains(&"aaaa".to_string()));
    assert!(hashes.contains(&"bbbb".to_string()));

    // 再跑一次没有旧文件，什么都不该做
    let again = migrate(&dir, &store, |_, _| true);
    assert!(!again.did_work());
    assert_eq!(again.mods, 0);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn imports_legacy_resourcepacks() {
    boot();
    let dir = case_dir("packs");
    write_legacy(
        &dir,
        names::RESOURCEPACK_DOWNLOAD_FILE,
        r#"{"p9":[{"pid":"p9","fid":"9","name":"Pack","hash":"CCCC","file":"p.zip","url":""}]}"#,
    );

    let store = Store::open(&dir.join(names::DOWNLOAD_INDEX_DIR)).unwrap();
    let report = migrate(&dir, &store, |_, _| true);

    assert!(report.did_work());
    assert_eq!(report.resourcepacks, 1);
    // 模组那边没有旧文件
    assert_eq!(report.mods, 0);
    assert_eq!(store.load(IndexKind::ResourcePack).unwrap().len(), 1);
    assert!(store.load(IndexKind::Mod).unwrap().is_empty());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn broken_legacy_file_is_kept() {
    boot();
    let dir = case_dir("broken");
    write_legacy(&dir, names::MOD_DOWNLOAD_FILE, "{not json");

    let store = Store::open(&dir.join(names::DOWNLOAD_INDEX_DIR)).unwrap();
    let report = migrate(&dir, &store, |_, _| true);

    assert!(!report.did_work());
    assert!(report.failed > 0);
    // 解析不了的文件原样留着，人工还能看
    assert!(dir.join(names::MOD_DOWNLOAD_FILE).exists());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn cancel_keeps_legacy_file_and_resumes() {
    boot();
    let dir = case_dir("cancel");
    write_legacy(
        &dir,
        names::MOD_DOWNLOAD_FILE,
        r#"{"p1":[
            {"pid":"p1","fid":"1","name":"A","hash":"AAAA","file":"a.jar","url":""},
            {"pid":"p1","fid":"2","name":"B","hash":"BBBB","file":"b.jar","url":""},
            {"pid":"p1","fid":"3","name":"C","hash":"CCCC","file":"c.jar","url":""}
        ]}"#,
    );

    let store = Store::open(&dir.join(names::DOWNLOAD_INDEX_DIR)).unwrap();

    // 写两条就叫停
    let mut seen = 0usize;
    let report = migrate(&dir, &store, |now, _| {
        seen = now;
        now < 2
    });
    assert!(report.cancelled);
    assert!(!report.migrated);
    // 旧文件还在，下次启动还会再跑
    assert!(dir.join(names::MOD_DOWNLOAD_FILE).exists());
    assert_eq!(store.keys(IndexKind::Mod).unwrap().len(), 2);
    assert!(seen <= 2);

    // 再来一次：已导入的跳过，剩下的补上，最后留底改名
    let report = migrate(&dir, &store, |_, _| true);
    assert!(!report.cancelled);
    assert!(report.migrated);
    assert_eq!(store.keys(IndexKind::Mod).unwrap().len(), 3);
    assert!(!dir.join(names::MOD_DOWNLOAD_FILE).exists());
    assert!(dir.join(format!("{}.bak", names::MOD_DOWNLOAD_FILE)).exists());

    let _ = fs::remove_dir_all(&dir);
}

