//! 主题包判定与校验的用例
//!
//! 包样本都用 zip 写出来，覆盖三种类型、清单有无、目录兜底、各类坏包与限额；
//! 临时目录统一走 test_support，落在 target/temp 下。

use std::{
    io::Write,
    path::{Path, PathBuf},
};

use super::*;
use crate::err_keys;
use crate::windows::web_assets::{
    MAX_ARCHIVE_BYTES, MAX_ENTRIES, MAX_ENTRY_BYTES, MAX_TOTAL_BYTES, WebArchive, replace_atomic,
};

/// zip 中央目录签名
const CENTRAL: [u8; 4] = [0x50, 0x4B, 0x01, 0x02];
/// zip 本地文件头签名
const LOCAL: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];

/// 待写入 zip 的一个条目
struct Spec<'a> {
    /// 条目名，结尾带 / 表示目录
    name: &'a str,
    /// 条目内容
    data: &'a [u8],
    /// Unix 权限位，None 表示普通文件
    mode: Option<u32>,
}

/// 普通文件条目
fn file<'a>(name: &'a str, data: &'a [u8]) -> Spec<'a> {
    Spec {
        name,
        data,
        mode: None,
    }
}

/// 目录条目
fn folder(name: &str) -> Spec<'_> {
    Spec {
        name,
        data: b"",
        mode: Some(0o040755),
    }
}

/// 建一个用例专用临时目录并启动内核单例
fn case(tag: &str) -> PathBuf {
    crate::test_support::ensure_boot();
    crate::test_support::temp_dir(tag)
}

/// 按条目表写一个 zip
fn write_package(dir: &Path, file_name: &str, entries: &[Spec]) -> PathBuf {
    let path = dir.join(file_name);
    let handle = std::fs::File::create(&path).unwrap();
    let mut writer = zip::ZipWriter::new(handle);
    for spec in entries {
        let mut options = zip::write::SimpleFileOptions::default();
        if let Some(mode) = spec.mode {
            // unix_permissions 会丢掉高位的类型位，造符号链接 / 设备条目只能写原始属性
            let mut attributes = mode << 16;
            if spec.name.ends_with('/') {
                attributes |= 0x10;
            }
            options = options
                .external_attributes(attributes)
                .system(zip::System::Unix);
        }
        if spec.name.ends_with('/') {
            writer.add_directory(spec.name, options).unwrap();
        } else {
            writer.start_file(spec.name, options).unwrap();
            writer.write_all(spec.data).unwrap();
        }
    }
    writer.finish().unwrap();
    path
}

/// 写一个带 AES 加密条目的 zip
fn write_encrypted(dir: &Path, file_name: &str) -> PathBuf {
    let path = dir.join(file_name);
    let handle = std::fs::File::create(&path).unwrap();
    let mut writer = zip::ZipWriter::new(handle);
    let options = zip::write::SimpleFileOptions::default()
        .with_aes_encryption(zip::AesMode::Aes256, "secret");
    writer.start_file("index.html", options).unwrap();
    writer.write_all(b"<html></html>").unwrap();
    writer.finish().unwrap();
    path
}

/// 把文件里所有 needle 字节换成 0xFF，用来造非 UTF-8 条目名
fn corrupt_name(path: &Path, needle: &str) {
    let mut bytes = std::fs::read(path).unwrap();
    let marker = needle.as_bytes();
    let mut pos = 0;
    while pos + marker.len() <= bytes.len() {
        if &bytes[pos..pos + marker.len()] == marker {
            for byte in &mut bytes[pos..pos + marker.len()] {
                *byte = 0xFF;
            }
            pos += marker.len();
        } else {
            pos += 1;
        }
    }
    std::fs::write(path, bytes).unwrap();
}

/// 改写第 index 个条目的解压后大小，中央目录与本地头一起改
fn patch_size(path: &Path, index: usize, size: u32) {
    let mut bytes = std::fs::read(path).unwrap();
    let mut seen = 0;
    let mut local = None;
    for pos in 0..bytes.len().saturating_sub(46) {
        if bytes[pos..pos + 4] == CENTRAL {
            if seen == index {
                bytes[pos + 24..pos + 28].copy_from_slice(&size.to_le_bytes());
                let offset = u32::from_le_bytes(bytes[pos + 42..pos + 46].try_into().unwrap());
                local = Some(offset as usize);
                break;
            }
            seen += 1;
        }
    }
    let local = local.unwrap();
    assert_eq!(bytes[local..local + 4], LOCAL);
    bytes[local + 22..local + 26].copy_from_slice(&size.to_le_bytes());
    std::fs::write(path, bytes).unwrap();
}

