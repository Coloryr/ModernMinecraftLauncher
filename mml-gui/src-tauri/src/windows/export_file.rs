//! 文本导出先写同目录临时文件，成功后才替换用户选定的文件。

use std::{fs::{self, File}, path::Path};

/// 完整写入导出文件，写入或提交失败时保留原文件。
/// - `path`: 用户选择的输出路径。
/// - `write`: 写入内容并检查取消状态的回调。
/// # 返回值
/// 文件已替换或写入失败的原因。
pub(super) fn write(path: &Path, write: impl FnOnce(&mut File) -> Result<(), String>) -> Result<(), String> {
    if path.file_name().is_none() { return Err(crate::err_keys::FILE_NAME.into()); }
    let temp = path.with_extension(format!("{}.part", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = File::create_new(&temp).map_err(|e| e.to_string())?;
        write(&mut file)?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        fs::rename(&temp, path).map_err(|e| e.to_string())
    })();
    if result.is_err() { let _ = fs::remove_file(&temp); }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

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
