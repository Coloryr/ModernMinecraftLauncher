//! 导出先写同目录临时文件，成功后才替换用户选定的文件。

use std::{fs::{self, File}, future::Future, path::{Path, PathBuf}};

/// 导出任务结束或取消时回收未提交的临时文件。
struct StagedFile {
    /// 与目标文件位于同一目录的临时路径。
    path: PathBuf,
}

impl StagedFile {
    fn new(destination: &Path) -> Result<Self, String> {
        if destination.file_name().is_none() { return Err(crate::err_keys::FILE_NAME.into()); }
        let path = destination.with_extension(format!("{}.part", uuid::Uuid::new_v4()));
        File::create_new(&path).map_err(|e| e.to_string())?;
        Ok(Self { path })
    }
}

impl Drop for StagedFile {
    fn drop(&mut self) { let _ = fs::remove_file(&self.path); }
}

/// 完整写入导出文件，写入或提交失败时保留原文件。
/// - `path`: 用户选择的输出路径。
/// - `write`: 写入内容并检查取消状态的回调。
/// # 返回值
/// 文件已替换或写入失败的原因。
pub(super) fn write(path: &Path, write: impl FnOnce(&mut File) -> Result<(), String>) -> Result<(), String> {
    let temp = StagedFile::new(path)?;
    let mut file = File::options().write(true).open(&temp.path).map_err(|e| e.to_string())?;
    write(&mut file)?;
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    fs::rename(&temp.path, path).map_err(|e| e.to_string())
}

/// 为异步导出提供暂存路径，成功后提交完整文件。
/// - `path`: 用户选择的输出路径。
/// - `write`: 将归档写入暂存路径的异步回调。
/// # 返回值
/// 文件已替换或导出失败的原因。
pub(super) async fn write_async<F: Future<Output = Result<(), String>>>(
    path: &Path, write: impl FnOnce(PathBuf) -> F,
) -> Result<(), String> {
    let temp = StagedFile::new(path)?;
    write(temp.path.clone()).await?;
    File::options().write(true).open(&temp.path).and_then(|file| file.sync_all()).map_err(|e| e.to_string())?;
    fs::rename(&temp.path, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[tokio::test]
    async fn async_export_failure_preserves_archive_and_success_replaces_it() {
        let dir = crate::test_support::temp_dir("archive-export-commit");
        let path = dir.join("instance.mrpack");
        fs::write(&path, "previous").unwrap();
        assert!(write_async(&path, |temp| async move {
            fs::write(temp, "partial").unwrap();
            Err("source disappeared".into())
        }).await.is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "previous");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        write_async(&path, |temp| async move { fs::write(temp, "complete").map_err(|e| e.to_string()) }).await.unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "complete");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        crate::test_support::cleanup(&[&dir]);
    }

    #[tokio::test]
    async fn cancelled_archive_task_cleans_its_partial_file() {
        let dir = crate::test_support::temp_dir("archive-export-cancelled");
        let path = dir.join("instance.zip");
        fs::write(&path, "previous").unwrap();
        let target = path.clone();
        let (ready, started) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            write_async(&target, |temp| async move {
                fs::write(temp, "partial").unwrap();
                ready.send(()).unwrap();
                std::future::pending::<Result<(), String>>().await
            }).await
        });
        started.await.unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert_eq!(fs::read_to_string(&path).unwrap(), "previous");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        crate::test_support::cleanup(&[&dir]);
    }

    #[test]
    fn successful_export_replaces_existing_text_without_bom() {
        let dir = crate::test_support::temp_dir("text-export-replace");
        let path = dir.join("中文.json");
        fs::write(&path, "previous").unwrap();
        write(&path, |file| file.write_all("[\"中文\"]".as_bytes()).map_err(|e| e.to_string())).unwrap();
        assert_eq!(fs::read(&path).unwrap(), "[\"中文\"]".as_bytes());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        crate::test_support::cleanup(&[&dir]);
    }

    #[test]
    fn failed_or_cancelled_write_keeps_previous_file_and_removes_partial_output() {
        let dir = crate::test_support::temp_dir("text-export-failed");
        let path = dir.join("mods.csv");
        fs::write(&path, "previous").unwrap();
        for reason in ["write failed", crate::err_keys::CANCELLED] {
            assert_eq!(write(&path, |file| {
                file.write_all(b"partial").unwrap();
                Err(reason.into())
            }).unwrap_err(), reason);
            assert_eq!(fs::read_to_string(&path).unwrap(), "previous");
            assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        }
        crate::test_support::cleanup(&[&dir]);
    }

    #[test]
    fn failed_new_export_leaves_no_destination_or_temporary_file() {
        let dir = crate::test_support::temp_dir("text-export-new-failed");
        assert!(write(&dir.join("mods.txt"), |_| Err("failed".into())).is_err());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
        assert!(write(&dir.join("missing/mods.txt"), |_| Ok(())).is_err());
        crate::test_support::cleanup(&[&dir]);
    }

    #[test]
    fn rename_failure_preserves_existing_directory_and_cleans_temporary_file() {
        let dir = crate::test_support::temp_dir("text-export-rename");
        let path = dir.join("mods.json");
        fs::create_dir(&path).unwrap();
        fs::write(path.join("keep"), "previous").unwrap();
        assert!(write(&path, |file| file.write_all(b"new").map_err(|e| e.to_string())).is_err());
        assert_eq!(fs::read_to_string(path.join("keep")).unwrap(), "previous");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        crate::test_support::cleanup(&[&dir]);
    }
}
