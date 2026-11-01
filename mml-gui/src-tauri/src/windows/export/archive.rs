//! 本地实例归档；不追随链接，写完后才替换目标文件。

use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};
use zip::ZipWriter;

use mml_base::archives::BaseArchiveGui;

use crate::{err_keys, log_text::LogText};

/// 递归收集目录下的全部文件，遇到链接直接报错
///
/// - `root`: 待遍历的目录
///
/// # 返回值
///
/// 目录下全部文件的绝对路径，已按路径排序
pub(crate) fn files_under(root: &Path) -> Result<Vec<PathBuf>, String> {
    reject_link(root)?;
    let mut files = Vec::new();
    for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let meta = reject_link(&entry.path())?;
        if meta.is_dir() {
            files.extend(files_under(&entry.path())?);
        } else if meta.is_file() {
            files.push(entry.path());
        }
    }
    files.sort();
    Ok(files)
}

/// 读元数据并拒绝符号链接与 Windows 重解析点
///
/// - `path`: 待检查的路径
///
/// # 返回值
///
/// 该路径的元数据；路径本身是链接时返回 `err.exportLink`
pub(super) fn reject_link(path: &Path) -> Result<fs::Metadata, String> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if meta.file_type().is_symlink() {
        return Err(err_keys::EXPORT_LINK.into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err(err_keys::EXPORT_LINK.into());
        }
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
    for part in path
        .strip_prefix(root)
        .map_err(|e| e.to_string())?
        .components()
    {
        current.push(part);
        let meta = reject_link(&current)?;
        if !meta.is_file() && !meta.is_dir() {
            return Err(err_keys::FILE_NAME.into());
        }
    }
    Ok(path)
}

/// 拒绝把导出文件写进源目录，否则下一次导出会把它自己也打进去
///
/// - `file`: 用户选择的输出路径
/// - `source`: 被导出的目录
///
/// # 返回值
///
/// 目标在源目录之外返回 `Ok(())`；落在源目录里返回 `err.exportDestination`
pub(super) fn check_destination(file: &Path, source: &Path) -> Result<(), String> {
    let parent = file
        .parent()
        .ok_or(err_keys::FILE_NAME)?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let source = source.canonicalize().map_err(|e| e.to_string())?;
    if parent.starts_with(source) {
        return Err(err_keys::EXPORT_DESTINATION.to_string());
    }
    Ok(())
}

/// 单次读写的块大小；`io::copy` 的 8KB 在写压缩包时系统调用开销明显
const COPY_BUF: usize = 256 * 1024;

