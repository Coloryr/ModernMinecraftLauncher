//! 本地实例归档；不追随链接，写完后才替换目标文件。

use std::{fs::{self, File}, io::{self, Write}, path::{Path, PathBuf}};
use zip::{ZipWriter, write::SimpleFileOptions};

use crate::err_keys;

pub(super) fn files_under(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let meta = fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
        if meta.file_type().is_symlink() { return Err(err_keys::EXPORT_LINK.to_string()); }
        if meta.is_dir() { files.extend(files_under(&entry.path())?); }
        else if meta.is_file() { files.push(entry.path()); }
    }
    files.sort();
    Ok(files)
}

pub(super) fn check_destination(file: &Path, source: &Path) -> Result<(), String> {
    let parent = file.parent().ok_or(err_keys::FILE_NAME)?
        .canonicalize().map_err(|e| e.to_string())?;
    let source = source.canonicalize().map_err(|e| e.to_string())?;
    if parent.starts_with(source) { return Err(err_keys::EXPORT_DESTINATION.to_string()); }
    Ok(())
}

pub(super) fn write_zip(
    file: &Path, files: &[(String, PathBuf)], data: &[(String, Vec<u8>)],
) -> Result<(), String> {
    let temp = file.with_extension(format!("{}.part", uuid::Uuid::new_v4()));
    let result = (|| -> Result<(), String> {
        let handle = File::create_new(&temp).map_err(|e| e.to_string())?;
        let mut zip = ZipWriter::new(handle);
        let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, path) in files {
            zip.start_file(name, options).map_err(|e| e.to_string())?;
            io::copy(&mut File::open(path).map_err(|e| e.to_string())?, &mut zip).map_err(|e| e.to_string())?;
        }
        for (name, bytes) in data {
            zip.start_file(name, options).map_err(|e| e.to_string())?;
            zip.write_all(bytes).map_err(|e| e.to_string())?;
        }
        zip.finish().map_err(|e| e.to_string())?.sync_all().map_err(|e| e.to_string())?;
        fs::rename(&temp, file).map_err(|e| e.to_string())
    })();
    if result.is_err() { let _ = fs::remove_file(&temp); }
    result
}

pub(super) fn native_instance(
    game: &mml_game::launcher::instance_setting_obj::InstanceSettingObj, file: &Path,
) -> Result<(), String> {
    let root = game.get_base_path();
    check_destination(file, &root)?;
    let files = files_under(&root)?.into_iter().filter_map(|path| {
        let name = path.strip_prefix(&root).ok()?.to_string_lossy().replace('\\', "/");
        (name != mml_names::names::GAME_FILE).then_some((name, path))
    }).collect::<Vec<_>>();
    let data = serde_json::to_vec_pretty(game).map_err(|e| e.to_string())?;
    write_zip(file, &files, &[(mml_names::names::GAME_FILE.to_string(), data)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn archive_replaces_destination_without_including_itself() {
        let dir = crate::test_support::temp_dir("native-export");
        let source = dir.join("instance");
        fs::create_dir(&source).unwrap();
        let config = source.join("options.txt");
        fs::write(&config, "language:zh_cn").unwrap();
        let output = dir.join("instance.zip");
        fs::write(&output, "old archive").unwrap();
        assert!(check_destination(&source.join("bad.zip"), &source).is_err());
        write_zip(&output, &[(".minecraft/options.txt".into(), config)],
            &[("game.json".into(), br#"{"Name":"test","Version":"1.21.1"}"#.to_vec())]).unwrap();
        let mut archive = zip::ZipArchive::new(File::open(&output).unwrap()).unwrap();
        assert_eq!(archive.len(), 2);
        let mut contents = String::new();
        archive.by_name(".minecraft/options.txt").unwrap().read_to_string(&mut contents).unwrap();
        assert_eq!(contents, "language:zh_cn");
        drop(archive);
        crate::test_support::cleanup(&[&dir]);
    }
}
