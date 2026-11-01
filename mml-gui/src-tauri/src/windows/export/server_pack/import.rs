//! 先解包，再逐文件替换；中途失败时恢复已替换文件。

use super::{CONFIG, GUI_CONFIG, INSTANCES, LAUNCHER, MANIFEST, Manifest, archive, format};
use crate::{err_keys, gui_config::GuiConfig};
use mml_config::config_obj::ConfigObj;
use mml_names::names;
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
};
use zip::ZipArchive;

/// 导入成功后替换进程内配置的数据。
pub(crate) struct ImportedPack {
    /// 导入后的内核配置；服务器包没带这一份时为 `None`。
    pub config: Option<ConfigObj>,
    /// 导入后的界面配置；服务器包没带这一份时为 `None`。
    pub gui: Option<GuiConfig>,
    /// 包里自带的 Java 运行时名称，落在运行目录的 java/ 下；导入后要登记进 Java 列表
    pub java: Vec<String>,
}

/// 已解包并等待替换的文件。
struct StagedFile {
    /// 相对运行目录的路径。
    relative: PathBuf,
    /// 安装位置。
    target: PathBuf,
    /// 暂存文件。
    staged: PathBuf,
}

/// 回滚时需要恢复的目标与备份。
struct AppliedFile {
    /// 已替换的目标文件。
    target: PathBuf,
    /// 原文件的备份位置。
    backup: PathBuf,
    /// 替换前是否有文件。
    existed: bool,
}

/// 解析安装位置，并逐段拒绝符号链接与 Windows 重解析点
///
/// 目标可以还不存在，只有已存在的路径段才做链接检查。
///
/// - `root`: 运行目录
/// - `relative`: 包内条目对应的相对路径
///
/// # 返回值
///
/// 安装位置的绝对路径；路径上有链接时返回 `err.exportLink`
fn destination(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    let mut path = root.to_path_buf();
    for component in relative.components() {
        path.push(component);
        if fs::symlink_metadata(&path).is_ok() {
            archive::reject_link(&path)?;
        }
    }
    Ok(path)
}

