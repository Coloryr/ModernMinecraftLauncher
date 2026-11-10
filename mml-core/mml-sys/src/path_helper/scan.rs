//! 目录遍历与搜索
//!
//! 列文件、列目录、算目录大小、按名字找文件。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use mml_names::i18_items::error_type::CoreResult;

use super::fs_error;

/// 检查非法名字
///
/// - `name`: 需要检查的名字
///
/// # 返回值
///
/// 名字非法时返回 true：空、全点号、超 80 字节或含非法字符
pub fn file_has_invalid_chars(name: &str) -> bool {
    if name.is_empty() || name.chars().all(|c| c == '.') {
        return true;
    }

    if name.len() > 80 {
        return true;
    }

    name.contains(&['<', '>', ':', '"', '/', '\\', '|', '?', '*'][..])
}

/// 获取所有文件
///
/// - `local`: 递归搜索的根目录
///
/// # 返回值
///
/// 返回递归收集的文件路径列表
pub fn get_all_files<P: AsRef<Path>>(local: P) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if let Ok(entries) = fs::read_dir(local) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                files.push(path);
            } else if path.is_dir() {
                files.extend(get_all_files(&path));
            }
        }
    }

    files
}

/// 获取当前目录所有文件
///
/// - `path`: 需要获取的路径
///
/// # 返回值
///
/// 返回当前目录下的文件路径列表，不含子目录
pub fn get_files<P: AsRef<Path>>(path: P) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if entry.path().is_file() {
                files.push(entry.path());
            }
        }
    }

    files
}

/// 获取文件夹下面最后写入的文件
///
/// - `path`: 获取的目录
///
/// # 返回值
///
/// 返回最近写入的文件路径，目录为空时返回 `Ok(None)`
pub fn get_last_written_file<P: AsRef<Path>>(path: P) -> CoreResult<Option<PathBuf>> {
    let entries = fs::read_dir(&path).map_err(|err| fs_error(path.as_ref(), err))?;

    let mut files: Vec<(PathBuf, SystemTime)> = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|err| fs_error(path.as_ref(), err))?;
        let path = entry.path();

        if path.is_file() {
            let metadata = fs::metadata(&path).map_err(|err| fs_error(&path, err))?;
            if let Ok(modified) = metadata.modified() {
                files.push((path, modified));
            }
        }
    }

    files.sort_by(|a, b| b.1.cmp(&a.1));

    Ok(files.first().map(|(path, _)| path.clone()))
}

/// 获取目录占用大小
///
/// - `path`: 需要获取的路径
///
/// # 返回值
///
/// 返回目录总大小，单位字节
pub fn get_folder_size<P: AsRef<Path>>(path: P) -> u64 {
    let mut size = 0;

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                size += entry.metadata().map(|m| m.len()).unwrap_or(0);
            } else if path.is_dir() {
                size += get_folder_size(&path);
            }
        }
    }

    size
}

/// 获取当前目录所有目录
///
/// - `path`: 需要获取的路径
///
/// # 返回值
///
/// 返回当前目录下的子目录路径列表
pub fn get_dirs<P: AsRef<Path>>(path: P) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            }
        }
    }

    dirs
}

/// 查找文件
///
/// - `path`: 查找的目录
/// - `name`: 查找的文件名
///
/// # 返回值
///
/// 返回首个同名文件的路径，未找到时返回 `None`
pub fn search_file<P: AsRef<Path>>(path: P, name: &str) -> Option<PathBuf> {
    let files = get_all_files(path);
    files.into_iter().find(|f| f.file_name().unwrap() == name)
}
