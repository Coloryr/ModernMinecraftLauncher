//! 按目标格式将所选文件分配到下载清单或本地覆盖文件。

use std::{collections::{HashMap, HashSet}, path::{Path, PathBuf}};
use mml_game::{game_export::{ExportPackType, OnlineFileExport}, launcher::file_online_info_obj::OnlineInfoObj};
use crate::dtos::export_dto::ExportPickDto;
use super::{archive, list::{collect_files, export_meta}};

/// 一个文件只使用目标格式能够识别的在线来源。
#[derive(Default)]
pub(super) struct Selection {
    /// 带项目来源的下载项。
    pub(super) mods: Vec<OnlineFileExport>,
    /// 用户填写下载地址的文件。
    pub(super) files: Vec<OnlineFileExport>,
    /// 直接打包的本地文件。
    pub(super) local: Vec<PathBuf>,
}

/// 路由选中的文件，无法索引的来源保留为本地文件。
/// - `base`: 游戏目录。
/// - `picks`: 模组与普通文件的选择。
/// - `online`: 文件名对应的在线来源。
/// - `pack`: 导出格式。
/// # 返回值
/// 各类导出条目；路径不可读或含链接时失败。
pub(super) fn collect<'a>(
    base: &Path, picks: impl Iterator<Item = &'a ExportPickDto>,
    online: &HashMap<String, OnlineInfoObj>, pack: ExportPackType,
) -> Result<Selection, String> {
    let mut result = Selection::default();
    let mut explicit = HashSet::new();
    for pick in picks.filter(|_| pack != ExportPackType::MML) {
        let path = archive::selected_path(base, &pick.path)?;
        if path.is_dir() {
            result.local.extend(collect_files(&path)?);
            continue;
        }
        if !explicit.insert(path.clone()) { continue; }
        if matches!(pack, ExportPackType::MMC | ExportPackType::HMCL) {
            result.local.push(path);
            continue;
        }
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let source = online.get(name.as_ref());
        let url = if pick.url.trim().is_empty() {
            source.map(|info| info.url.trim()).unwrap_or_default()
        } else { pick.url.trim() };
        let can_index = match pack {
            ExportPackType::CurseForge => source.is_some_and(|info|
                [&info.modid, &info.fileid].iter().all(|id| id.parse::<u64>().is_ok_and(|n| n > 0))),
            ExportPackType::Modrinth => !url.is_empty(),
            _ => false,
        };
        if can_index {
            let meta = export_meta(&path)?;
            let item = OnlineFileExport {
                file: path, size: meta.size, pack, url: url.to_string(),
                hash: meta.hash, info: source.cloned(),
            };
            if source.is_some() { result.mods.push(item); }
            else { result.files.push(item); }
        } else { result.local.push(path); }
    }
    // 文件既能在模组页选中，也能从目录展开；下载项优先，每个路径只导出一次。
    let indexed: HashSet<_> = result.mods.iter().chain(&result.files).map(|item| &item.file).collect();
    result.local.retain(|path| !indexed.contains(path));
    result.local.sort();
    result.local.dedup();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn unreadable_indexed_file_fails_instead_of_emitting_empty_hashes() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = crate::test_support::temp_dir("export-locked-download");
        let path = dir.join("mod.jar");
        std::fs::write(&path, b"mod").unwrap();
        let locked = std::fs::OpenOptions::new().read(true).share_mode(0).open(&path).unwrap();
        assert!(std::fs::metadata(&path).is_ok());
        let picks = [ExportPickDto { path: "mod.jar".into(), url: "https://example.invalid/mod.jar".into() }];
        let online = HashMap::from([("mod.jar".into(), OnlineInfoObj {
            modid: "123".into(), fileid: "456".into(), ..Default::default()
        })]);
        for pack in [ExportPackType::CurseForge, ExportPackType::Modrinth] {
            assert!(collect(&dir, picks.iter(), &online, pack).is_err());
        }
        drop(locked);
        crate::test_support::cleanup(&[&dir]);
    }

    #[test]
    fn overlapping_directories_and_files_are_written_once() {
        let dir = crate::test_support::temp_dir("export-overlap");
        std::fs::create_dir_all(dir.join("config/nested")).unwrap();
        std::fs::write(dir.join("config/nested/settings.toml"), "enabled = true").unwrap();
        let picks: Vec<_> = ["config", "config/nested", "config/nested/settings.toml", "config/nested/settings.toml"]
            .into_iter().map(|path| ExportPickDto { path: path.into(), url: "".into() }).collect();
        let result = collect(&dir, picks.iter(), &HashMap::new(), ExportPackType::HMCL).unwrap();
        assert_eq!(result.local, [dir.join("config/nested/settings.toml")]);
        crate::test_support::cleanup(&[&dir]);
    }

    #[test]
    fn indexed_file_is_not_duplicated_in_overrides() {
        let dir = crate::test_support::temp_dir("export-index-overlap");
        std::fs::create_dir(dir.join("mods")).unwrap();
        std::fs::write(dir.join("mods/a.jar"), b"a").unwrap();
        std::fs::write(dir.join("mods/b.jar"), b"b").unwrap();
        let picks: Vec<_> = ["mods", "mods/a.jar", "mods/a.jar"]
            .into_iter().map(|path| ExportPickDto { path: path.into(), url: "https://example.invalid/a".into() }).collect();
        for picks in [picks.clone(), picks.into_iter().rev().collect()] {
            let result = collect(&dir, picks.iter(), &HashMap::new(), ExportPackType::Modrinth).unwrap();
            assert_eq!(result.local, [dir.join("mods/b.jar")]);
            assert_eq!(result.files.len(), 1);
            assert_eq!(result.files[0].file, dir.join("mods/a.jar"));
        }
        crate::test_support::cleanup(&[&dir]);
    }

    #[test]
    fn curseforge_keeps_foreign_or_invalid_ids_as_local_files() {
        let dir = crate::test_support::temp_dir("export-source");
        std::fs::write(dir.join("mod.jar"), b"mod").unwrap();
        let picks = [ExportPickDto { path: "mod.jar".into(), url: "".into() }];
        for (project, file) in [("modrinth", "version"), ("0", "1"), ("1", "0"), ("123", ""), ("123", "456")] {
            let online = HashMap::from([("mod.jar".into(), OnlineInfoObj {
                modid: project.into(), fileid: file.into(), ..Default::default()
            })]);
            let result = collect(&dir, picks.iter(), &online, ExportPackType::CurseForge).unwrap();
            let indexed = project == "123" && file == "456";
            assert_eq!(result.mods.len(), usize::from(indexed), "{project}/{file}");
            assert_eq!(result.local.len(), usize::from(!indexed), "{project}/{file}");
            assert!(result.files.is_empty());
        }
        crate::test_support::cleanup(&[&dir]);
    }

    #[test]
    fn modrinth_uses_edited_or_saved_urls_and_falls_back_to_local() {
        let dir = crate::test_support::temp_dir("export-downloads");
        std::fs::write(dir.join("mod.jar"), b"mod").unwrap();
        let online = HashMap::from([("mod.jar".into(), OnlineInfoObj {
            url: "https://example.invalid/saved".into(), ..Default::default()
        })]);
        for url in ["", "  https://example.invalid/edited  "] {
            let picks = [ExportPickDto { path: "mod.jar".into(), url: url.into() }];
            let result = collect(&dir, picks.iter(), &online, ExportPackType::Modrinth).unwrap();
            assert_eq!(result.mods[0].url, if url.is_empty() { "https://example.invalid/saved" } else { url.trim() });
            let local = collect(&dir, picks.iter(), &HashMap::new(), ExportPackType::Modrinth).unwrap();
            assert_eq!(local.local.len(), usize::from(url.is_empty()));
            assert_eq!(local.files.len(), usize::from(!url.is_empty()));
            for pack in [ExportPackType::MMC, ExportPackType::HMCL] {
                let whole = collect(&dir, picks.iter(), &online, pack).unwrap();
                assert_eq!(whole.local, [dir.join("mod.jar")]);
                assert!(whole.mods.is_empty());
            }
        }
        crate::test_support::cleanup(&[&dir]);
    }
}
