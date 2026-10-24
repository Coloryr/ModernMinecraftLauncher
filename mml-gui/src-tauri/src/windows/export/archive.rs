//! 本地实例归档；不追随链接，写完后才替换目标文件。

use std::{fs::{self, File}, io::{Read, Write}, path::{Path, PathBuf}, sync::atomic::{AtomicBool, Ordering}, time::Instant};
use zip::ZipWriter;

use mml_base::archives::BaseArchiveGui;

use crate::{err_keys, log_text::LogText};

pub(super) fn files_under(root: &Path) -> Result<Vec<PathBuf>, String> {
    reject_link(root)?;
    let mut files = Vec::new();
    for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let meta = reject_link(&entry.path())?;
        if meta.is_dir() { files.extend(files_under(&entry.path())?); }
        else if meta.is_file() { files.push(entry.path()); }
    }
    files.sort();
    Ok(files)
}

pub(super) fn reject_link(path: &Path) -> Result<fs::Metadata, String> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if meta.file_type().is_symlink() { return Err(err_keys::EXPORT_LINK.into()); }
    #[cfg(windows)] {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 { return Err(err_keys::EXPORT_LINK.into()); }
    }
    Ok(meta)
}

/// 解析所选路径并逐级拒绝链接，防止中间目录跳出实例。
/// - `root`: 游戏目录。
/// - `relative`: 前端选择的相对路径。
/// # 返回值
/// 已验证存在的普通文件或目录路径。
pub(super) fn selected_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = crate::windows::config::resolve_under(root, relative)?;
    reject_link(root)?;
    let mut current = root.to_path_buf();
    for part in path.strip_prefix(root).map_err(|e| e.to_string())?.components() {
        current.push(part);
        let meta = reject_link(&current)?;
        if !meta.is_file() && !meta.is_dir() { return Err(err_keys::FILE_NAME.into()); }
    }
    Ok(path)
}

pub(super) fn check_destination(file: &Path, source: &Path) -> Result<(), String> {
    let parent = file.parent().ok_or(err_keys::FILE_NAME)?
        .canonicalize().map_err(|e| e.to_string())?;
    let source = source.canonicalize().map_err(|e| e.to_string())?;
    if parent.starts_with(source) { return Err(err_keys::EXPORT_DESTINATION.to_string()); }
    Ok(())
}

/// 单次读写的块大小；`io::copy` 的 8KB 在写压缩包时系统调用开销明显
const COPY_BUF: usize = 256 * 1024;

pub(super) fn write_zip(
    file: &Path, files: &[(String, PathBuf)], data: &[(String, Vec<u8>)],
    gui: &BaseArchiveGui, cancel: &AtomicBool,
) -> Result<(), String> {
    let temp = file.with_extension(format!("{}.part", uuid::Uuid::new_v4()));
    // 分四段计时：建包、写条目、收尾、改名，结束时一次打进日志
    let started = Instant::now();
    let result = (|| -> Result<(), String> {
        let handle = File::create_new(&temp).map_err(|e| e.to_string())?;
        let mut zip = ZipWriter::new(handle);
        let opened = Instant::now();
        let mut buffer = vec![0u8; COPY_BUF];
        if let Some(gui) = gui { gui.start(files.len() + data.len()); }
        let mut index = 0;
        for (name, path) in files {
            // 每写一个文件看一次取消：一个文件内部的复制无法打断
            if cancel.load(Ordering::Relaxed) { return Err(err_keys::CANCELLED.to_string()); }
            if let Some(gui) = gui { gui.update(Some(name.clone()), index); }
            zip.start_file(name, mml_base::archives::zip_entry_options(name, 0o644)).map_err(|e| e.to_string())?;
            let mut source = File::open(path).map_err(|e| e.to_string())?;
            loop {
                let read = source.read(&mut buffer).map_err(|e| e.to_string())?;
                if read == 0 { break; }
                zip.write_all(&buffer[..read]).map_err(|e| e.to_string())?;
            }
            index += 1;
        }
        for (name, bytes) in data {
            if let Some(gui) = gui { gui.update(Some(name.clone()), index); }
            zip.start_file(name, mml_base::archives::zip_entry_options(name, 0o644)).map_err(|e| e.to_string())?;
            zip.write_all(bytes).map_err(|e| e.to_string())?;
            index += 1;
        }
        let written = Instant::now();
        zip.finish().map_err(|e| e.to_string())?.sync_all().map_err(|e| e.to_string())?;
        let finished = Instant::now();
        if let Some(gui) = gui { gui.done(); }
        fs::rename(&temp, file).map_err(|e| e.to_string())?;
        mml_log::info(LogText::ExportPackDetail {
            open: (opened - started).as_secs_f64(),
            write: (written - opened).as_secs_f64(),
            finish: (finished - written).as_secs_f64(),
            commit: Some(Instant::now().duration_since(finished).as_secs_f64()),
            entries: files.len() + data.len(),
        }.text());
        Ok(())
    })();
    if result.is_err() { let _ = fs::remove_file(&temp); }
    result
}

