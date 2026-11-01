use super::{
    clean_url, find_http, main_window_label, read_shortcut_url, take_startup_quiet,
    window_probe_drop, work_area_is_full_monitor,
};
use crate::test_support::{cleanup, temp_dir};

/// 写一个临时文件并读回它的地址
fn url_of(name: &str, content: &[u8]) -> String {
    let dir = temp_dir("probe-shortcut");
    let file = dir.join(name);
    std::fs::write(&file, content).unwrap();
    let url = read_shortcut_url(&file.to_string_lossy());
    cleanup(&[&dir]);
    url
}

/// Windows 互联网快捷方式 INI：读 `URL=`，忽略其它键
#[test]
fn shortcut_url_from_ini() {
    let text =
        "[InternetShortcut]\r\nURL=https://littleskin.cn/api/yggdrasil\r\nIconIndex=0\r\n";
    assert_eq!(
        url_of("LittleSkin.url", text.as_bytes()),
        "https://littleskin.cn/api/yggdrasil"
    );
}

/// 键名大小写不敏感；结尾斜杠去掉，后端认证时会自己补
#[test]
fn shortcut_url_is_normalized() {
    assert_eq!(
        url_of(
            "a.url",
            b"[InternetShortcut]\nurl=https://example.com/api/yggdrasil/\n"
        ),
        "https://example.com/api/yggdrasil"
    );
}

/// macOS 的 .webloc 是 plist：XML 那种直接扫地址，二进制那种也一样能捞到
#[test]
fn shortcut_url_from_plist() {
    let xml = br#"<?xml version="1.0"?><plist version="1.0"><dict><key>URL</key><string>https://example.com/api/yggdrasil</string></dict></plist>"#;
    assert_eq!(url_of("a.webloc", xml), "https://example.com/api/yggdrasil");

    // 二进制 plist：字符串原样存在文件里，兜底扫描要能捞出来
    let mut bin = b"bplist00\xd1\x01\x02X$versionYURL\x00".to_vec();
    bin.extend_from_slice(b"https://bin.example.com/api/yggdrasil");
    bin.push(0);
    assert_eq!(
        url_of("b.webloc", &bin),
        "https://bin.example.com/api/yggdrasil"
    );
}

/// 没有地址 / 只有协议头：都返回空串，上层当"不认识的文件"处理
#[test]
fn shortcut_without_url_is_empty() {
    assert_eq!(url_of("a.url", b"[InternetShortcut]\nIconIndex=0\n"), "");
    assert_eq!(url_of("b.url", b"[InternetShortcut]\nURL=https://\n"), "");
}

/// 探测：`.url` → shortcut；"只有一个地址"的 `.txt` / `.json` → 也认
#[test]
fn probe_shortcut_files() {
    assert_eq!(
        probe(
            "a.url",
            b"[InternetShortcut]\nURL=https://littleskin.cn/api/yggdrasil\n"
        ),
        "shortcut"
    );
    // 卡片拖到桌面有时是这些后缀：内容可能是裸地址、带引号、或包一层 json
    assert_eq!(
        probe("card.txt", b"  https://littleskin.cn/api/yggdrasil  \n"),
        "shortcut"
    );
    assert_eq!(
        probe("quoted.json", b"\"https://littleskin.cn/api/yggdrasil\""),
        "shortcut"
    );
    assert_eq!(
        probe(
            "wrapped.json",
            br#"{"url":"https://littleskin.cn/api/yggdrasil"}"#
        ),
        "shortcut"
    );
    // 里面夹着别的东西，如 modpack 的 manifest.json 有多个地址 + 大段内容，不能当登录快捷方式
    assert_eq!(
        probe(
            "manifest.json",
            br#"{"files":[{"url":"https://a.com/1.jar"},{"url":"https://a.com/2.jar"}]}"#
        ),
        "other"
    );
    assert_eq!(probe("readme.txt", b"hello"), "other");
    // 太长的小文本也不认：只有一个地址但夹着一大段说明
    let long = format!("{}https://a.com/x", "说明文字".repeat(30));
    assert_eq!(probe("note.txt", long.as_bytes()), "other");
}

/// 写法统一：建临时文件 → 探测 → 拿 kind
fn probe(name: &str, content: &[u8]) -> String {
    let dir = temp_dir("probe-kind");
    let file = dir.join(name);
    std::fs::write(&file, content).unwrap();
    let kind = window_probe_drop(file.to_string_lossy().to_string()).kind;
    cleanup(&[&dir]);
    kind
}

/// 地址截断点：到空白 / 引号 / 尖括号为止，不会把后面的内容一起吞掉
#[test]
fn find_http_stops_at_delimiters() {
    assert_eq!(
        find_http("<string>https://a.com/x</string>"),
        Some(String::from("https://a.com/x"))
    );
    assert_eq!(
        find_http("see https://a.com/x and more"),
        Some(String::from("https://a.com/x"))
    );
    assert_eq!(find_http("no url here"), None);
}

/// 规范化：去空白、去尾斜杠、拒绝无主机
#[test]
fn clean_url_rules() {
    assert_eq!(clean_url("  https://a.com/  "), "https://a.com");
    assert_eq!(clean_url("http://a.com/x/"), "http://a.com/x");
    assert_eq!(clean_url("https://"), "");
    assert_eq!(clean_url("ftp://a.com"), "");
}

/// 只有主窗口的第一次显示不抢前台，其余照常
#[test]
fn startup_quiet_only_for_first_main_show() {
    crate::windows::quiet_main_window_startup();
    assert!(!take_startup_quiet("mml-log-x"), "别的窗口不该走不抢前台");
    assert!(take_startup_quiet(main_window_label()), "主窗口第一次要走");
    assert!(
        !take_startup_quiet(main_window_label()),
        "第二次起照常拿焦点"
    );
}

/// 工作区与显示器完全重合时判定为「平台没给任务栏信息」
#[test]
fn full_work_area_matches_monitor() {
    let position = tauri::PhysicalPosition::new(0, 0);
    let size = tauri::PhysicalSize::new(1920, 1080);

    let full = tauri::PhysicalRect { position, size };
    assert!(work_area_is_full_monitor(&full, &position, &size));

    let bottom_bar = tauri::PhysicalRect {
        position,
        size: tauri::PhysicalSize::new(1920, 1032),
    };
    assert!(!work_area_is_full_monitor(&bottom_bar, &position, &size));

    let left_bar = tauri::PhysicalRect {
        position: tauri::PhysicalPosition::new(72, 0),
        size: tauri::PhysicalSize::new(1848, 1080),
    };
    assert!(!work_area_is_full_monitor(&left_bar, &position, &size));
}

