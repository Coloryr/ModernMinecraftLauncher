//! 导出先写同目录临时文件，成功后才替换用户选定的文件。

use std::{
    fs::{self, File},
    future::Future,
    path::{Path, PathBuf},
};

/// 导出任务结束或取消时回收未提交的临时文件。
struct StagedFile {
    /// 与目标文件位于同一目录的临时路径。
    path: PathBuf,
}

impl StagedFile {
    /// 在目标文件同目录建一个占位临时文件
    ///
    /// - `destination`: 用户选择的输出路径
    ///
    /// # 返回值
    ///
    /// 已创建的暂存文件；目标没有文件名或建不出来时返回错误文案
    fn new(destination: &Path) -> Result<Self, String> {
        if destination.file_name().is_none() {
            return Err(crate::err_keys::FILE_NAME.into());
        }
        let path = destination.with_extension(format!("{}.part", uuid::Uuid::new_v4()));
        File::create_new(&path).map_err(|e| e.to_string())?;
        Ok(Self { path })
    }
}

impl Drop for StagedFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// 完整写入导出文件，写入或提交失败时保留原文件
///
/// - `path`: 用户选择的输出路径
/// - `write`: 写入内容并检查取消状态的回调
///
/// # 返回值
///
/// 文件已替换返回 `Ok(())`；写入或改名失败返回错误文案
pub(super) fn write(
    path: &Path,
    write: impl FnOnce(&mut File) -> Result<(), String>,
) -> Result<(), String> {
    let temp = StagedFile::new(path)?;
    let mut file = File::options()
        .write(true)
        .open(&temp.path)
        .map_err(|e| e.to_string())?;
    write(&mut file)?;
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    fs::rename(&temp.path, path).map_err(|e| e.to_string())
}

/// 为异步导出提供暂存路径，成功后提交完整文件
///
/// - `path`: 用户选择的输出路径
/// - `write`: 将归档写入暂存路径的异步回调
///
/// # 返回值
///
/// 文件已替换返回 `Ok(())`；导出被取消或失败时返回错误文案
pub(super) async fn write_async<F: Future<Output = Result<(), String>>>(
    path: &Path,
    write: impl FnOnce(PathBuf) -> F,
) -> Result<(), String> {
    let temp = StagedFile::new(path)?;
    write(temp.path.clone()).await?;
    File::options()
        .write(true)
        .open(&temp.path)
        .and_then(|file| file.sync_all())
        .map_err(|e| e.to_string())?;
    fs::rename(&temp.path, path).map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "../../tests/unit/windows_export_file_tests.rs"]
mod tests;