pub(super) fn native_instance(
    game: &mml_game::launcher::instance_setting_obj::InstanceSettingObj, file: &Path,
    gui: &BaseArchiveGui, cancel: &AtomicBool,
) -> Result<(), String> {
    let root = game.get_base_path();
    native_archive(game, &root, file, gui, cancel)
}

/// 只归档勾选的文件，外加当前实例配置快照
///
/// 与 [`native_instance`] 的区别只在文件集合：勾选为空时调用方应回退到整份归档，
/// 这里假定 selected 非空。归档名相对**实例根目录**，即 `.minecraft/...` 那一段。
///
/// - `game`: 实例配置，包里的 game.json 用当前快照
/// - `selected`: 勾选的文件绝对路径，必须都在实例目录内
/// - `file`: 输出 zip
///
/// # 返回值
///
/// 写盘成功返回 `Ok(())`；路径越界、读文件或压缩失败返回错误文案
pub(super) fn native_instance_selected(
    game: &mml_game::launcher::instance_setting_obj::InstanceSettingObj,
    selected: &[std::path::PathBuf], file: &Path, gui: &BaseArchiveGui, cancel: &AtomicBool,
) -> Result<(), String> {
    let root = game.get_base_path();
    native_archive_selected(game, &root, selected, file, gui, cancel)
}