/// 解包并逐文件替换，失败时回滚已替换的文件
///
/// 先把整包解到运行目录下的暂存目录，确认全部可写之后才逐个替换，
/// 中途失败时按相反顺序把备份放回去。
///
/// - `file`: 服务器包
/// - `root`: 运行目录
///
/// # 返回值
///
/// 已替换的目标文件与导入后的配置；校验、解包或写盘失败返回错误文案
pub(crate) fn import_pack(
    file: &Path,
    root: &Path,
) -> Result<mml_config::config_save::ReplacedFiles<ImportedPack>, String> {
    let mut zip =
        ZipArchive::new(File::open(file).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let manifest = format::validate(&mut zip)?;
    let config = if zip.by_name(CONFIG).is_ok() {
        Some(format::read_json(&mut zip, CONFIG)?)
    } else {
        None
    };
    let gui = if zip.by_name(GUI_CONFIG).is_ok() {
        Some(format::read_json(&mut zip, GUI_CONFIG)?)
    } else {
        None
    };
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let directories = instance_directories(&root, &manifest)?;
    let stage = root.join(format!(".server-import-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&stage).map_err(|e| e.to_string())?;
    let result = (|| {
        let mut files = Vec::new();
        let mut java = BTreeSet::new();
        for index in 0..zip.len() {
            let mut entry = zip.by_index(index).map_err(|e| e.to_string())?;
            let name = entry.name().map_err(|e| e.to_string())?;
            if entry.is_dir() || matches!(name.as_ref(), MANIFEST | LAUNCHER) {
                continue;
            }
            let name = name.into_owned();
            let (relative, game_directory) = if let Some(rest) = name.strip_prefix(INSTANCES) {
                let (directory, suffix) =
                    rest.split_once('/').ok_or(err_keys::SERVER_PACK_INVALID)?;
                let target = directories
                    .get(directory)
                    .ok_or(err_keys::SERVER_PACK_INVALID)?;
                (
                    PathBuf::from(format!("minecraft/instances/{target}/{suffix}")),
                    (suffix == "game.json").then_some(target),
                )
            } else {
                (
                    PathBuf::from(
                        name.strip_prefix("mml/")
                            .ok_or(err_keys::SERVER_PACK_INVALID)?,
                    ),
                    None,
                )
            };
            // 包内自带 Java：记下运行时名称，导入完成后登记进 Java 列表
            if let Ok(rest) = relative.strip_prefix(names::JAVA_DIR)
                && let Some(name) = rest.components().next()
            {
                java.insert(name.as_os_str().to_string_lossy().to_string());
            }
            let is_lock = relative == Path::new(crate::client_lock::FILE_NAME);
            let target = destination(&root, &relative)?;
            let staged = stage.join("files").join(&relative);
            fs::create_dir_all(staged.parent().unwrap()).map_err(|e| e.to_string())?;
            let mut output = File::create_new(&staged).map_err(|e| e.to_string())?;
            if is_lock {
                // 锁文件先读进内存校验：坏包不该把玩家锁在一个乱码密码上
                let mut data = Vec::new();
                entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
                if !crate::client_lock::parse_ok(&data) {
                    return Err(err_keys::SERVER_PACK_INVALID.into());
                }
                io::copy(&mut data.as_slice(), &mut output).map_err(|e| e.to_string())?;
            } else if let Some(directory) = game_directory {
                let mut game: mml_game::launcher::instance_setting_obj::InstanceSettingObj =
                    serde_json::from_reader(&mut entry)
                        .map_err(|_| err_keys::SERVER_PACK_INVALID)?;
                game.dir = directory.clone();
                serde_json::to_writer_pretty(&mut output, &game).map_err(|e| e.to_string())?;
            } else {
                io::copy(&mut entry, &mut output).map_err(|e| e.to_string())?;
            }
            output.sync_all().map_err(|e| e.to_string())?;
            files.push(StagedFile {
                relative,
                target,
                staged,
            });
        }
        let mut applied = Vec::new();
        for StagedFile {
            relative,
            target,
            staged,
        } in files
        {
            let backup = stage.join("backup").join(&relative);
            let is_lock = relative == Path::new(crate::client_lock::FILE_NAME);
            let replace = (|| -> Result<(), String> {
                destination(&root, &relative)?;
                fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
                let existed = target.exists();
                if existed {
                    if !target.is_file() {
                        return Err(err_keys::SERVER_PACK_INVALID.into());
                    }
                    fs::create_dir_all(backup.parent().unwrap()).map_err(|e| e.to_string())?;
                    fs::rename(&target, &backup).map_err(|e| e.to_string())?;
                }
                applied.push(AppliedFile {
                    target: target.clone(),
                    backup,
                    existed,
                });
                fs::rename(&staged, &target).map_err(|e| e.to_string())?;
                // 锁文件落到运行目录后补回隐藏属性
                if is_lock {
                    crate::client_lock::hide(&target);
                }
                Ok(())
            })();
            if let Err(error) = replace {
                let mut rollback_failed = false;
                for AppliedFile {
                    target: path,
                    backup,
                    existed,
                } in applied.into_iter().rev()
                {
                    if path.exists() && fs::remove_file(&path).is_err() {
                        rollback_failed = true;
                        continue;
                    }
                    if existed && fs::rename(&backup, &path).is_err() {
                        rollback_failed = true;
                    }
                }
                if rollback_failed {
                    return Err(format!("{error}; backup: {}", stage.display()));
                }
                return Err(error);
            }
        }
        Ok(mml_config::config_save::ReplacedFiles {
            files: applied.into_iter().map(|file| file.target).collect(),
            value: ImportedPack {
                config,
                gui,
                java: java.into_iter().collect(),
            },
        })
    })();
    // 恢复失败时保留备份，避免丢掉旧文件。
    if !result
        .as_ref()
        .err()
        .is_some_and(|e: &String| e.contains("; backup:"))
    {
        let _ = fs::remove_dir_all(&stage);
    }
    result
}

/// 同 UUID 已经安装时覆盖原目录，避免重启扫描出两个相同实例
///
/// - `root`: 运行目录
/// - `manifest`: 包内清单
///
/// # 返回值
///
/// 包内目录名 → 实际安装的目录名；同 UUID 对应多个目录或目录名非法时返回错误
fn instance_directories(
    root: &Path,
    manifest: &Manifest,
) -> Result<HashMap<String, String>, String> {
    let mut existing = HashMap::new();
    let base = root.join("minecraft/instances");
    if base.exists() {
        destination(root, Path::new("minecraft/instances"))?;
        for entry in fs::read_dir(&base).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if !archive::reject_link(&entry.path()).is_ok_and(|meta| meta.is_dir()) {
                continue;
            }
            let file = entry.path().join("game.json");
            if !archive::reject_link(&file)
                .is_ok_and(|meta| meta.is_file() && meta.len() <= 4 * 1024 * 1024)
            {
                continue;
            }
            let Ok(file) = File::open(file) else {
                continue;
            };
            let Ok(game) = serde_json::from_reader::<
                _,
                mml_game::launcher::instance_setting_obj::InstanceSettingObj,
            >(file.take(4 * 1024 * 1024)) else {
                continue;
            };
            if manifest.instances.iter().any(|item| item.uuid == game.uuid) {
                let directory = entry.file_name().to_string_lossy().into_owned();
                if !format::safe_component(&directory)
                    || existing.insert(game.uuid, directory).is_some()
                {
                    return Err(err_keys::SERVER_PACK_INVALID.into());
                }
            }
        }
    }
    let mut used = HashSet::new();
    let mut result = HashMap::new();
    for instance in &manifest.instances {
        let target = existing
            .get(&instance.uuid)
            .unwrap_or(&instance.directory)
            .clone();
        if !used.insert(target.to_lowercase()) {
            return Err(err_keys::SERVER_PACK_INVALID.into());
        }
        result.insert(instance.directory.clone(), target);
    }
    Ok(result)
}

#[cfg(test)]
#[path = "../../../../tests/unit/windows_export_server_pack_import_tests.rs"]
mod tests;
