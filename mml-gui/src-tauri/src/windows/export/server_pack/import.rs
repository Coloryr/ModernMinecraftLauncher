//! 先解包，再逐文件替换；中途失败时恢复已替换文件。

use std::{fs::{self, File}, io, path::{Path, PathBuf}};
use mml_config::config_obj::ConfigObj;
use zip::ZipArchive;
use crate::{err_keys, gui_config::GuiConfig};
use super::{CONFIG, GUI_CONFIG, LAUNCHER, MANIFEST, format};

fn destination(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    let mut path = root.to_path_buf();
    for component in relative.components() {
        path.push(component);
        if let Ok(meta) = fs::symlink_metadata(&path) {
            if meta.file_type().is_symlink() { return Err(err_keys::EXPORT_LINK.into()); }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if meta.file_attributes() & 0x400 != 0 { return Err(err_keys::EXPORT_LINK.into()); }
            }
        }
    }
    Ok(path)
}

pub(crate) fn import_pack(file: &Path, root: &Path) -> Result<(ConfigObj, GuiConfig), String> {
    let mut zip = ZipArchive::new(File::open(file).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let _manifest = format::validate(&mut zip)?;
    let config = format::read_json(&mut zip, CONFIG)?;
    let gui = format::read_json(&mut zip, GUI_CONFIG)?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let stage = root.join(format!(".server-import-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&stage).map_err(|e| e.to_string())?;
    let result = (|| {
        let mut files = Vec::new();
        for index in 0..zip.len() {
            let mut entry = zip.by_index(index).map_err(|e| e.to_string())?;
            let name = entry.name().map_err(|e| e.to_string())?;
            if entry.is_dir() || matches!(name.as_ref(), MANIFEST | LAUNCHER) { continue; }
            let relative = PathBuf::from(name.strip_prefix("mml/").ok_or(err_keys::SERVER_PACK_INVALID)?);
            let target = destination(&root, &relative)?;
            let staged = stage.join("files").join(&relative);
            fs::create_dir_all(staged.parent().unwrap()).map_err(|e| e.to_string())?;
            let mut output = File::create_new(&staged).map_err(|e| e.to_string())?;
            io::copy(&mut entry, &mut output).map_err(|e| e.to_string())?;
            output.sync_all().map_err(|e| e.to_string())?;
            files.push((relative, target, staged));
        }
        let mut applied = Vec::new();
        for (relative, target, staged) in files {
            let backup = stage.join("backup").join(&relative);
            let replace = (|| -> Result<(), String> {
                destination(&root, &relative)?;
                fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
                let existed = target.exists();
                if existed {
                    if !target.is_file() { return Err(err_keys::SERVER_PACK_INVALID.into()); }
                    fs::create_dir_all(backup.parent().unwrap()).map_err(|e| e.to_string())?;
                    fs::rename(&target, &backup).map_err(|e| e.to_string())?;
                }
                applied.push((target.clone(), backup, existed));
                fs::rename(staged, &target).map_err(|e| e.to_string())
            })();
            if let Err(error) = replace {
                let mut rollback_failed = false;
                for (path, backup, existed) in applied.into_iter().rev() {
                    if path.exists() && fs::remove_file(&path).is_err() { rollback_failed = true; continue; }
                    if existed && fs::rename(&backup, &path).is_err() { rollback_failed = true; }
                }
                if rollback_failed { return Err(format!("{error}; backup: {}", stage.display())); }
                return Err(error);
            }
        }
        Ok((config, gui))
    })();
    // 恢复失败时保留备份，避免丢掉旧文件。
    if !result.as_ref().err().is_some_and(|e: &String| e.contains("; backup:")) {
        let _ = fs::remove_dir_all(&stage);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{archive, Manifest, Instance};

    fn fixture(extra: Option<(&str, &[u8])>) -> (PathBuf, PathBuf) {
        let dir = crate::test_support::temp_dir("server-pack");
        let file = dir.join("pack.zip");
        let uuid = uuid::Uuid::new_v4();
        let manifest = Manifest { format: "mml-server-pack".into(), version: 1,
            instances: vec![Instance { uuid, directory: "test".into() }], launcher: false };
        let mut data = vec![
            (MANIFEST.into(), serde_json::to_vec(&manifest).unwrap()),
            (CONFIG.into(), b"{}".to_vec()), (GUI_CONFIG.into(), b"{}".to_vec()),
            ("mml/minecraft/instances/test/game.json".into(), format!(r#"{{"UUID":"{uuid}","DirName":"test","Name":"Test"}}"#).into_bytes()),
            ("mml/minecraft/instances/test/.minecraft/options.txt".into(), b"new".to_vec()),
        ];
        if let Some((name, content)) = extra { data.push((name.into(), content.to_vec())); }
        archive::write_zip(&file, &[], &data).unwrap();
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
        assert_eq!(fs::read_to_string(root.join("accounts.json")).unwrap(), "private");
        assert!(root.join("gui_config.json").is_file());
        crate::test_support::cleanup(&[&dir]);
    }

    #[test]
    fn rejects_traversal_and_unrelated_files_before_overwriting() {
        for name in ["mml/../outside", "mml/accounts.json", "mml/minecraft/instances/test/CON", "mml/minecraft/instances/test/a:stream"] {
            let (dir, file) = fixture(Some((name, b"bad")));
            fs::write(dir.join("config.json"), "original").unwrap();
            assert!(import_pack(&file, &dir).is_err(), "{name}");
            assert_eq!(fs::read_to_string(dir.join("config.json")).unwrap(), "original");
            crate::test_support::cleanup(&[&dir]);
        }
    }
}
