use mml_cloud::{DELETE_FILE, Manifest, archive};
use std::{
    fs,
    io::{Cursor, Write},
};
use uuid::Uuid;
use zip::{ZipWriter, write::SimpleFileOptions};

fn directory(label: &str) -> std::path::PathBuf {
    let root = mml_testutil::temp_path(format!("cloud-{label}-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    root
}

fn zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in files {
        zip.start_file(*name, SimpleFileOptions::default()).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

#[test]
fn delta_roundtrip_includes_deletions_and_keeps_backup() {
    let root = directory("roundtrip");
    let source = root.join("source");
    let target = root.join("target");
    fs::create_dir_all(source.join("region")).unwrap();
    fs::create_dir_all(&target).unwrap();
    fs::write(source.join("level.dat"), b"new").unwrap();
    fs::write(source.join("region/r.0.0.mca"), b"region").unwrap();
    fs::write(source.join("session.lock"), b"volatile").unwrap();
    fs::write(target.join("level.dat"), b"old").unwrap();
    fs::write(target.join("removed.dat"), b"remove").unwrap();
    let local = archive::scan(&source).unwrap();
    let bytes = archive::delta(&source, &local, &archive::scan(&target).unwrap())
        .unwrap()
        .unwrap();
    let backup = archive::apply(&target, &bytes, &root.join("backups"))
        .unwrap()
        .unwrap();
    assert_eq!(archive::scan(&target).unwrap(), local);
    assert_eq!(fs::read(backup.join("level.dat")).unwrap(), b"old");
    assert!(!target.join("removed.dat").exists());
    assert!(!target.join("session.lock").exists());
    assert!(archive::delta(&source, &local, &local).unwrap().is_none());
}

#[test]
fn invalid_delete_and_traversal_never_modify_original() {
    let root = directory("invalid");
    let target = root.join("world");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("level.dat"), b"keep").unwrap();
    for bytes in [
        zip(&[("../outside", b"escape")]),
        zip(&[("level.dat", b"bad"), (DELETE_FILE, br#"["../outside"]"#)]),
        zip(&[("C:/escape", b"escape")]),
        zip(&[("LEVEL.DAT", b"one"), ("level.dat", b"two")]),
        zip(&[("region:ads", b"escape")]),
    ] {
        assert!(archive::apply(&target, &bytes, &root.join("backups")).is_err());
        assert_eq!(fs::read(target.join("level.dat")).unwrap(), b"keep");
    }
    assert!(!root.join("outside").exists());
}

#[test]
fn file_directory_conflict_rolls_back() {
    let root = directory("conflict");
    let target = root.join("world");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("region"), b"original").unwrap();
    assert!(
        archive::apply(
            &target,
            &zip(&[("region/r.0.0.mca", b"new")]),
            &root.join("backups")
        )
        .is_err()
    );
    assert_eq!(fs::read(target.join("region")).unwrap(), b"original");
}

#[test]
fn stale_snapshot_refuses_upload() {
    let root = directory("stale");
    fs::write(root.join("level.dat"), b"before").unwrap();
    let manifest = archive::scan(&root).unwrap();
    fs::write(root.join("level.dat"), b"after").unwrap();
    assert_eq!(
        archive::delta(&root, &manifest, &Manifest::new())
            .unwrap_err()
            .key,
        "err.cloudConflict"
    );
}

#[test]
fn portable_names_and_manifests_are_validated() {
    for name in [
        "", ".", "..", "CON", "com1.txt", "a/b", "a\\b", "a:", "a.", "a ",
    ] {
        assert!(archive::validate_name(name).is_err(), "{name}");
    }
    archive::validate_name("新的世界").unwrap();
    assert!(archive::validate_manifest(&Manifest::from([("a".into(), "bad".into())])).is_err());
}
