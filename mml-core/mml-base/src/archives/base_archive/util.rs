//! 内部工具
//!
//! 去掉目录前缀与替换压缩包文件。

use std::{fs, path::Path};

use mml_names::i18_items::error_type::CoreResult;

use crate::archives::fs_err;

/// 去掉条目名指定的顶层包裹目录前缀
///
/// 例如 `dir/file.txt` 去掉 `dir` 后得到 `file.txt`，同时兼容 `\` 分隔的条目名
///
/// - `name`: 条目名
/// - `dir`: 要去掉的顶层包裹目录名
///
/// # 返回值
///
/// 条目不在该目录下、或本身就是 `dir` 时保持原名，避免误剥
pub(super) fn strip_dir_prefix(name: &str, dir: &str) -> String {
    name.strip_prefix(&format!("{dir}/"))
        .or_else(|| name.strip_prefix(&format!("{dir}\\")))
        .map(str::to_string)
        .unwrap_or_else(|| name.to_string())
}

/// 同目录重命名覆盖，失败时保留原包并尝试清理临时包。
pub(super) fn replace_archive(replacement: &Path, original: &Path) -> CoreResult<()> {
    fs::rename(replacement, original).map_err(|err| {
        let _ = fs::remove_file(replacement);
        fs_err(original.to_path_buf(), err)
    })
}
