//! 剪贴板操作
//!
//! Windows 直接调 Win32 剪贴板 API：clipboard-rs 会额外链一套 `windows 0.59`，
//! 而这里只需要"写文本"和"写文件路径列表"两件事。其它平台仍走 clipboard-rs。

use std::path::PathBuf;

#[cfg(not(windows))]
use std::sync::LazyLock;

#[cfg(not(windows))]
use clipboard_rs::{Clipboard, ClipboardContext};

/// 非 Windows 的全局剪贴板上下文，首次使用时惰性创建
#[cfg(not(windows))]
static CONTEXT: LazyLock<ClipboardContext> = LazyLock::new(|| ClipboardContext::new().unwrap());

/// 将文本复制到剪贴板
///
/// - `text`: 要复制的文本
#[cfg(windows)]
pub fn copy_text(text: &str) {
    windows_clipboard::set_text(text);
}

/// 将文本复制到剪贴板
///
/// - `text`: 要复制的文本
#[cfg(not(windows))]
pub fn copy_text(text: &str) {
    _ = CONTEXT.set_text(text.to_string());
}

/// 将文件复制到剪贴板
///
/// - `files`: 要复制的文件路径列表
#[cfg(windows)]
pub fn copy_files(files: Vec<PathBuf>) {
    windows_clipboard::set_files(&files);
}

/// 将文件复制到剪贴板
///
/// - `files`: 要复制的文件路径列表
#[cfg(not(windows))]
pub fn copy_files(files: Vec<PathBuf>) {
    _ = CONTEXT.set_files(
        files
            .iter()
            .map(|item| item.to_string_lossy().to_string())
            .collect(),
    );
}

#[cfg(windows)]
mod windows_clipboard {
    use std::mem::{offset_of, size_of};
    use std::path::PathBuf;

    use windows::Win32::Foundation::{GlobalFree, HANDLE};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
    use windows::Win32::UI::Shell::DROPFILES;

    /// 文本格式，即 `CF_UNICODETEXT`；windows crate 把它放在 Ole 模块，这里直接写值
    const CF_UNICODETEXT: u32 = 13;
    /// 文件名列表格式，即 `CF_HDROP`
    const CF_HDROP: u32 = 15;

    /// 把一段字节写进剪贴板
    ///
    /// - `format`: 剪贴板格式号
    /// - `bytes`: 要写入的数据，长度即全局块大小
    ///
    /// 剪贴板是全局独占资源：打不开就放弃，不重试也不报错
    fn put(format: u32, bytes: &[u8]) {
        unsafe {
            if OpenClipboard(None).is_err() {
                return;
            }
            _ = EmptyClipboard();
            if let Ok(handle) = GlobalAlloc(GMEM_MOVEABLE, bytes.len()) {
                let dst = GlobalLock(handle);
                if dst.is_null() {
                    _ = GlobalFree(Some(handle));
                } else {
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst.cast::<u8>(), bytes.len());
                    _ = GlobalUnlock(handle);
                    // 交给系统后由剪贴板负责释放；失败时自己释放，避免泄漏
                    if SetClipboardData(format, Some(HANDLE(handle.0))).is_err() {
                        _ = GlobalFree(Some(handle));
                    }
                }
            }
            _ = CloseClipboard();
        }
    }

    /// 写文本到剪贴板
    pub(super) fn set_text(text: &str) {
        let mut wide: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
        wide.extend_from_slice(&[0, 0]);
        put(CF_UNICODETEXT, &wide);
    }

    /// 写文件路径列表到剪贴板
    pub(super) fn set_files(files: &[PathBuf]) {
        let header = DROPFILES {
            pFiles: size_of::<DROPFILES>() as u32,
            ..Default::default()
        };
        // fWide 必须为 1：路径按 UTF-16 存放，否则资源管理器会按 ANSI 解析
        let mut data = header_bytes(&header);
        data[offset_of!(DROPFILES, fWide)..][..4].copy_from_slice(&1i32.to_le_bytes());
        for file in files {
            for unit in file.to_string_lossy().encode_utf16() {
                data.extend_from_slice(&unit.to_le_bytes());
            }
            data.push(0);
            data.push(0);
        }
        // 整个列表再补一个 0：CF_HDROP 要求双 0 结尾
        data.push(0);
        data.push(0);
        put(CF_HDROP, &data);
    }

    /// 取 DROPFILES 头的字节表示
    fn header_bytes(header: &DROPFILES) -> Vec<u8> {
        // SAFETY: DROPFILES 是不含指针的 POD，按字节读取始终有效
        unsafe {
            std::slice::from_raw_parts(
                header as *const DROPFILES as *const u8,
                size_of::<DROPFILES>(),
            )
        }
        .to_vec()
    }
}
