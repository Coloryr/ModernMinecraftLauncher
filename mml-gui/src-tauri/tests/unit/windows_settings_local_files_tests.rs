use super::*;

/// 清理只删内容：目录本身、图片缓存与界面数据都留着
#[test]
fn clear_removes_children_but_keeps_reserved_dirs() {
    let dir = mml_testutil::temp_dir().join(format!(
        "local_files_clear_{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let cache = LocalFileKind {
        id: "cache",
        dir: dir.clone(),
        deletable: true,
    };
    fs::create_dir_all(dir.join(names::IMAGE_DIR)).unwrap();
    fs::create_dir_all(dir.join(names::WEBVIEW_DIR)).unwrap();
    fs::create_dir_all(dir.join(names::TEMP_DIR)).unwrap();
    fs::write(dir.join("a.zip"), b"x").unwrap();
    fs::write(dir.join(names::IMAGE_DIR).join("i.png"), b"x").unwrap();
    fs::write(dir.join(names::WEBVIEW_DIR).join("db"), b"x").unwrap();
    fs::write(dir.join(names::TEMP_DIR).join("t"), b"x").unwrap();

    clear(&cache).unwrap();

    assert!(dir.exists());
    assert!(dir.join(names::IMAGE_DIR).join("i.png").exists());
    assert!(dir.join(names::WEBVIEW_DIR).join("db").exists());
    assert!(!dir.join("a.zip").exists());
    assert!(!dir.join(names::TEMP_DIR).exists());
    fs::remove_dir_all(&dir).unwrap();
}

/// 「缓存」的占用不含图片缓存与界面数据，否则几行数字加起来会重复计算
#[test]
fn cache_size_excludes_reserved_children() {
    let dir = mml_testutil::temp_dir().join(format!(
        "local_files_size_{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let cache = LocalFileKind {
        id: "cache",
        dir: dir.clone(),
        deletable: true,
    };
    fs::create_dir_all(dir.join(names::IMAGE_DIR)).unwrap();
    fs::create_dir_all(dir.join(names::WEBVIEW_DIR)).unwrap();
    fs::write(dir.join("a.zip"), vec![0u8; 300]).unwrap();
    fs::write(dir.join(names::IMAGE_DIR).join("i.png"), vec![0u8; 100]).unwrap();
    fs::write(dir.join(names::WEBVIEW_DIR).join("db"), vec![0u8; 200]).unwrap();

    assert_eq!(size_of(&cache), 300);

    fs::remove_dir_all(&dir).unwrap();
}

