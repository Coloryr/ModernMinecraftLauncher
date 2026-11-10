use std::fs;

use uuid::Uuid;

use crate::archives::base_archive::util::replace_archive;
use crate::archives::{ArchiveType, base_archive::BaseArchive};

#[test]
fn replacement_failure_keeps_original() {
    let root = mml_testutil::temp_path(format!("archive_replace_{}", Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let original = root.join("original.tar.gz");
    fs::write(&original, b"original").unwrap();
    assert!(replace_archive(&root.join("missing.tmp"), &original).is_err());
    assert_eq!(fs::read(&original).unwrap(), b"original");
    let replacement = root.join("replacement.tmp");
    fs::write(&replacement, b"replacement").unwrap();
    replace_archive(&replacement, &original).unwrap();
    assert_eq!(fs::read(&original).unwrap(), b"replacement");
    assert!(!replacement.exists());
}

#[test]
fn created_archive_opens_by_type_even_with_temp_file_name() {
    let root = mml_testutil::temp_path(format!("archive_part_{}", Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    // 导出写的是 xxx.part：扩展名认不出格式，仍要能按 zip 继续写
    let path = root.join("pack.zip.1234.part");
    let mut archive = BaseArchive::create_empty(ArchiveType::Zip, &path).unwrap();
    archive.add_data("manifest.json", b"{}").unwrap();
    assert_eq!(archive.entries().len(), 1);
    drop(archive);
    let reopened = BaseArchive::open_readonly_as(&path, ArchiveType::Zip).unwrap();
    assert_eq!(reopened.entries().len(), 1);
    fs::remove_dir_all(&root).unwrap();
}
