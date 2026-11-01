use super::*;
use std::io::Read;

#[test]
fn native_archive_contains_all_files_and_current_instance_snapshot() {
    crate::test_support::ensure_boot();
    let dir = crate::test_support::temp_dir("native-roundtrip");
    let root = dir.join("source");
    fs::create_dir_all(root.join(".minecraft/config")).unwrap();
    fs::write(root.join(".minecraft/config/中文.toml"), "enabled = true").unwrap();
    fs::write(root.join("notes.txt"), "user notes").unwrap();
    fs::write(root.join(mml_names::names::GAME_FILE), "stale config").unwrap();
    let game =
        serde_json::from_value(serde_json::json!({"Name":"当前实例", "Version":"1.21.1"}))
            .unwrap();
    let output = dir.join("instance.zip");
    native_archive(&game, &root, &output, &None, &AtomicBool::new(false)).unwrap();
    let mut zip = zip::ZipArchive::new(File::open(&output).unwrap()).unwrap();
    assert_eq!(zip.len(), 3);
    let mut text = String::new();
    zip.by_name(".minecraft/config/中文.toml")
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    assert_eq!(text, "enabled = true");
    let json: serde_json::Value =
        serde_json::from_reader(zip.by_name(mml_names::names::GAME_FILE).unwrap()).unwrap();
    assert_eq!(json["Name"], "当前实例");
    assert_eq!(json["Version"], "1.21.1");
    assert!(zip.file_names().all(|name| !name.unwrap().contains('\\')));
    drop(zip);
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn native_selected_archive_keeps_only_checked_files() {
    crate::test_support::ensure_boot();
    let dir = crate::test_support::temp_dir("native-selected");
    let root = dir.join("source");
    fs::create_dir_all(root.join(".minecraft/config")).unwrap();
    fs::write(root.join(".minecraft/config/keep.toml"), "keep").unwrap();
    fs::write(root.join(".minecraft/config/skip.toml"), "skip").unwrap();
    fs::write(root.join(mml_names::names::GAME_FILE), "stale config").unwrap();
    let game =
        serde_json::from_value(serde_json::json!({"Name":"勾选实例", "Version":"1.21.1"}))
            .unwrap();
    let output = dir.join("selected.zip");
    let selected = vec![root.join(".minecraft/config/keep.toml")];
    native_archive_selected(
        &game,
        &root,
        &selected,
        &output,
        &None,
        &AtomicBool::new(false),
    )
    .unwrap();
    let mut zip = zip::ZipArchive::new(File::open(&output).unwrap()).unwrap();
    let mut names: Vec<String> = zip
        .file_names()
        .map(|n| n.unwrap_or_default().to_string())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [".minecraft/config/keep.toml", mml_names::names::GAME_FILE]
    );
    let json: serde_json::Value =
        serde_json::from_reader(zip.by_name(mml_names::names::GAME_FILE).unwrap()).unwrap();
    assert_eq!(json["Name"], "勾选实例");
    drop(zip);
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn archive_read_failure_preserves_existing_archive_and_removes_partial_file() {
    crate::test_support::ensure_boot();
    let dir = crate::test_support::temp_dir("native-write-failure");
    let source = dir.join("source.txt");
    fs::write(&source, "content").unwrap();
    let output = dir.join("instance.zip");
    fs::write(&output, "previous archive").unwrap();
    let files = [
        ("valid.txt".into(), source),
        ("missing.txt".into(), dir.join("missing")),
    ];
    assert!(write_zip(&output, &files, &[], &None, &AtomicBool::new(false)).is_err());
    assert_eq!(fs::read_to_string(&output).unwrap(), "previous archive");
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 2);
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn archive_replaces_destination_without_including_itself() {
    crate::test_support::ensure_boot();
    let dir = crate::test_support::temp_dir("native-export");
    let source = dir.join("instance");
    fs::create_dir(&source).unwrap();
    let config = source.join("options.txt");
    fs::write(&config, "language:zh_cn").unwrap();
    let output = dir.join("instance.zip");
    fs::write(&output, "old archive").unwrap();
    assert!(check_destination(&source.join("bad.zip"), &source).is_err());
    write_zip(
        &output,
        &[(".minecraft/options.txt".into(), config)],
        &[(
            "game.json".into(),
            br#"{"Name":"test","Version":"1.21.1"}"#.to_vec(),
        )],
        &None,
        &AtomicBool::new(false),
    )
    .unwrap();
    let mut archive = zip::ZipArchive::new(File::open(&output).unwrap()).unwrap();
    assert_eq!(archive.len(), 2);
    let mut contents = String::new();
    archive
        .by_name(".minecraft/options.txt")
        .unwrap()
        .read_to_string(&mut contents)
        .unwrap();
    assert_eq!(contents, "language:zh_cn");
    drop(archive);
    crate::test_support::cleanup(&[&dir]);
}
#[cfg(windows)]
#[test]
fn refuses_windows_junctions_in_export_tree() {
    use std::os::windows::process::CommandExt;
    crate::test_support::ensure_boot();
    let dir = crate::test_support::temp_dir("export-junction");
    let source = dir.join("instance");
    let outside = dir.join("outside");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("private.txt"), "private").unwrap();
    let link = source.join("linked");
    let quoted = |path: &Path| path.to_string_lossy().replace('\'', "''");
    let status = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &format!(
            "New-Item -ItemType Junction -Path '{}' -Target '{}' -ErrorAction Stop | Out-Null", quoted(&link), quoted(&outside))])
        .creation_flags(0x08000000).status().unwrap();
    assert!(status.success());
    assert!(files_under(&source).is_err());
    assert!(files_under(&link).is_err());
    assert!(selected_path(&source, "linked/private.txt").is_err());
    assert!(selected_path(&source, "linked").is_err());
    fs::remove_dir(&link).unwrap();
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn selected_paths_reject_missing_and_traversal_without_dropping_files() {
    crate::test_support::ensure_boot();
    let dir = crate::test_support::temp_dir("export-selection");
    let config = dir.join("config");
    fs::create_dir(&config).unwrap();
    fs::write(config.join("中文.toml"), "enabled = true").unwrap();
    assert_eq!(
        selected_path(&dir, "config/中文.toml").unwrap(),
        config.join("中文.toml")
    );
    assert_eq!(
        files_under(&selected_path(&dir, "config").unwrap()).unwrap(),
        [config.join("中文.toml")]
    );
    for path in ["missing", "../outside", "/outside", "config/../../outside"] {
        assert!(selected_path(&dir, path).is_err(), "{path}");
    }
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn already_compressed_entries_are_stored_and_text_is_deflated() {
    crate::test_support::ensure_boot();
    let dir = crate::test_support::temp_dir("zip-entry-method");
    let jar = dir.join("a.jar");
    let text = dir.join("b.txt");
    // jar 换成本来就压不动的字节，txt 用高重复内容让 deflate 一定赢
    let bytes: Vec<u8> = (0..4096u32)
        .map(|i| (i.wrapping_mul(2654435761) >> 13) as u8)
        .collect();
    fs::write(&jar, &bytes).unwrap();
    fs::write(&text, "a".repeat(64 * 1024)).unwrap();
    let output = dir.join("methods.zip");
    let files = vec![
        ("mods/a.jar".to_string(), jar),
        ("config/b.txt".to_string(), text),
    ];
    write_zip(&output, &files, &[], &None, &AtomicBool::new(false)).unwrap();
    let mut zip = zip::ZipArchive::new(File::open(&output).unwrap()).unwrap();
    assert_eq!(
        zip.by_name("mods/a.jar").unwrap().compression(),
        zip::CompressionMethod::Stored
    );
    assert_eq!(
        zip.by_name("config/b.txt").unwrap().compression(),
        zip::CompressionMethod::Deflated
    );
    drop(zip);
    crate::test_support::cleanup(&[&dir]);
}

#[cfg(unix)]
#[test]
fn selected_paths_refuse_file_and_directory_symlinks() {
    crate::test_support::ensure_boot();
    let dir = crate::test_support::temp_dir("export-symlink");
    fs::write(dir.join("original"), "private").unwrap();
    std::os::unix::fs::symlink(dir.join("original"), dir.join("linked")).unwrap();
    assert!(selected_path(&dir, "linked").is_err());
    assert!(files_under(&dir).is_err());
    crate::test_support::cleanup(&[&dir]);
}

