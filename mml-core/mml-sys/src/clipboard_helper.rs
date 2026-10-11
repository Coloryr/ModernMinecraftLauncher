//! 剪贴板操作

use std::{path::PathBuf, sync::LazyLock};

use clipboard_rs::{Clipboard, ClipboardContext};

/// 全局剪贴板上下文，首次使用时惰性创建
static CONTEXT: LazyLock<ClipboardContext> = LazyLock::new(|| ClipboardContext::new().unwrap());

/// 将文本复制到剪贴板
///
/// - `text`: 要复制的文本
pub fn copy_text(text: &str) {
    _ = CONTEXT.set_text(text.to_string());
}

/// 将文件复制到剪贴板
///
/// - `files`: 要复制的文件路径列表
pub fn copy_files(files: Vec<PathBuf>) {
    _ = CONTEXT.set_files(
        files
            .iter()
            .map(|item| item.to_string_lossy().to_string())
            .collect(),
    );
}
