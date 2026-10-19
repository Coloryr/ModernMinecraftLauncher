//! 导入的公共口径
//!
//! 同名冲突的处理方式、唯一名与文件名归一，三个导入入口共用。

use std::path::{Path, PathBuf};





/// 只预检冲突，不落盘
pub(super) const MODE_CHECK: &str = "check";

/// 冲突项跳过，保留盘上已有的那个
pub(super) const MODE_SKIP: &str = "skip";

/// 冲突项自动加 ` (1)` 序号
pub(super) const MODE_RENAME: &str = "rename";

/// 改名后的落点：新文件名与它在目标目录下的完整路径
///
/// 两者一起返回，是因为调用方一个要落盘 `path`、另一个要把 `name` 报给前端，
/// 而两者必须同源。
pub(super) struct UniqueName {
    /// 新的文件名，含扩展名，形如 `a (1).jar`
    pub(super) name: String,
    /// `name` 拼在目标目录下的完整路径，与 `name` 同源
    pub(super) path: PathBuf,
}

/// 在 `dir` 里给 `name` 找一个不撞的名字：`a.jar` → `a (1).jar`
///
/// - `dir`: 目标目录
/// - `name`: 原始文件名
///
/// # 返回值
///
/// 见 [`UniqueName`]；999 个候选都撞时退回原名
///
/// 试到 999 还撞就退回原名，宁可覆盖，也不要在这里无限找下去。
pub(super) fn unique_name(dir: &Path, name: &str) -> UniqueName {
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .map(|item| item.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|item| format!(".{}", item.to_string_lossy()))
        .unwrap_or_default();

    for index in 1..1000 {
        let candidate = format!("{stem} ({index}){ext}");
        let target = dir.join(&candidate);
        if !target.exists() {
            return UniqueName {
                name: candidate,
                path: target,
            };
        }
    }
    UniqueName {
        name: name.to_string(),
        path: dir.join(name),
    }
}

/// 取路径的文件名
///
/// - `file`: 文件路径
///
/// # 返回值
///
/// 文件名；路径以 `..` 结尾等取不到的情况返回 `None`
pub(super) fn file_name(file: &Path) -> Option<String> {
    file.file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

/// 存档压缩包会解压成哪个目录名
///
/// - `file`: 存档压缩包路径
///
/// # 返回值
///
/// 解压后的目录名；推不出来时是空串
///
/// 口径与 `mml_game` 的 `import_save` 保持一致：包内只有一个顶层目录就用它，
/// 否则用压缩包文件名。
pub(super) fn save_dir_name(file: &Path) -> String {
    if let Ok(archive) = mml_base::archives::BaseArchive::open(file)
        && let Some(top) = archive.single_top_dir()
    {
        return top.to_string();
    }
    file.file_stem()
        .map(|stem| {
            // 二次去后缀，处理 .tar.gz 这类双后缀，与内核 import_save 同款
            stem.to_string_lossy().trim_end_matches(".tar").to_string()
        })
        .unwrap_or_default()
}
