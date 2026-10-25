//! 先解包，再逐文件替换；中途失败时恢复已替换文件。

use super::{CONFIG, GUI_CONFIG, INSTANCES, LAUNCHER, MANIFEST, Manifest, archive, format};
use crate::{err_keys, gui_config::GuiConfig};
use mml_config::config_obj::ConfigObj;
use std::{
    collections::{HashMap, HashSet},
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

fn destination(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    let mut path = root.to_path_buf();
    for component in relative.components() {
        path.push(component);
        if let Ok(meta) = fs::symlink_metadata(&path) {
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
        }
    }
    Ok(path)
}

/// 解包并逐文件替换
///
/// - `file`: 服务器包
/// - `root`: 运行目录
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
            value: ImportedPack { config, gui },
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

// 同 UUID 已经安装时覆盖原目录，避免重启扫描出两个相同实例。
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
mod tests {
    use super::super::{Instance, Manifest, archive};
    use super::*;

    fn fixture(extra: Option<(&str, &[u8])>) -> (PathBuf, PathBuf) {
        crate::test_support::ensure_boot();
        let dir = crate::test_support::temp_dir("server-pack");
        let file = dir.join("pack.zip");
        let uuid = uuid::Uuid::new_v4();
        let manifest = Manifest {
            format: "mml-server-pack".into(),
            version: 1,
            instances: vec![Instance {
                uuid,
                directory: "test".into(),
            }],
            launcher: extra.is_some_and(|(name, _)| name == LAUNCHER),
        };
        let mut data = vec![
            (MANIFEST.into(), serde_json::to_vec(&manifest).unwrap()),
            (CONFIG.into(), b"{}".to_vec()),
            (GUI_CONFIG.into(), b"{}".to_vec()),
            (
                "mml/minecraft/instances/test/game.json".into(),
                format!(r#"{{"UUID":"{uuid}","DirName":"test","Name":"Test"}}"#).into_bytes(),
            ),
            (
                "mml/minecraft/instances/test/.minecraft/options.txt".into(),
                b"new".to_vec(),
            ),
        ];
        if let Some((name, content)) = extra {
            data.push((name.into(), content.to_vec()));
        }
        archive::write_zip(
            &file,
            &[],
            &data,
            &None,
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();
        (dir, file)
    }

    #[test]
    fn overwrites_configs_and_instances_but_preserves_other_files() {
        let (dir, file) = fixture(None);
        let root = dir.join("runtime");
        let options = root.join("minecraft/instances/test/.minecraft/options.txt");
        fs::create_dir_all(options.parent().unwrap()).unwrap();
        fs::write(&options, "old").unwrap();
        fs::write(root.join("accounts.json"), "private").unwrap();
        import_pack(&file, &root).unwrap();
        assert_eq!(fs::read_to_string(options).unwrap(), "new");
        assert_eq!(
            fs::read_to_string(root.join("accounts.json")).unwrap(),
            "private"
        );
        assert!(root.join("gui_config.json").is_file());
        crate::test_support::cleanup(&[&dir]);
    }

    #[test]
    fn rejects_traversal_and_unrelated_files_before_overwriting() {
        for name in [
            "mml/../outside",
            "mml/accounts.json",
            "mml/minecraft/instances/test/CON",
            "mml/minecraft/instances/test/a:stream",
        ] {
            let (dir, file) = fixture(Some((name, b"bad")));
            fs::write(dir.join("config.json"), "original").unwrap();
            assert!(import_pack(&file, &dir).is_err(), "{name}");
            assert_eq!(
                fs::read_to_string(dir.join("config.json")).unwrap(),
                "original"
            );
            crate::test_support::cleanup(&[&dir]);
        }
    }

    #[test]
    fn failed_overwrite_restores_configs_and_removes_new_files() {
        let (dir, file) = fixture(None);
        let root = dir.join("runtime");
        let conflict = root.join("minecraft/instances/test/.minecraft/options.txt");
        fs::create_dir_all(&conflict).unwrap();
        fs::write(root.join("config.json"), "original core").unwrap();
        fs::write(root.join("gui_config.json"), "original gui").unwrap();
        assert!(import_pack(&file, &root).is_err());
        assert_eq!(
            fs::read_to_string(root.join("config.json")).unwrap(),
            "original core"
        );
        assert_eq!(
            fs::read_to_string(root.join("gui_config.json")).unwrap(),
            "original gui"
        );
        assert!(!root.join("minecraft/instances/test/game.json").exists());
        assert!(conflict.is_dir());
        assert!(fs::read_dir(&root).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".server-import-")
        }));
        crate::test_support::cleanup(&[&dir]);
    }
    #[test]
    fn reimport_follows_instance_uuid_and_never_replaces_launcher() {
        let (dir, file) = fixture(Some((LAUNCHER, b"bundled launcher")));
        let root = dir.join("runtime");
        fs::create_dir(&root).unwrap();
        import_pack(&file, &root).unwrap();
        let original = root.join("minecraft/instances/test");
        let renamed = root.join("minecraft/instances/renamed");
        fs::rename(&original, &renamed).unwrap();
        fs::write(renamed.join(".minecraft/options.txt"), "old").unwrap();
        fs::write(root.join(LAUNCHER), "running launcher").unwrap();
        import_pack(&file, &root).unwrap();
        assert!(!original.exists());
        assert_eq!(
            fs::read_to_string(renamed.join(".minecraft/options.txt")).unwrap(),
            "new"
        );
        let game: serde_json::Value =
            serde_json::from_slice(&fs::read(renamed.join("game.json")).unwrap()).unwrap();
        assert_eq!(game["DirName"], "renamed");
        assert_eq!(
            fs::read_to_string(root.join(LAUNCHER)).unwrap(),
            "running launcher"
        );
        crate::test_support::cleanup(&[&dir]);
    }

    #[test]
    fn imports_without_configs_and_keeps_existing_ones() {
        crate::test_support::ensure_boot();
        let dir = crate::test_support::temp_dir("server-pack-partial");
        let file = dir.join("pack.zip");
        let uuid = uuid::Uuid::new_v4();
        let manifest = Manifest {
            format: "mml-server-pack".into(),
            version: 1,
            instances: vec![Instance {
                uuid,
                directory: "test".into(),
            }],
            launcher: false,
        };
        let data = vec![
            (MANIFEST.into(), serde_json::to_vec(&manifest).unwrap()),
            (
                "mml/minecraft/instances/test/game.json".into(),
                format!(r#"{{"UUID":"{uuid}","DirName":"test","Name":"Test"}}"#).into_bytes(),
            ),
            (
                "mml/minecraft/instances/test/.minecraft/options.txt".into(),
                b"new".to_vec(),
            ),
            ("mml/custom_ui/page.zip".into(), b"page".to_vec()),
        ];
        archive::write_zip(
            &file,
            &[],
            &data,
            &None,
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();

        let root = dir.join("runtime");
        fs::create_dir_all(root.join("minecraft/instances/test/.minecraft")).unwrap();
        fs::write(root.join("config.json"), "keep-core").unwrap();
        fs::write(root.join("gui_config.json"), "keep-gui").unwrap();
        let imported = import_pack(&file, &root).unwrap();
        assert!(imported.value.config.is_none());
        assert!(imported.value.gui.is_none());
        assert_eq!(
            fs::read_to_string(root.join("config.json")).unwrap(),
            "keep-core"
        );
        assert_eq!(
            fs::read_to_string(root.join("gui_config.json")).unwrap(),
            "keep-gui"
        );
        assert!(root.join("custom_ui/page.zip").is_file());
        crate::test_support::cleanup(&[&dir]);
    }

    #[test]
    fn installs_client_lock_in_run_dir() {
        crate::test_support::ensure_boot();
        let dir = crate::test_support::temp_dir("server-pack-lock");
        let file = dir.join("pack.zip");
        let uuid = uuid::Uuid::new_v4();
        let manifest = Manifest {
            format: "mml-server-pack".into(),
            version: 1,
            instances: vec![Instance {
                uuid,
                directory: "test".into(),
            }],
            launcher: false,
        };
        let data = vec![
            (MANIFEST.into(), serde_json::to_vec(&manifest).unwrap()),
            (
                "mml/.client_lock.json".into(),
                br#"{"Password":"$pbkdf2-sha256$120000$aa$bb"}"#.to_vec(),
            ),
            (
                "mml/minecraft/instances/test/game.json".into(),
                format!(r#"{{"UUID":"{uuid}","DirName":"test","Name":"Test"}}"#).into_bytes(),
            ),
        ];
        archive::write_zip(
            &file,
            &[],
            &data,
            &None,
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();

        let root = dir.join("runtime");
        fs::create_dir_all(&root).unwrap();
        import_pack(&file, &root).unwrap();
        // 锁文件落在运行目录，跟 gui_config.json 同级
        assert!(root.join(".client_lock.json").is_file());
        crate::test_support::cleanup(&[&dir]);
    }

    #[test]
    fn failed_import_restores_previous_lock() {
        crate::test_support::ensure_boot();
        let dir = crate::test_support::temp_dir("server-pack-lock-rollback");
        let file = dir.join("pack.zip");
        let uuid = uuid::Uuid::new_v4();
        let manifest = Manifest {
            format: "mml-server-pack".into(),
            version: 1,
            instances: vec![Instance {
                uuid,
                directory: "test".into(),
            }],
            launcher: false,
        };
        let data = vec![
            (MANIFEST.into(), serde_json::to_vec(&manifest).unwrap()),
            (
                "mml/.client_lock.json".into(),
                br#"{"Salt":"new","Hash":"new"}"#.to_vec(),
            ),
            (
                "mml/minecraft/instances/test/game.json".into(),
                format!(r#"{{"UUID":"{uuid}","DirName":"test","Name":"Test"}}"#).into_bytes(),
            ),
            (
                "mml/minecraft/instances/test/.minecraft/options.txt".into(),
                b"new".to_vec(),
            ),
        ];
        archive::write_zip(
            &file,
            &[],
            &data,
            &None,
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();

        let root = dir.join("runtime");
        let conflict = root.join("minecraft/instances/test/.minecraft/options.txt");
        fs::create_dir_all(&conflict).unwrap();
        let old = br#"{"Salt":"old","Hash":"old"}"#;
        fs::write(root.join(".client_lock.json"), old).unwrap();
        assert!(import_pack(&file, &root).is_err());
        // 文件回滚的同时，锁也要回到导入前那份
        assert_eq!(
            fs::read(root.join(".client_lock.json")).unwrap(),
            old.to_vec()
        );
        crate::test_support::cleanup(&[&dir]);
    }
}
