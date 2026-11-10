//! 文件名清理
//!
//! 把用户填的名字换成能当目录名用的形式。

/// 替换文件名非法字符
///
/// - `name`: 原文件名
///
/// # 返回值
///
/// 返回把非法字符 `<>:"/\|?*` 与 `\0` 替换为 `_` 后的名字
pub fn replace_file_name(name: &str) -> String {
    name.replace(|c: char| "<>:\"/\\|?*\0".contains(c), "_")
}

/// 替换文件名非法字符
///
/// - `name`: 原路径名
///
/// # 返回值
///
/// 返回替换后的路径名，Windows 额外替换 `<>:"/\|?*`，其他平台仅替换 `\0`
pub fn replace_path_name(name: &str) -> String {
    #[cfg(not(windows))]
    let invalid_chars: Vec<char> = vec!['\0'];

    #[cfg(windows)]
    let invalid_chars: Vec<char> = vec!['<', '>', ':', '"', '/', '\\', '|', '?', '*', '\0'];

    name.chars()
        .map(|c| if invalid_chars.contains(&c) { '_' } else { c })
        .collect()
}
