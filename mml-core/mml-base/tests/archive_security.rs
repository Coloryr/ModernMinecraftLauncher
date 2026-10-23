//! 归档解压的目录边界回归测试。

use mml_base::archives::{ArchiveType, BaseArchive};
use std::{
    fs,
    io::{Cursor, Write},
    path::PathBuf,
};

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

#[test]
fn tar_rename_stays_inside_output() {
    for name in [
        "../escaped?.txt",
        "/escaped?.txt",
        "nested/../../escaped?.txt",
        "nested/valid?.txt",
    ] {
        let root = root("tar_rename");
        let archive = root.join("input.tar");
        let mut writer = tar::Builder::new(fs::File::create(&archive).unwrap());
        let mut header = tar::Header::new_gnu();
        header.set_size(7);
        header.set_mode(0o644);
        header.set_entry_type(tar::EntryType::Regular);
        header.as_mut_bytes()[..name.len()].copy_from_slice(name.as_bytes());
        header.set_cksum();
        writer.append(&header, Cursor::new(b"payload")).unwrap();
        writer.finish().unwrap();
        drop(writer);
        let out = root.join("out");
        let result = BaseArchive::decompress(ArchiveType::Tar, &archive, &out, None);
        if name == "nested/valid?.txt" {
            result.unwrap();
            assert_eq!(fs::read(out.join("nested/valid_.txt")).unwrap(), b"payload");
        } else {
            assert!(result.is_err(), "{name}");
            assert!(!root.join("escaped_.txt").exists());
        }
    }
}

#[test]
fn zip_rejects_escaping_symlink_targets() {
    for target in ["../outside", "/outside", "C:/outside", "\\outside"] {
        let root = root("zip_link");
        let archive = root.join("input.zip");
        let mut writer = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
        writer
            .add_symlink("link", target, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.finish().unwrap();
        let result = BaseArchive::decompress(ArchiveType::Zip, &archive, &root.join("out"), None);
        assert!(
            matches!(
                result,
                Err(mml_names::i18_items::error_type::ErrorType::UnsafePath { .. })
            ),
            "{target}: {result:?}"
        );
        assert!(!root.join("out/link").exists());
    }
}

#[test]
fn filtered_extraction_rejects_escaping_paths() {
    for strip in [false, true] {
        let root = root("filtered_escape");
        let archive = root.join("input.zip");
        let mut writer = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
        writer
            .start_file("../escaped.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"payload").unwrap();
        writer.finish().unwrap();
        let archive = BaseArchive::open_readonly(archive).unwrap();
        let result = archive.extract_all(
            root.join("out"),
            (!strip).then(Vec::new),
            strip.then(|| "wrapped".into()),
            None,
        );
        assert!(result.is_err(), "strip={strip}: {result:?}");
        assert!(!root.join("escaped.txt").exists());
    }
}
