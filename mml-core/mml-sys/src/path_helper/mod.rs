//! 文件与路径处理模块
//!
//! 提供跨平台的文件系统操作封装，包括：
//!
//! - **基础 IO** — 文件读写、目录创建、复制与移动，同步与异步各一份
//! - **权限管理** — Unix chmod / Java 权限修复
//! - **回收站操作** — Windows (SHFileOperationW) / Linux (Trash) / macOS (osascript Finder)
//! - **文件搜索** — 递归搜索、目录遍历、大小统计
//! - **路径安全** — 非法字符过滤、文件名/路径名清理
//!
//! 跨平台
//!
//! 所有函数均提供同步版与 Tokio 异步版。
//! 回收站操作自动根据编译目标选择 Windows/Linux/macOS 实现。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use mml_names::i18_items::error_type::{CoreResult, ErrorType, FileSystemErrorData};

mod io_op;
mod link;
mod name;
mod perm;
mod safe_path;
mod scan;
mod trash;

pub use safe_path::{safe_join, safe_join_with_root};

/// 把底层 io 错误包成带路径的文件系统错误
///
/// - `path`: 出错的路径，用来定位是哪个文件
/// - `err`: 底层错误，原文直接进 `error` 字段
pub(super) fn fs_error(path: &Path, err: impl std::fmt::Display) -> ErrorType {
    ErrorType::FileSystemError(FileSystemErrorData {
        path: path.to_path_buf(),
        error: err.to_string(),
    })
}

pub use self::io_op::*;
pub use self::link::{FileId, file_id, link_count};
pub use self::name::{replace_file_name, replace_path_name};
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use self::perm::{chmod, per_java_chmod};
pub use self::scan::{
    file_has_invalid_chars, get_all_files, get_dirs, get_files, get_folder_size,
    get_last_written_file, search_file,
};
pub use self::trash::move_to_trash;

#[cfg(test)]
#[path = "../../tests/unit/path_helper_tests.rs"]
mod tests;
