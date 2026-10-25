//! 主题包判定与校验的集成测试
//!
//! 包样本都用 zip 写出来，覆盖三种类型、清单有无、目录兜底、各类坏包与限额。
//! 判定与整包句柄是窗口模块的内部实现，经 `windows::test_api` 再导出取用；
//! 临时目录落在仓库 target/temp 下，内核单例在本进程内只 boot 一次。

use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::Once,
};

use mml_gui_lib::err_keys;
use mml_gui_lib::windows::test_api::{
    MAX_ARCHIVE_BYTES, MAX_ENTRIES, MAX_ENTRY_BYTES, MAX_TOTAL_BYTES, ThemeKind, WebArchive,
    detect_theme, meta_of_archive, replace_atomic,
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

/// 启动内核单例，进程内只执行一次
///
/// `mml_log` 的 `STREAM` 是 `OnceLock`，第二次 `start()` 会 panic，所以整份用例共用这一次。
/// 集成测试与库里的单测编在不同进程，各自的 boot 互不影响。
fn ensure_boot() {
    static BOOT: Once = Once::new();
    BOOT.call_once(|| {
        let dir = mml_testutil::temp_path(format!("mml-gui-theme-boot-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        mml_base::init(&dir);
        mml_names::init(&dir).unwrap();
        mml_log::start(&dir).unwrap();
    });
}

/// 一个干净的测试临时目录，建在 target/temp 下，按 tag 加随机名分开
///
/// - `tag`: 用例特征名，用来在临时目录里认出是谁的
fn temp_dir(tag: &str) -> PathBuf {
    let dir = mml_testutil::temp_path(format!("mml-gui-{tag}-{}", uuid::Uuid::new_v4()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 用完清掉临时目录，用例失败时留着现场方便查
///
/// - `dirs`: 要删的目录列表，删不掉就忽略
fn cleanup(dirs: &[&Path]) {
    for dir in dirs {
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// 建一个用例专用临时目录并启动内核单例
fn case(tag: &str) -> PathBuf {
    ensure_boot();
    temp_dir(tag)
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

/// 写一个标着"已加密"的 zip
///
/// 依赖里已经去掉 zip 的 AES 支持，这里改置通用标志的第 0 位：校验只看这个标志，
/// 不需要真的加密数据。
fn write_encrypted(dir: &Path, file_name: &str) -> PathBuf {
    let path = dir.join(file_name);
    {
        let handle = std::fs::File::create(&path).unwrap();
        let mut writer = zip::ZipWriter::new(handle);
        writer
            .start_file("index.html", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"<html></html>").unwrap();
        writer.finish().unwrap();
    }

    // 本地文件头与中央目录各存一份标志位，条目的 encrypted() 读的是后者
    let mut bytes = std::fs::read(&path).unwrap();
    for (signature, offset) in [
        (b"PK\x03\x04".as_slice(), 6usize),
        (b"PK\x01\x02".as_slice(), 8usize),
    ] {
        let mut from = 0;
        while let Some(pos) = bytes[from..]
            .windows(signature.len())
            .position(|window| window == signature)
        {
            bytes[from + pos + offset] |= 1;
            from += pos + signature.len();
        }
    }
    std::fs::write(&path, &bytes).unwrap();
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
    cleanup(&[&dir]);
}

#[test]
fn page_accepts_entry_variants() {
    let dir = case("theme-page");
    let html: &[u8] = b"<html></html>";
    let js: &[u8] = b"module.exports = 1;";
    let css: &[u8] = b"body{}";
    let cases: [&[Spec]; 4] = [
        &[file("index.html", html)],
        &[file("custom.js", js)],
        &[file("index.html", html), file("custom.js", js)],
        &[file("index.html", html), file("custom.css", css)],
    ];
    for (index, entries) in cases.iter().enumerate() {
        let path = write_package(&dir, &format!("page-{index}.zip"), entries);
        let info = detect_theme(&path, Some(ThemeKind::Page)).unwrap();
        assert_eq!(info.kind, ThemeKind::Page);
        assert!(!info.declared);
    }
    cleanup(&[&dir]);
}

#[test]
fn css_accepts_entry_variants() {
    let dir = case("theme-css");
    let css: &[u8] = b"body{}";
    let js: &[u8] = b"module.exports = 1;";
    // 纯样式包没有清单也收
    let plain = write_package(&dir, "css-0.zip", &[file("custom.css", css)]);
    assert_eq!(
        detect_theme(&plain, Some(ThemeKind::Css)).unwrap().kind,
        ThemeKind::Css
    );
    // 带 custom.js 的与主页面 / 主题包重合，必须靠清单声明类型
    let manifest = r#"{"type":"css"}"#;
    let scripted = write_package(
        &dir,
        "css-1.zip",
        &[
            file("theme.json", manifest.as_bytes()),
            file("custom.css", css),
            file("custom.js", js),
        ],
    );
    let info = detect_theme(&scripted, Some(ThemeKind::Css)).unwrap();
    assert_eq!(info.kind, ThemeKind::Css);
    assert!(info.declared);
    cleanup(&[&dir]);
}

#[test]
fn unmarked_theme_package_rejected_as_css() {
    let dir = case("theme-css-ambiguous");
    let html = write_package(
        &dir,
        "home-page.zip",
        &[file("index.html", b"x"), file("custom.css", b"y")],
    );
    assert_eq!(
        detect_theme(&html, Some(ThemeKind::Css)).unwrap_err(),
        err_keys::THEME_TYPE_UNKNOWN
    );
    let component = write_package(
        &dir,
        "home-component.zip",
        &[file("custom.js", b"x"), file("custom.css", b"y")],
    );
    assert_eq!(
        detect_theme(&component, Some(ThemeKind::Css)).unwrap_err(),
        err_keys::THEME_TYPE_UNKNOWN
    );
    cleanup(&[&dir]);
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
    cleanup(&[&dir]);
}

#[test]
fn manifest_declares_type_and_metadata() {
    let dir = case("theme-manifest");
    let manifest = r#"{"type":"home","name":"Coloryr 主页","version":"1.0.0","api":1}"#;
    let path = write_package(
        &dir,
        "declared.zip",
        &[
            file("theme.json", manifest.as_bytes()),
            file("index.html", b"x"),
        ],
    );
    let info = detect_theme(&path, Some(ThemeKind::Home)).unwrap();
    assert_eq!(info.kind, ThemeKind::Home);
    assert_eq!(info.name, "Coloryr 主页");
    assert_eq!(info.version, "1.0.0");
    assert!(info.declared);
    cleanup(&[&dir]);
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
    cleanup(&[&dir]);
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
    cleanup(&[&dir]);
}

#[test]
fn case_insensitive_entry_accepted() {
    let dir = case("theme-case");
    let path = write_package(&dir, "case.zip", &[file("INDEX.HTML", b"x")]);
    let info = detect_theme(&path, Some(ThemeKind::Home)).unwrap();
    assert_eq!(info.kind, ThemeKind::Home);
    cleanup(&[&dir]);
}

// ================= 清单与入口错误 =================

#[test]
fn manifest_type_mismatch_rejected() {
    let dir = case("theme-mismatch");
    let page = r#"{"type":"page"}"#;
    let path = write_package(
        &dir,
        "page.zip",
        &[
            file("theme.json", page.as_bytes()),
            file("index.html", b"x"),
        ],
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
    cleanup(&[&dir]);
}

#[test]
fn manifest_entry_missing() {
    let dir = case("theme-manifest-no-entry");
    let manifest = r#"{"type":"home"}"#;
    let path = write_package(
        &dir,
        "no-entry.zip",
        &[
            file("theme.json", manifest.as_bytes()),
            file("custom.css", b"x"),
        ],
    );
    assert_eq!(
        detect_theme(&path, Some(ThemeKind::Home)).unwrap_err(),
        err_keys::THEME_MISSING_ENTRY
    );
    cleanup(&[&dir]);
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
            &[
                file("theme.json", manifest.as_bytes()),
                file("index.html", b"x"),
            ],
        );
        assert_eq!(
            detect_theme(&path, Some(ThemeKind::Home)).unwrap_err(),
            err_keys::THEME_MANIFEST_INVALID,
            "manifest {index}"
        );
    }
    cleanup(&[&dir]);
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
        &[
            file("theme.json", manifest.as_bytes()),
            file("index.html", b"x"),
        ],
    );
    assert_eq!(
        detect_theme(&path, Some(ThemeKind::Home)).unwrap_err(),
        err_keys::THEME_MANIFEST_INVALID
    );
    cleanup(&[&dir]);
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
    assert_eq!(detect_theme(&path, None).unwrap().kind, ThemeKind::Css);
    cleanup(&[&dir]);
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
    cleanup(&[&dir]);
}

#[test]
fn directory_entry_is_not_an_entry() {
    let dir = case("theme-dir-entry");
    let path = write_package(&dir, "dir.zip", &[folder("index.html/")]);
    assert_eq!(
        detect_theme(&path, Some(ThemeKind::Home)).unwrap_err(),
        err_keys::THEME_MISSING_ENTRY
    );
    cleanup(&[&dir]);
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
    cleanup(&[&dir]);
}

#[test]
fn encrypted_entry_rejected() {
    let dir = case("theme-encrypted");
    let path = write_encrypted(&dir, "encrypted.zip");
    assert_eq!(
        detect_theme(&path, Some(ThemeKind::Home)).unwrap_err(),
        err_keys::THEME_ZIP_ENCRYPTED
    );
    cleanup(&[&dir]);
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
    cleanup(&[&dir]);
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
    cleanup(&[&dir]);
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
    cleanup(&[&dir]);
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
    cleanup(&[&dir]);
}

// ================= 限额 =================

#[test]
fn entry_count_limit() {
    let dir = case("theme-count");
    let names: Vec<String> = (0..=MAX_ENTRIES)
        .map(|index| format!("f{index}.txt"))
        .collect();
    let entries: Vec<Spec> = names.iter().map(|name| file(name, b"")).collect();
    let path = write_package(&dir, "many.zip", &entries);
    assert_eq!(
        detect_theme(&path, Some(ThemeKind::Home)).unwrap_err(),
        err_keys::PAGE_ARCHIVE_LIMIT
    );
    cleanup(&[&dir]);
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
    cleanup(&[&dir]);
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
    cleanup(&[&dir]);
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
    cleanup(&[&dir]);
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
    cleanup(&[&dir]);
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
    cleanup(&[&dir]);
}

#[test]
fn opens_archive_without_extension() {
    let dir = case("theme-no-ext");
    // 自定义主页面存成不带扩展名的 home，打开时不能靠后缀判断格式
    let path = write_package(&dir, "home", &[file("index.html", b"x")]);
    let archive = WebArchive::open(&path).unwrap();
    assert_eq!(archive.read("index.html").unwrap(), b"x".to_vec());
    cleanup(&[&dir]);
}

#[test]
fn replace_atomic_while_handle_is_open() {
    let dir = case("theme-replace-open");
    let target = dir.join("package.zip");
    let old = write_package(&dir, "old.zip", &[file("index.html", b"old")]);
    std::fs::copy(&old, &target).unwrap();
    // 常驻句柄按整包只读打开：替换会覆盖正在被读的目标文件，不能因占用失败
    let held = WebArchive::open(&target).unwrap();
    let new = write_package(&dir, "new.zip", &[file("index.html", b"new")]);
    replace_atomic(&new, &target).unwrap();
    // 旧句柄仍指向被覆盖前的内容，重开之后才是新包
    assert_eq!(held.read("index.html").unwrap(), b"old".to_vec());
    let reopened = WebArchive::open(&target).unwrap();
    assert_eq!(reopened.read("index.html").unwrap(), b"new".to_vec());
    cleanup(&[&dir]);
}

#[test]
fn meta_of_archive_prefers_manifest() {
    let dir = case("theme-meta");
    let manifest = r#"{"type":"page","name":"整页","version":"2.0"}"#;
    let path = write_package(
        &dir,
        "declared.zip",
        &[
            file("theme.json", manifest.as_bytes()),
            file("index.html", b"x"),
        ],
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
    cleanup(&[&dir]);
}