/// 归档勾选文件的实现；root 显式传入，测试可以不初始化实例根目录
fn native_archive_selected(
    game: &mml_game::launcher::instance_setting_obj::InstanceSettingObj,
    root: &Path, selected: &[std::path::PathBuf], file: &Path, gui: &BaseArchiveGui,
    cancel: &AtomicBool,
) -> Result<(), String> {
    check_destination(file, root)?;
    let mut files = Vec::with_capacity(selected.len());
    for path in selected {
        let rel = path.strip_prefix(&root).map_err(|_| err_keys::FILE_NAME.to_string())?;
        let name = rel.to_string_lossy().replace('\\', "/");
        if name == mml_names::names::GAME_FILE { continue; }
        files.push((name, path.clone()));
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let data = serde_json::to_vec_pretty(game).map_err(|e| e.to_string())?;
    write_zip(file, &files, &[(mml_names::names::GAME_FILE.to_string(), data)], gui, cancel)
}

/// 归档实例目录，并用当前配置快照替换磁盘上尚未更新的配置。
fn native_archive(
    game: &mml_game::launcher::instance_setting_obj::InstanceSettingObj, root: &Path, file: &Path,
    gui: &BaseArchiveGui, cancel: &AtomicBool,
) -> Result<(), String> {
    check_destination(file, root)?;
    let files = files_under(root)?.into_iter().filter_map(|path| {
        let name = path.strip_prefix(root).ok()?.to_string_lossy().replace('\\', "/");
        (name != mml_names::names::GAME_FILE).then_some((name, path))
    }).collect::<Vec<_>>();
    let data = serde_json::to_vec_pretty(game).map_err(|e| e.to_string())?;
    write_zip(file, &files, &[(mml_names::names::GAME_FILE.to_string(), data)], gui, cancel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn native_archive_contains_all_files_and_current_instance_snapshot() {
        crate::test_support::ensure_boot();
        let dir = crate::test_support::temp_dir("native-roundtrip");
        let root = dir.join("source");
        fs::create_dir_all(root.join(".minecraft/config")).unwrap();
        fs::write(root.join(".minecraft/config/中文.toml"), "enabled = true").unwrap();
        fs::write(root.join("notes.txt"), "user notes").unwrap();
        fs::write(root.join(mml_names::names::GAME_FILE), "stale config").unwrap();
        let game = serde_json::from_value(serde_json::json!({"Name":"当前实例", "Version":"1.21.1"})).unwrap();
        let output = dir.join("instance.zip");
        native_archive(&game, &root, &output, &None, &AtomicBool::new(false)).unwrap();
        let mut zip = zip::ZipArchive::new(File::open(&output).unwrap()).unwrap();
        assert_eq!(zip.len(), 3);
        let mut text = String::new();
        zip.by_name(".minecraft/config/中文.toml").unwrap().read_to_string(&mut text).unwrap();
        assert_eq!(text, "enabled = true");
        let json: serde_json::Value = serde_json::from_reader(zip.by_name(mml_names::names::GAME_FILE).unwrap()).unwrap();
        assert_eq!(json["Name"], "当前实例");
        assert_eq!(json["Version"], "1.21.1");
        assert!(zip.file_names().all(|name| !name.unwrap().contains('\\')));
        drop(zip);
        crate::test_support::cleanup(&[&dir]);
    }

    #[test]
    fn native_selected_archive_keeps_only_checked_files() {
        crate::test_support::ensure_boot();
        let dir = crate::test_support::temp_dir("native-selected");
        let root = dir.join("source");
        fs::create_dir_all(root.join(".minecraft/config")).unwrap();
        fs::write(root.join(".minecraft/config/keep.toml"), "keep").unwrap();
        fs::write(root.join(".minecraft/config/skip.toml"), "skip").unwrap();
        fs::write(root.join(mml_names::names::GAME_FILE), "stale config").unwrap();
        let game = serde_json::from_value(serde_json::json!({"Name":"勾选实例", "Version":"1.21.1"})).unwrap();
        let output = dir.join("selected.zip");
        let selected = vec![root.join(".minecraft/config/keep.toml")];
        native_archive_selected(&game, &root, &selected, &output, &None, &AtomicBool::new(false)).unwrap();
        let mut zip = zip::ZipArchive::new(File::open(&output).unwrap()).unwrap();
        let mut names: Vec<String> = zip.file_names().map(|n| n.unwrap_or_default().to_string()).collect();
        names.sort();
        assert_eq!(names, [".minecraft/config/keep.toml", mml_names::names::GAME_FILE]);
        let json: serde_json::Value = serde_json::from_reader(zip.by_name(mml_names::names::GAME_FILE).unwrap()).unwrap();
        assert_eq!(json["Name"], "勾选实例");
        drop(zip);
        crate::test_support::cleanup(&[&dir]);
    }

    #[test]
    fn archive_read_failure_preserves_existing_archive_and_removes_partial_file() {
        crate::test_support::ensure_boot();
        let dir = crate::test_support::temp_dir("native-write-failure");
        let source = dir.join("source.txt");
        fs::write(&source, "content").unwrap();
        let output = dir.join("instance.zip");
        fs::write(&output, "previous archive").unwrap();
        let files = [("valid.txt".into(), source), ("missing.txt".into(), dir.join("missing"))];
        assert!(write_zip(&output, &files, &[], &None, &AtomicBool::new(false)).is_err());
        assert_eq!(fs::read_to_string(&output).unwrap(), "previous archive");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 2);
        crate::test_support::cleanup(&[&dir]);
    }

    #[test]
    fn archive_replaces_destination_without_including_itself() {
        crate::test_support::ensure_boot();
        let dir = crate::test_support::temp_dir("native-export");
        let source = dir.join("instance");
        fs::create_dir(&source).unwrap();
        let config = source.join("options.txt");
        fs::write(&config, "language:zh_cn").unwrap();
        let output = dir.join("instance.zip");
        fs::write(&output, "old archive").unwrap();
        assert!(check_destination(&source.join("bad.zip"), &source).is_err());
        write_zip(&output, &[(".minecraft/options.txt".into(), config)],
            &[("game.json".into(), br#"{"Name":"test","Version":"1.21.1"}"#.to_vec())], &None, &AtomicBool::new(false)).unwrap();
        let mut archive = zip::ZipArchive::new(File::open(&output).unwrap()).unwrap();
        assert_eq!(archive.len(), 2);
        let mut contents = String::new();
        archive.by_name(".minecraft/options.txt").unwrap().read_to_string(&mut contents).unwrap();
        assert_eq!(contents, "language:zh_cn");
        drop(archive);
        crate::test_support::cleanup(&[&dir]);
    }
    #[cfg(windows)]
    #[test]
    fn refuses_windows_junctions_in_export_tree() {
        use std::os::windows::process::CommandExt;
        crate::test_support::ensure_boot();
        let dir = crate::test_support::temp_dir("export-junction");
        let source = dir.join("instance");
        let outside = dir.join("outside");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("private.txt"), "private").unwrap();
        let link = source.join("linked");
        let quoted = |path: &Path| path.to_string_lossy().replace('\'', "''");
        let status = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", &format!(
                "New-Item -ItemType Junction -Path '{}' -Target '{}' -ErrorAction Stop | Out-Null", quoted(&link), quoted(&outside))])
            .creation_flags(0x08000000).status().unwrap();
        assert!(status.success());
        assert!(files_under(&source).is_err());
        assert!(files_under(&link).is_err());
        assert!(selected_path(&source, "linked/private.txt").is_err());
        assert!(selected_path(&source, "linked").is_err());
        fs::remove_dir(&link).unwrap();
        crate::test_support::cleanup(&[&dir]);
    }

    #[test]
    fn selected_paths_reject_missing_and_traversal_without_dropping_files() {
        crate::test_support::ensure_boot();
        let dir = crate::test_support::temp_dir("export-selection");
        let config = dir.join("config");
        fs::create_dir(&config).unwrap();
        fs::write(config.join("中文.toml"), "enabled = true").unwrap();
        assert_eq!(selected_path(&dir, "config/中文.toml").unwrap(), config.join("中文.toml"));
        assert_eq!(files_under(&selected_path(&dir, "config").unwrap()).unwrap(), [config.join("中文.toml")]);
        for path in ["missing", "../outside", "/outside", "config/../../outside"] {
            assert!(selected_path(&dir, path).is_err(), "{path}");
        }
        crate::test_support::cleanup(&[&dir]);
    }

    #[test]
    fn already_compressed_entries_are_stored_and_text_is_deflated() {
        crate::test_support::ensure_boot();
        let dir = crate::test_support::temp_dir("zip-entry-method");
        let jar = dir.join("a.jar");
        let text = dir.join("b.txt");
        // jar 换成本来就压不动的字节，txt 用高重复内容让 deflate 一定赢
        let bytes: Vec<u8> = (0..4096u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect();
        fs::write(&jar, &bytes).unwrap();
        fs::write(&text, "a".repeat(64 * 1024)).unwrap();
        let output = dir.join("methods.zip");
        let files = vec![("mods/a.jar".to_string(), jar), ("config/b.txt".to_string(), text)];
        write_zip(&output, &files, &[], &None, &AtomicBool::new(false)).unwrap();
        let mut zip = zip::ZipArchive::new(File::open(&output).unwrap()).unwrap();
        assert_eq!(zip.by_name("mods/a.jar").unwrap().compression(), zip::CompressionMethod::Stored);
        assert_eq!(zip.by_name("config/b.txt").unwrap().compression(), zip::CompressionMethod::Deflated);
        drop(zip);
        crate::test_support::cleanup(&[&dir]);
    }

    #[cfg(unix)]
    #[test]
    fn selected_paths_refuse_file_and_directory_symlinks() {
        crate::test_support::ensure_boot();
        let dir = crate::test_support::temp_dir("export-symlink");
        fs::write(dir.join("original"), "private").unwrap();
        std::os::unix::fs::symlink(dir.join("original"), dir.join("linked")).unwrap();
        assert!(selected_path(&dir, "linked").is_err());
        assert!(files_under(&dir).is_err());
        crate::test_support::cleanup(&[&dir]);
    }

}
