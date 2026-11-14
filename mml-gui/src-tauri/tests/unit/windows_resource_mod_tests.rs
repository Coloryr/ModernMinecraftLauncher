use super::check_parent;
use super::image_mime;
use super::mods::{mod_note, mod_note_key};
use std::collections::HashMap;
use std::path::{MAIN_SEPARATOR, Path};

/// 按魔术字节认 MIME；认不出来的一律 png，PNG 自己的魔数也走这条
#[test]
fn test_image_mime() {
    assert_eq!(
        image_mime(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]),
        "image/png"
    );
    assert_eq!(image_mime(&[0xFF, 0xD8, 0xFF, 0xE0]), "image/jpeg");
    assert_eq!(image_mime(b"GIF89a..."), "image/gif");
    assert_eq!(image_mime(b"GIF87a..."), "image/gif");
    assert_eq!(image_mime(b"BM\x00\x00"), "image/bmp");
    assert_eq!(image_mime(b"RIFF\x00\x00\x00\x00WEBPVP8 "), "image/webp");
    // 太短 / 认不出来：退回 png，不 panic
    assert_eq!(image_mime(b""), "image/png");
    assert_eq!(image_mime(b"RIFF"), "image/png");
    assert_eq!(image_mime(b"\x00\x01\x02\x03"), "image/png");
}

/// 存档目录名只收**纯名字**：带分隔符 / 上跳 / 空白一律拒绝，这是数据包落点的路径穿越防线
#[test]
fn test_check_parent() {
    assert_eq!(check_parent("New World").unwrap(), Path::new("New World"));
    // 前后空白不算名字的一部分
    assert_eq!(
        check_parent("  New World  ").unwrap(),
        Path::new("New World")
    );

    let nested = format!("a{MAIN_SEPARATOR}b");
    let bad = [
        "",
        "   ",
        ".",
        "..",
        "../x",
        &nested,
        &format!("..{MAIN_SEPARATOR}..{MAIN_SEPARATOR}x"),
    ];
    for item in bad {
        assert!(check_parent(item).is_err(), "{item:?} 不该通过");
    }
    // 反斜杠只有在 Windows 上才是分隔符；macOS / Linux 上它是普通文件名字符
    #[cfg(windows)]
    assert!(check_parent("..\\x").is_err(), "..\\x 不该通过");
}

/// 备注的键：禁用后缀要剥掉，这样启用 / 禁用来回切时备注才是同一份
#[test]
fn test_mod_note_key() {
    assert_eq!(mod_note_key("sodium.jar"), "sodium.jar");
    assert_eq!(mod_note_key("sodium.jar.disabled"), "sodium.jar");
    assert_eq!(mod_note_key("sodium.jar.disable"), "sodium.jar");
    // 名字里本来就带 disabled 的不许误伤，只认结尾
    assert_eq!(mod_note_key("disabled.jar"), "disabled.jar");
}

/// 读备注：归一键优先，退化到原始文件名，即 ColorMC 在禁用状态下写的那个
#[test]
fn test_mod_note() {
    let mut notes: HashMap<String, Option<String>> = HashMap::new();
    notes.insert("sodium.jar".into(), Some("优化".into()));
    assert_eq!(mod_note(&notes, "sodium.jar"), "优化");
    assert_eq!(mod_note(&notes, "sodium.jar.disabled"), "优化");

    // 只有带后缀那个键也能读出来，那是别的启动器写的
    let mut legacy: HashMap<String, Option<String>> = HashMap::new();
    legacy.insert("old.jar.disabled".into(), Some("旧备注".into()));
    assert_eq!(mod_note(&legacy, "old.jar.disabled"), "旧备注");

    // 没有 / 值为 None / 文件名为空：都返回空串，不 panic
    assert_eq!(mod_note(&notes, "missing.jar"), "");
    assert_eq!(mod_note(&notes, ""), "");
    let mut empty: HashMap<String, Option<String>> = HashMap::new();
    empty.insert("none.jar".into(), None);
    assert_eq!(mod_note(&empty, "none.jar"), "");
}

