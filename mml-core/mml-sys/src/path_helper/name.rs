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

/// 把下载来源给的文件名清成当前平台能落盘的名字
///
/// 平台（Modrinth / CurseForge）允许上传带 `:` 的文件名，直接 join 到本地路径会在
/// Windows 上搬运失败（os error 123）。在 [`replace_file_name`] 的基础上再处理
/// 结尾的点 / 空格与 Windows 保留设备名。
///
/// - `name`: 上游给的文件名
///
/// # 返回值
///
/// 可安全落盘的文件名；清洗后为空时返回 `download`
pub fn sanitize_file_name(name: &str) -> String {
    let mut out = replace_file_name(name);
    while out.ends_with('.') || out.ends_with(' ') {
        out.pop();
    }
    if out.is_empty() {
        return String::from("download");
    }
    let stem = out
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6",
        "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7",
        "LPT8", "LPT9",
    ];
    if RESERVED.contains(&stem.as_str()) {
        out.insert(0, '_');
    }
    out
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
