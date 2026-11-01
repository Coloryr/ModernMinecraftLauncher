use super::super::{Instance, Manifest, archive};
use super::*;

fn fixture(extra: Option<(&str, &[u8])>) -> (PathBuf, PathBuf) {
    crate::test_support::ensure_boot();
    let dir = crate::test_support::temp_dir("server-pack");
    let file = dir.join("pack.zip");
    let uuid = uuid::Uuid::new_v4();
    let manifest = Manifest {
        format: "mml-server-pack".into(),
        version: 1,
        instances: vec![Instance {
            uuid,
            directory: "test".into(),
        }],
        launcher: extra.is_some_and(|(name, _)| name == LAUNCHER),
    };
    let mut data = vec![
        (MANIFEST.into(), serde_json::to_vec(&manifest).unwrap()),
        (CONFIG.into(), b"{}".to_vec()),
        (GUI_CONFIG.into(), b"{}".to_vec()),
        (
            "mml/minecraft/instances/test/game.json".into(),
            format!(r#"{{"UUID":"{uuid}","DirName":"test","Name":"Test"}}"#).into_bytes(),
        ),
        (
            "mml/minecraft/instances/test/.minecraft/options.txt".into(),
            b"new".to_vec(),
        ),
    ];
    if let Some((name, content)) = extra {
        data.push((name.into(), content.to_vec()));
    }
    archive::write_zip(
        &file,
        &[],
        &data,
        &None,
        &std::sync::atomic::AtomicBool::new(false),
    )
    .unwrap();
    (dir, file)
}

#[test]
fn overwrites_configs_and_instances_but_preserves_other_files() {
    let (dir, file) = fixture(None);
    let root = dir.join("runtime");
    let options = root.join("minecraft/instances/test/.minecraft/options.txt");
    fs::create_dir_all(options.parent().unwrap()).unwrap();
    fs::write(&options, "old").unwrap();
    fs::write(root.join("accounts.json"), "private").unwrap();
    import_pack(&file, &root).unwrap();
    assert_eq!(fs::read_to_string(options).unwrap(), "new");
    assert_eq!(
        fs::read_to_string(root.join("accounts.json")).unwrap(),
        "private"
    );
    assert!(root.join("gui_config.json").is_file());
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn rejects_traversal_and_unrelated_files_before_overwriting() {
    for name in [
        "mml/../outside",
        "mml/accounts.json",
        "mml/minecraft/instances/test/CON",
        "mml/minecraft/instances/test/a:stream",
    ] {
        let (dir, file) = fixture(Some((name, b"bad")));
        fs::write(dir.join("config.json"), "original").unwrap();
        assert!(import_pack(&file, &dir).is_err(), "{name}");
        assert_eq!(
            fs::read_to_string(dir.join("config.json")).unwrap(),
            "original"
        );
        crate::test_support::cleanup(&[&dir]);
    }
}

#[test]
fn failed_overwrite_restores_configs_and_removes_new_files() {
    let (dir, file) = fixture(None);
    let root = dir.join("runtime");
    let conflict = root.join("minecraft/instances/test/.minecraft/options.txt");
    fs::create_dir_all(&conflict).unwrap();
    fs::write(root.join("config.json"), "original core").unwrap();
    fs::write(root.join("gui_config.json"), "original gui").unwrap();
    assert!(import_pack(&file, &root).is_err());
    assert_eq!(
        fs::read_to_string(root.join("config.json")).unwrap(),
        "original core"
    );
    assert_eq!(
        fs::read_to_string(root.join("gui_config.json")).unwrap(),
        "original gui"
    );
    assert!(!root.join("minecraft/instances/test/game.json").exists());
    assert!(conflict.is_dir());
    assert!(fs::read_dir(&root).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".server-import-")
    }));
    crate::test_support::cleanup(&[&dir]);
}
#[test]
fn reimport_follows_instance_uuid_and_never_replaces_launcher() {
    let (dir, file) = fixture(Some((LAUNCHER, b"bundled launcher")));
    let root = dir.join("runtime");
    fs::create_dir(&root).unwrap();
    import_pack(&file, &root).unwrap();
    let original = root.join("minecraft/instances/test");
    let renamed = root.join("minecraft/instances/renamed");
    fs::rename(&original, &renamed).unwrap();
    fs::write(renamed.join(".minecraft/options.txt"), "old").unwrap();
    fs::write(root.join(LAUNCHER), "running launcher").unwrap();
    import_pack(&file, &root).unwrap();
    assert!(!original.exists());
    assert_eq!(
        fs::read_to_string(renamed.join(".minecraft/options.txt")).unwrap(),
        "new"
    );
    let game: serde_json::Value =
        serde_json::from_slice(&fs::read(renamed.join("game.json")).unwrap()).unwrap();
    assert_eq!(game["DirName"], "renamed");
    assert_eq!(
        fs::read_to_string(root.join(LAUNCHER)).unwrap(),
        "running launcher"
    );
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn imports_without_configs_and_keeps_existing_ones() {
    crate::test_support::ensure_boot();
    let dir = crate::test_support::temp_dir("server-pack-partial");
    let file = dir.join("pack.zip");
    let uuid = uuid::Uuid::new_v4();
    let manifest = Manifest {
        format: "mml-server-pack".into(),
        version: 1,
        instances: vec![Instance {
            uuid,
            directory: "test".into(),
        }],
        launcher: false,
    };
    let data = vec![
        (MANIFEST.into(), serde_json::to_vec(&manifest).unwrap()),
        (
            "mml/minecraft/instances/test/game.json".into(),
            format!(r#"{{"UUID":"{uuid}","DirName":"test","Name":"Test"}}"#).into_bytes(),
        ),
        (
            "mml/minecraft/instances/test/.minecraft/options.txt".into(),
            b"new".to_vec(),
        ),
        ("mml/custom_ui/page.zip".into(), b"page".to_vec()),
    ];
    archive::write_zip(
        &file,
        &[],
        &data,
        &None,
        &std::sync::atomic::AtomicBool::new(false),
    )
    .unwrap();

    let root = dir.join("runtime");
    fs::create_dir_all(root.join("minecraft/instances/test/.minecraft")).unwrap();
    fs::write(root.join("config.json"), "keep-core").unwrap();
    fs::write(root.join("gui_config.json"), "keep-gui").unwrap();
    let imported = import_pack(&file, &root).unwrap();
    assert!(imported.value.config.is_none());
    assert!(imported.value.gui.is_none());
    assert_eq!(
        fs::read_to_string(root.join("config.json")).unwrap(),
        "keep-core"
    );
    assert_eq!(
        fs::read_to_string(root.join("gui_config.json")).unwrap(),
        "keep-gui"
    );
    assert!(root.join("custom_ui/page.zip").is_file());
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn installs_client_lock_in_run_dir() {
    crate::test_support::ensure_boot();
    let dir = crate::test_support::temp_dir("server-pack-lock");
    let file = dir.join("pack.zip");
    let uuid = uuid::Uuid::new_v4();
    let manifest = Manifest {
        format: "mml-server-pack".into(),
        version: 1,
        instances: vec![Instance {
            uuid,
            directory: "test".into(),
        }],
        launcher: false,
    };
    let data = vec![
        (MANIFEST.into(), serde_json::to_vec(&manifest).unwrap()),
        (
            "mml/.client_lock.json".into(),
            br#"{"Password":"$pbkdf2-sha256$120000$aa$bb"}"#.to_vec(),
        ),
        (
            "mml/minecraft/instances/test/game.json".into(),
            format!(r#"{{"UUID":"{uuid}","DirName":"test","Name":"Test"}}"#).into_bytes(),
        ),
    ];
    archive::write_zip(
        &file,
        &[],
        &data,
        &None,
        &std::sync::atomic::AtomicBool::new(false),
    )
    .unwrap();

    let root = dir.join("runtime");
    fs::create_dir_all(&root).unwrap();
    import_pack(&file, &root).unwrap();
    // 锁文件落在运行目录，跟 gui_config.json 同级
    assert!(root.join(".client_lock.json").is_file());
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn failed_import_restores_previous_lock() {
    crate::test_support::ensure_boot();
    let dir = crate::test_support::temp_dir("server-pack-lock-rollback");
    let file = dir.join("pack.zip");
    let uuid = uuid::Uuid::new_v4();
    let manifest = Manifest {
        format: "mml-server-pack".into(),
        version: 1,
        instances: vec![Instance {
            uuid,
            directory: "test".into(),
        }],
        launcher: false,
    };
    let data = vec![
        (MANIFEST.into(), serde_json::to_vec(&manifest).unwrap()),
        (
            "mml/.client_lock.json".into(),
            br#"{"Salt":"new","Hash":"new"}"#.to_vec(),
        ),
        (
            "mml/minecraft/instances/test/game.json".into(),
            format!(r#"{{"UUID":"{uuid}","DirName":"test","Name":"Test"}}"#).into_bytes(),
        ),
        (
            "mml/minecraft/instances/test/.minecraft/options.txt".into(),
            b"new".to_vec(),
        ),
    ];
    archive::write_zip(
        &file,
        &[],
        &data,
        &None,
        &std::sync::atomic::AtomicBool::new(false),
    )
    .unwrap();

    let root = dir.join("runtime");
    let conflict = root.join("minecraft/instances/test/.minecraft/options.txt");
    fs::create_dir_all(&conflict).unwrap();
    let old = br#"{"Salt":"old","Hash":"old"}"#;
    fs::write(root.join(".client_lock.json"), old).unwrap();
    assert!(import_pack(&file, &root).is_err());
    // 文件回滚的同时，锁也要回到导入前那份
    assert_eq!(
        fs::read(root.join(".client_lock.json")).unwrap(),
        old.to_vec()
    );
    crate::test_support::cleanup(&[&dir]);
}

