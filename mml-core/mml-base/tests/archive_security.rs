//! 归档解压的目录边界回归测试。

use mml_base::archives::{ArchiveType, BaseArchive};
use std::{fs, io::Cursor, path::PathBuf};

fn root(case: &str) -> PathBuf {
    let root = mml_testutil::temp_path(format!("archive_{case}_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn seven_zip_rejects_escaping_paths() {
    for name in [
        "../escaped.txt",
        "/escaped.txt",
        "\\escaped.txt",
        "C:/escaped.txt",
        "nested/../../escaped.txt",
    ] {
        let root = root("7z_escape");
        let archive = root.join("input.7z");
        let mut writer = sevenz_rust2::ArchiveWriter::create(&archive).unwrap();
        let mut entry = sevenz_rust2::ArchiveEntry::new();
        entry.name = name.into();
        entry.has_stream = true;
        writer
            .push_archive_entry(entry, Some(Cursor::new(b"payload")))
            .unwrap();
        writer.finish().unwrap();
        assert!(
            BaseArchive::decompress(ArchiveType::R7Z, &archive, &root.join("out"), None).is_err(),
            "{name}"
        );
        assert!(!root.join("escaped.txt").exists());
    }
}