// ================= 合法包 =================

#[test]
fn home_accepts_entry_variants() {
    let dir = case("theme-home");
    let html: &[u8] = b"<html></html>";
    let js: &[u8] = b"module.exports = 1;";
    let css: &[u8] = b"body{}";
    let cases: [&[Spec]; 4] = [
        &[file("index.html", html)],
        &[file("custom.js", js)],
        &[file("index.html", html), file("custom.js", js)],
        &[file("custom.js", js), file("custom.css", css)],
    ];
    for (index, entries) in cases.iter().enumerate() {
        let path = write_package(&dir, &format!("home-{index}.zip"), entries);
        let info = detect_theme(&path, Some(ThemeKind::Home)).unwrap();
        assert_eq!(info.kind, ThemeKind::Home);
        assert!(!info.declared);
    }
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn page_accepts_entry_variants() {
    let dir = case("theme-page");
    let html: &[u8] = b"<html></html>";
    let js: &[u8] = b"module.exports = 1;";
    let cases: [&[Spec]; 3] = [
        &[file("index.html", html)],
        &[file("custom.js", js)],
        &[file("index.html", html), file("custom.js", js)],
    ];
    for (index, entries) in cases.iter().enumerate() {
        let path = write_package(&dir, &format!("page-{index}.zip"), entries);
        let info = detect_theme(&path, Some(ThemeKind::Page)).unwrap();
        assert_eq!(info.kind, ThemeKind::Page);
        assert!(!info.declared);
    }
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn css_accepts_entry_variants() {
    let dir = case("theme-css");
    let css: &[u8] = b"body{}";
    let js: &[u8] = b"module.exports = 1;";
    let cases: [&[Spec]; 2] = [
        &[file("custom.css", css)],
        &[file("custom.css", css), file("custom.js", js)],
    ];
    for (index, entries) in cases.iter().enumerate() {
        let path = write_package(&dir, &format!("css-{index}.zip"), entries);
        let info = detect_theme(&path, Some(ThemeKind::Css)).unwrap();
        assert_eq!(info.kind, ThemeKind::Css);
    }
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn accepts_wrapped_directory() {
    let dir = case("theme-wrapped");
    let html: &[u8] = b"<html></html>";
    let path = write_package(
        &dir,
        "wrapped.zip",
        &[
            folder("demo/"),
            file("demo/index.html", html),
            file("demo/assets/app.js", b"app"),
        ],
    );
    let info = detect_theme(&path, Some(ThemeKind::Home)).unwrap();
    assert_eq!(info.kind, ThemeKind::Home);
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn manifest_declares_type_and_metadata() {
    let dir = case("theme-manifest");
    let manifest = r#"{"type":"home","name":"Coloryr 主页","version":"1.0.0","api":1}"#;
    let path = write_package(
        &dir,
        "declared.zip",
        &[file("theme.json", manifest.as_bytes()), file("index.html", b"x")],
    );
    let info = detect_theme(&path, Some(ThemeKind::Home)).unwrap();
    assert_eq!(info.kind, ThemeKind::Home);
    assert_eq!(info.name, "Coloryr 主页");
    assert_eq!(info.version, "1.0.0");
    assert!(info.declared);
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn manifest_in_wrapped_directory() {
    let dir = case("theme-manifest-wrapped");
    let manifest = r#"{"type":"css"}"#;
    let path = write_package(
        &dir,
        "wrapped.zip",
        &[
            folder("demo/"),
            file("demo/theme.json", manifest.as_bytes()),
            file("demo/custom.css", b"body{}"),
        ],
    );
    let info = detect_theme(&path, Some(ThemeKind::Css)).unwrap();
    assert_eq!(info.kind, ThemeKind::Css);
    assert!(info.declared);
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn legacy_package_uses_import_entry() {
    let dir = case("theme-legacy");
    let path = write_package(&dir, "legacy.zip", &[file("index.html", b"x")]);
    // 没有清单时 Home 与 Page 入口相同，按导入入口收下
    assert_eq!(
        detect_theme(&path, Some(ThemeKind::Home)).unwrap().kind,
        ThemeKind::Home
    );
    assert_eq!(
        detect_theme(&path, Some(ThemeKind::Page)).unwrap().kind,
        ThemeKind::Page
    );
    assert_eq!(
        detect_theme(&path, None).unwrap_err(),
        err_keys::THEME_TYPE_UNKNOWN
    );
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn case_insensitive_entry_accepted() {
    let dir = case("theme-case");
    let path = write_package(&dir, "case.zip", &[file("INDEX.HTML", b"x")]);
    let info = detect_theme(&path, Some(ThemeKind::Home)).unwrap();
    assert_eq!(info.kind, ThemeKind::Home);
    crate::test_support::cleanup(&[&dir]);
}

// ================= 清单与入口错误 =================

#[test]
fn manifest_type_mismatch_rejected() {
    let dir = case("theme-mismatch");
    let page = r#"{"type":"page"}"#;
    let path = write_package(
        &dir,
        "page.zip",
        &[file("theme.json", page.as_bytes()), file("index.html", b"x")],
    );
    assert_eq!(
        detect_theme(&path, Some(ThemeKind::Home)).unwrap_err(),
        err_keys::THEME_TYPE_MISMATCH
    );

    let css = r#"{"type":"css"}"#;
    let path = write_package(
        &dir,
        "css.zip",
        &[file("theme.json", css.as_bytes()), file("custom.css", b"x")],
    );
    assert_eq!(
        detect_theme(&path, Some(ThemeKind::Page)).unwrap_err(),
        err_keys::THEME_TYPE_MISMATCH
    );
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn manifest_entry_missing() {
    let dir = case("theme-manifest-no-entry");
    let manifest = r#"{"type":"home"}"#;
    let path = write_package(
        &dir,
        "no-entry.zip",
        &[file("theme.json", manifest.as_bytes()), file("custom.css", b"x")],
    );
    assert_eq!(
        detect_theme(&path, Some(ThemeKind::Home)).unwrap_err(),
        err_keys::THEME_MISSING_ENTRY
    );
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn manifest_invalid_rejected() {
    let dir = case("theme-manifest-invalid");
    let manifests = [
        r#"{"type":"home",}"#,
        r#"{}"#,
        r#"[]"#,
        r#"{"type":"skin"}"#,
        r#"{"type":"home","api":2}"#,
    ];
    for (index, manifest) in manifests.iter().enumerate() {
        let path = write_package(
            &dir,
            &format!("bad-{index}.zip"),
            &[file("theme.json", manifest.as_bytes()), file("index.html", b"x")],
        );
        assert_eq!(
            detect_theme(&path, Some(ThemeKind::Home)).unwrap_err(),
            err_keys::THEME_MANIFEST_INVALID,
            "manifest {index}"
        );
    }
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn manifest_too_large_rejected() {
    let dir = case("theme-manifest-big");
    // 填充物要够随机，否则压缩比先超限，报到 err.themeZipRatio 上
    let filler: String = (0..70 * 1024)
        .map(|index| char::from(b'!' + (index * 37 % 90) as u8))
        .collect();
    let manifest = format!(r#"{{"type":"home","name":"{filler}"}}"#);
    let path = write_package(
        &dir,
        "big.zip",
        &[file("theme.json", manifest.as_bytes()), file("index.html", b"x")],
    );
    assert_eq!(
        detect_theme(&path, Some(ThemeKind::Home)).unwrap_err(),
        err_keys::THEME_MANIFEST_INVALID
    );
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn css_only_package_rejected_by_home_and_page() {
    let dir = case("theme-css-only");
    let path = write_package(&dir, "css.zip", &[file("custom.css", b"body{}")]);
    assert_eq!(
        detect_theme(&path, Some(ThemeKind::Home)).unwrap_err(),
        err_keys::THEME_TYPE_MISMATCH
    );
    assert_eq!(
        detect_theme(&path, Some(ThemeKind::Page)).unwrap_err(),
        err_keys::THEME_TYPE_MISMATCH
    );
    assert_eq!(
        detect_theme(&path, None).unwrap().kind,
        ThemeKind::Css
    );
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn missing_entry_rejected() {
    let dir = case("theme-no-entry");
    let path = write_package(&dir, "empty-ish.zip", &[file("readme.txt", b"hi")]);
    assert_eq!(
        detect_theme(&path, Some(ThemeKind::Home)).unwrap_err(),
        err_keys::THEME_MISSING_ENTRY
    );
    assert_eq!(
        detect_theme(&path, Some(ThemeKind::Css)).unwrap_err(),
        err_keys::THEME_MISSING_ENTRY
    );
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn directory_entry_is_not_an_entry() {
    let dir = case("theme-dir-entry");
    let path = write_package(&dir, "dir.zip", &[folder("index.html/")]);
    assert_eq!(
        detect_theme(&path, Some(ThemeKind::Home)).unwrap_err(),
        err_keys::THEME_MISSING_ENTRY
    );
    crate::test_support::cleanup(&[&dir]);
}

// ================= 坏包 =================

#[test]
fn non_zip_rejected() {
    let dir = case("theme-not-zip");
    let missing = dir.join("missing.zip");
    assert_eq!(
        detect_theme(&missing, Some(ThemeKind::Home)).unwrap_err(),
        err_keys::THEME_ZIP_INVALID
    );

    let empty = dir.join("empty.zip");
    std::fs::write(&empty, b"").unwrap();
    assert_eq!(
        detect_theme(&empty, Some(ThemeKind::Home)).unwrap_err(),
        err_keys::THEME_ZIP_INVALID
    );

    let garbage = dir.join("garbage.zip");
    std::fs::write(&garbage, b"not a zip").unwrap();
    assert_eq!(
        detect_theme(&garbage, Some(ThemeKind::Home)).unwrap_err(),
        err_keys::THEME_ZIP_INVALID
    );

    let good = write_package(&dir, "good.zip", &[file("index.html", b"x")]);
    let bytes = std::fs::read(&good).unwrap();
    let truncated = dir.join("truncated.zip");
    std::fs::write(&truncated, &bytes[..bytes.len() / 2]).unwrap();
    assert_eq!(
        detect_theme(&truncated, Some(ThemeKind::Home)).unwrap_err(),
        err_keys::THEME_ZIP_INVALID
    );
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn encrypted_entry_rejected() {
    let dir = case("theme-encrypted");
    let path = write_encrypted(&dir, "encrypted.zip");
    assert_eq!(
        detect_theme(&path, Some(ThemeKind::Home)).unwrap_err(),
        err_keys::THEME_ZIP_ENCRYPTED
    );
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn bad_entry_path_rejected() {
    let dir = case("theme-path");
    let names = [
        "../evil.txt",
        "/etc/passwd",
        "C:/Windows/win.ini",
        "a/../../b.txt",
        "dir\\..\\evil.txt",
        "bad\u{0}name.txt",
        "bad\tname.txt",
    ];
    for (index, name) in names.iter().enumerate() {
        let path = write_package(&dir, &format!("path-{index}.zip"), &[file(name, b"x")]);
        assert_eq!(
            detect_theme(&path, Some(ThemeKind::Home)).unwrap_err(),
            err_keys::THEME_ZIP_ENTRY_PATH,
            "entry {name}"
        );
    }
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn invalid_utf8_entry_name_rejected() {
    let dir = case("theme-utf8");
    let path = write_package(&dir, "utf8.zip", &[file("INVALID.txt", b"x")]);
    corrupt_name(&path, "INVALID.txt");
    assert_eq!(
        detect_theme(&path, Some(ThemeKind::Home)).unwrap_err(),
        err_keys::THEME_ZIP_INVALID
    );
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn duplicate_entry_names_rejected() {
    let dir = case("theme-duplicate");
    let case_dup = write_package(
        &dir,
        "case.zip",
        &[file("A.png", b"1"), file("a.png", b"2")],
    );
    assert_eq!(
        detect_theme(&case_dup, Some(ThemeKind::Home)).unwrap_err(),
        err_keys::THEME_ZIP_DUPLICATE
    );

    let norm_dup = write_package(
        &dir,
        "norm.zip",
        &[file("a/b.txt", b"1"), file("a//b.txt", b"2")],
    );
    assert_eq!(
        detect_theme(&norm_dup, Some(ThemeKind::Home)).unwrap_err(),
        err_keys::THEME_ZIP_DUPLICATE
    );
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn unsupported_entry_type_rejected() {
    let dir = case("theme-unsupported");
    let modes = [0o120777, 0o010644, 0o020644, 0o060644, 0o140644];
    for (index, mode) in modes.iter().enumerate() {
        let path = write_package(
            &dir,
            &format!("mode-{index}.zip"),
            &[Spec {
                name: "entry.bin",
                data: b"x",
                mode: Some(*mode),
            }],
        );
        assert_eq!(
            detect_theme(&path, Some(ThemeKind::Home)).unwrap_err(),
            err_keys::THEME_ZIP_UNSUPPORTED,
            "mode {mode:o}"
        );
    }
    crate::test_support::cleanup(&[&dir]);
}

// ================= 限额 =================

#[test]
fn entry_count_limit() {
    let dir = case("theme-count");
    let names: Vec<String> = (0..=MAX_ENTRIES).map(|index| format!("f{index}.txt")).collect();
    let entries: Vec<Spec> = names.iter().map(|name| file(name, b"")).collect();
    let path = write_package(&dir, "many.zip", &entries);
    assert_eq!(
        detect_theme(&path, Some(ThemeKind::Home)).unwrap_err(),
        err_keys::PAGE_ARCHIVE_LIMIT
    );
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn entry_size_limit() {
    let dir = case("theme-entry-size");
    let path = write_package(&dir, "big-entry.zip", &[file("index.html", b"x")]);
    patch_size(&path, 0, (MAX_ENTRY_BYTES + 1) as u32);
    assert_eq!(
        detect_theme(&path, Some(ThemeKind::Home)).unwrap_err(),
        err_keys::PAGE_ARCHIVE_LIMIT
    );
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn total_size_limit() {
    let dir = case("theme-total-size");
    let path = write_package(
        &dir,
        "total.zip",
        &[
            file("index.html", b"1"),
            file("a.bin", b"2"),
            file("b.bin", b"3"),
            file("c.bin", b"4"),
            file("d.bin", b"5"),
        ],
    );
    // 每条 120 MiB：单条没超，五条合计超过 512 MiB
    for index in 0..5 {
        patch_size(&path, index, 120 * 1024 * 1024);
    }
    assert!(120 * 1024 * 1024 < MAX_ENTRY_BYTES);
    assert!(5 * 120 * 1024 * 1024 > MAX_TOTAL_BYTES);
    assert_eq!(
        detect_theme(&path, Some(ThemeKind::Home)).unwrap_err(),
        err_keys::PAGE_ARCHIVE_LIMIT
    );
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn archive_size_limit() {
    let dir = case("theme-archive-size");
    let path = dir.join("huge.zip");
    std::fs::File::create(&path)
        .unwrap()
        .set_len(MAX_ARCHIVE_BYTES + 1)
        .unwrap();
    assert_eq!(
        detect_theme(&path, Some(ThemeKind::Home)).unwrap_err(),
        err_keys::PAGE_ARCHIVE_LIMIT
    );
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn compression_ratio_limit() {
    let dir = case("theme-ratio");
    let data = vec![0u8; 1024 * 1024];
    let path = write_package(&dir, "bomb.zip", &[file("index.html", &data)]);
    assert_eq!(
        detect_theme(&path, Some(ThemeKind::Home)).unwrap_err(),
        err_keys::THEME_ZIP_RATIO
    );
    crate::test_support::cleanup(&[&dir]);
}

// ================= 原子替换与状态读回 =================

#[test]
fn replace_atomic_keeps_old_on_failure() {
    let dir = case("theme-replace");
    let target = dir.join("package.zip");
    let old = write_package(&dir, "old.zip", &[file("index.html", b"old")]);
    std::fs::copy(&old, &target).unwrap();

    let missing = dir.join("missing.zip");
    assert!(replace_atomic(&missing, &target).is_err());
    let archive = WebArchive::open(&target).unwrap();
    assert_eq!(archive.read("index.html").unwrap(), b"old".to_vec());
    assert!(!target.with_file_name("package.zip.tmp").exists());

    let new = write_package(&dir, "new.zip", &[file("index.html", b"new")]);
    replace_atomic(&new, &target).unwrap();
    let archive = WebArchive::open(&target).unwrap();
    assert_eq!(archive.read("index.html").unwrap(), b"new".to_vec());
    assert!(!target.with_file_name("package.zip.tmp").exists());
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn meta_of_archive_prefers_manifest() {
    let dir = case("theme-meta");
    let manifest = r#"{"type":"page","name":"整页","version":"2.0"}"#;
    let path = write_package(
        &dir,
        "declared.zip",
        &[file("theme.json", manifest.as_bytes()), file("index.html", b"x")],
    );
    let archive = WebArchive::open(&path).unwrap();
    let meta = meta_of_archive(&archive, ThemeKind::Home);
    assert_eq!(meta.kind, ThemeKind::Page);
    assert_eq!(meta.name, "整页");
    assert_eq!(meta.version, "2.0");
    assert!(meta.declared);

    let legacy = write_package(&dir, "legacy.zip", &[file("index.html", b"x")]);
    let archive = WebArchive::open(&legacy).unwrap();
    let meta = meta_of_archive(&archive, ThemeKind::Page);
    assert_eq!(meta.kind, ThemeKind::Page);
    assert!(meta.name.is_empty());
    assert!(!meta.declared);
    crate::test_support::cleanup(&[&dir]);
}
