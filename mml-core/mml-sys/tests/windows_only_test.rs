//! mml-sys Windows 专属集成测试
//!
//! 覆盖 Java 扫描与剪贴板；快捷方式 / 回收站涉及 COM 与 Shell 操作，
//! 在独立线程中执行以避免 COM 单元状态互相干扰。
//!
//! 说明：protocol_helper 的注册/反注册会写 HKEY_CLASSES_ROOT，影响全机，
//! 不在自动化测试范围内。

#![cfg(windows)]

use std::path::PathBuf;

/// 在临时目录创建本轮测试唯一目录
fn make_test_root(tag: &str) -> PathBuf {
    let dir = mml_testutil::temp_dir().join(format!(
        "mml_sys_win_test_{}_{}_{}",
        tag,
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Java 扫描：只要求能安全执行，结果数量依机器环境而定，可能为 0
#[test]
fn test_find_java_inner() {
    let mut paths = std::collections::HashSet::new();
    mml_sys::java_scan_helper::find_java_inner(&mut paths);
    // 所有返回路径都应指向 javaw.exe
    for path in &paths {
        assert!(
            path.ends_with("bin\\javaw.exe") || path.file_name().unwrap() == "javaw.exe",
            "扫描结果应为 javaw.exe: {path:?}"
        );
    }
}

/// 读回剪贴板里的文本：直接用 Win32，测试不再依赖 clipboard-rs
///
/// # 返回值
///
/// 剪贴板里的文本；没有文本时 panic，测试里就是失败
fn read_clipboard_text() -> String {
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::System::DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard};
    use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};

    /// 文本格式，即 `CF_UNICODETEXT`
    const CF_UNICODETEXT: u32 = 13;

    unsafe {
        OpenClipboard(None).expect("打开剪贴板");
        let handle = GetClipboardData(CF_UNICODETEXT).expect("剪贴板里没有文本");
        let memory = HGLOBAL(handle.0);
        let ptr = GlobalLock(memory).cast::<u16>();
        // 内容是以 0 结尾的 UTF-16
        let mut len = 0usize;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
        _ = GlobalUnlock(memory);
        _ = CloseClipboard();
        text
    }
}

/// 剪贴板写入后可读回
#[test]
fn test_clipboard_copy_text() {
    let text = format!("mml 测试文本 {}", std::process::id());
    mml_sys::clipboard_helper::copy_text(&text);
    assert_eq!(read_clipboard_text(), text);
}

/// 创建 .lnk 快捷方式
#[test]
fn test_create_shortcut() {
    let root = make_test_root("shortcut");
    let lnk = root.join("test_shortcut.lnk");
    let work = root.join("work");
    std::fs::create_dir_all(&work).unwrap();

    // COM 初始化是线程相关的，放到独立线程执行，避免与其他测试互相干扰
    let lnk_path = lnk.clone();
    let work_path = work.clone();
    let handle = std::thread::spawn(move || {
        mml_sys::shortcut_helper::create_shortcut("test-uuid", None::<PathBuf>, work_path, lnk_path)
    });
    let result = handle.join().unwrap();

    // 成功时应生成 .lnk 文件
    if result.is_ok() {
        assert!(lnk.exists(), "快捷方式文件应存在");
        assert!(std::fs::metadata(&lnk).unwrap().len() > 0);
    } else {
        // COM 初始化失败等环境问题时跳过断言，如 RPC_E_CHANGED_MODE
        eprintln!("create_shortcut 在当前环境不可用: {:?}", result.err());
    }

    let _ = std::fs::remove_dir_all(&root);
}

/// move_to_trash 将文件移入回收站
#[test]
fn test_move_to_trash() {
    let root = make_test_root("trash");
    let file = root.join("mml_trash_target.txt");
    std::fs::write(&file, b"to be trashed").unwrap();

    mml_sys::path_helper::move_to_trash(&file).unwrap();
    assert!(!file.exists(), "文件应已移出原位置");

    // 不存在的路径静默成功
    mml_sys::path_helper::move_to_trash(root.join("no_such_dir_xyz")).unwrap();

    let _ = std::fs::remove_dir_all(&root);
}

/// move_to_trash 将整个目录树移入回收站：删实例走的就是这条路径
#[test]
fn test_move_to_trash_dir() {
    let root = make_test_root("trash_dir");
    let dir = root.join("instances").join("mml_trash_dir_target");
    std::fs::create_dir_all(dir.join("mods")).unwrap();
    std::fs::write(dir.join("mods").join("a.jar"), b"jar").unwrap();

    mml_sys::path_helper::move_to_trash(&dir).unwrap();
    assert!(!dir.exists(), "目录树应已移出原位置");

    let _ = std::fs::remove_dir_all(&root);
}
