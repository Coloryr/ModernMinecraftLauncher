use std::path::PathBuf;

use mml_base::archives::{ArchiveType, BaseArchive};

use super::*;

/// 路径穿越：`..` 段、反斜杠与冒号一律拒绝
#[test]
fn rejects_unsafe_relative_paths() {
    assert!(is_safe_rel("assets/app.js"));
    assert!(is_safe_rel("index.html"));
    assert!(!is_safe_rel("../secret.txt"));
    assert!(!is_safe_rel("a/../../b"));
    assert!(!is_safe_rel("a\\b.js"));
    assert!(!is_safe_rel("C:/Windows/win.ini"));
    assert!(!is_safe_rel("a\0b"));
}

/// 编码后的穿越要在解码之后再过一次检查
#[test]
fn percent_decode_restores_bytes() {
    assert_eq!(percent_decode("..%2fsecret"), "../secret");
    assert_eq!(percent_decode("100%.png"), "100%.png");
}

/// 造一个只有入口页与一个资源文件的整页包
fn test_archive() -> (PathBuf, WebArchive) {
    let dir = mml_testutil::temp_path(format!("web-assets-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("建临时目录");
    let zip = dir.join("page.zip");
    let mut archive = BaseArchive::create_empty(ArchiveType::Zip, &zip).expect("建包");
    archive
        .add_data("index.html", b"<html></html>")
        .expect("写入口页");
    archive
        .add_data("assets/app.js", b"console.log(1)")
        .expect("写资源");
    drop(archive);
    let opened = WebArchive::open(&zip).expect("打开包");
    (dir, opened)
}

/// 整包套一层目录时，入口页、子资源与入口脚本都能解析
#[test]
fn resolves_entries_inside_single_top_dir() {
    let dir = mml_testutil::temp_path(format!("web-wrapped-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("建临时目录");
    let zip = dir.join("wrapped.zip");
    let mut archive = BaseArchive::create_empty(ArchiveType::Zip, &zip).expect("建包");
    archive
        .add_data("demo/index.html", b"<html></html>")
        .expect("写入口页");
    archive
        .add_data("demo/assets/app.js", b"console.log(1)")
        .expect("写资源");
    archive
        .add_data("demo/custom.js", b"console.log(2)")
        .expect("写入口脚本");
    drop(archive);
    let archive = WebArchive::open(&zip).expect("打开包");

    assert_eq!(
        resolve_entry(&archive, "/", "index.html"),
        Some("demo/index.html")
    );
    assert_eq!(
        resolve_entry(&archive, "/assets/app.js", "index.html"),
        Some("demo/assets/app.js")
    );
    assert_eq!(
        resolve_entry(&archive, "/custom.js", "custom.css"),
        Some("demo/custom.js")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// 只认包内条目：根路径落到入口页，越界与不存在的路径都返回 None
#[test]
fn resolves_only_entries_inside_archive() {
    let (dir, archive) = test_archive();
    assert_eq!(
        resolve_entry(&archive, "/", "index.html"),
        Some("index.html")
    );
    assert_eq!(
        resolve_entry(&archive, "/assets/app.js", "index.html"),
        Some("assets/app.js")
    );
    assert_eq!(
        resolve_entry(&archive, "/%2e%2e/secret.txt", "index.html"),
        None
    );
    assert_eq!(
        resolve_entry(&archive, "/../secret.txt", "index.html"),
        None
    );
    assert_eq!(resolve_entry(&archive, "/missing.js", "index.html"), None);
    let _ = std::fs::remove_dir_all(&dir);
}

/// 扩展名决定 MIME，未知类型一律二进制流
#[test]
fn mime_follows_extension() {
    assert_eq!(mime_of("index.html"), "text/html");
    assert_eq!(mime_of("a.WOFF2"), "font/woff2");
    assert_eq!(mime_of("blob"), "application/octet-stream");
}