/// 写一个 zip：文件按路径流式复制，内存数据直接写入
///
/// 先写同目录的临时文件，成功后才改名替换目标，失败或取消时删掉半成品。
///
/// - `file`: 输出 zip 路径
/// - `files`: 打包条目，依次为包内路径与源文件路径
/// - `data`: 直接写进包内存数据的条目，依次为包内路径与内容
/// - `gui`: 进度回调，`None` 表示不报进度
/// - `cancel`: 取消令牌，每写完一个文件检查一次
///
/// # 返回值
///
/// 写盘成功返回 `Ok(())`；取消、读文件或压缩失败返回错误文案
pub(crate) fn write_zip(
    file: &Path,
    files: &[(String, PathBuf)],
    data: &[(String, Vec<u8>)],
    gui: &BaseArchiveGui,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let temp = file.with_extension(format!("{}.part", uuid::Uuid::new_v4()));
    // 分四段计时：建包、写条目、收尾、改名，结束时一次打进日志
    let started = Instant::now();
    let result = (|| -> Result<(), String> {
        let handle = File::create_new(&temp).map_err(|e| e.to_string())?;
        let mut zip = ZipWriter::new(handle);
        let opened = Instant::now();
        let mut buffer = vec![0u8; COPY_BUF];
        if let Some(gui) = gui {
            gui.start(files.len() + data.len());
        }
        let mut index = 0;
        for (name, path) in files {
            // 每写一个文件看一次取消：一个文件内部的复制无法打断
            if cancel.load(Ordering::Relaxed) {
                return Err(err_keys::CANCELLED.to_string());
            }
            if let Some(gui) = gui {
                gui.update(Some(name.clone()), index);
            }
            zip.start_file(name, mml_base::archives::zip_entry_options(name, 0o644))
                .map_err(|e| e.to_string())?;
            let mut source = File::open(path).map_err(|e| e.to_string())?;
            loop {
                let read = source.read(&mut buffer).map_err(|e| e.to_string())?;
                if read == 0 {
                    break;
                }
                zip.write_all(&buffer[..read]).map_err(|e| e.to_string())?;
            }
            index += 1;
        }
        for (name, bytes) in data {
            if let Some(gui) = gui {
                gui.update(Some(name.clone()), index);
            }
            zip.start_file(name, mml_base::archives::zip_entry_options(name, 0o644))
                .map_err(|e| e.to_string())?;
            zip.write_all(bytes).map_err(|e| e.to_string())?;
            index += 1;
        }
        let written = Instant::now();
        zip.finish()
            .map_err(|e| e.to_string())?
            .sync_all()
            .map_err(|e| e.to_string())?;
        let finished = Instant::now();
        if let Some(gui) = gui {
            gui.done();
        }
        fs::rename(&temp, file).map_err(|e| e.to_string())?;
        mml_log::info(
            LogText::ExportPackDetail {
                open: (opened - started).as_secs_f64(),
                write: (written - opened).as_secs_f64(),
                finish: (finished - written).as_secs_f64(),
                commit: Some(Instant::now().duration_since(finished).as_secs_f64()),
                entries: files.len() + data.len(),
            }
            .text(),
        );
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// 整份归档一个实例，包内路径相对实例目录
///
/// - `game`: 实例配置，包里的 game.json 用当前快照
/// - `file`: 输出 zip
/// - `gui`: 进度回调
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 写盘成功返回 `Ok(())`；路径越界、读文件或压缩失败返回错误文案
pub(super) fn native_instance(
    game: &mml_game::launcher::instance_setting_obj::InstanceSettingObj,
    file: &Path,
    gui: &BaseArchiveGui,
    cancel: &AtomicBool,
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
    selected: &[std::path::PathBuf],
    file: &Path,
    gui: &BaseArchiveGui,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let root = game.get_base_path();
    native_archive_selected(game, &root, selected, file, gui, cancel)
}

/// 归档勾选文件的实现；root 显式传入，测试可以不初始化实例根目录
fn native_archive_selected(
    game: &mml_game::launcher::instance_setting_obj::InstanceSettingObj,
    root: &Path,
    selected: &[std::path::PathBuf],
    file: &Path,
    gui: &BaseArchiveGui,
    cancel: &AtomicBool,
) -> Result<(), String> {
    check_destination(file, root)?;
    let mut files = Vec::with_capacity(selected.len());
    for path in selected {
        let rel = path
            .strip_prefix(&root)
            .map_err(|_| err_keys::FILE_NAME.to_string())?;
        let name = rel.to_string_lossy().replace('\\', "/");
        if name == mml_names::names::GAME_FILE {
            continue;
        }
        files.push((name, path.clone()));
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let data = serde_json::to_vec_pretty(game).map_err(|e| e.to_string())?;
    write_zip(
        file,
        &files,
        &[(mml_names::names::GAME_FILE.to_string(), data)],
        gui,
        cancel,
    )
}

/// 归档实例目录，并用当前配置快照替换磁盘上尚未更新的配置
///
/// - `game`: 实例配置，包里的 game.json 用当前快照
/// - `root`: 实例根目录
/// - `file`: 输出 zip
/// - `gui`: 进度回调
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 写盘成功返回 `Ok(())`；路径越界、读文件或压缩失败返回错误文案
fn native_archive(
    game: &mml_game::launcher::instance_setting_obj::InstanceSettingObj,
    root: &Path,
    file: &Path,
    gui: &BaseArchiveGui,
    cancel: &AtomicBool,
) -> Result<(), String> {
    check_destination(file, root)?;
    let files = files_under(root)?
        .into_iter()
        .filter_map(|path| {
            let name = path
                .strip_prefix(root)
                .ok()?
                .to_string_lossy()
                .replace('\\', "/");
            (name != mml_names::names::GAME_FILE).then_some((name, path))
        })
        .collect::<Vec<_>>();
    let data = serde_json::to_vec_pretty(game).map_err(|e| e.to_string())?;
    write_zip(
        file,
        &files,
        &[(mml_names::names::GAME_FILE.to_string(), data)],
        gui,
        cancel,
    )
}

#[cfg(test)]
#[path = "../../../tests/unit/windows_export_archive_tests.rs"]
mod tests;
