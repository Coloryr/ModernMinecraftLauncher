//! 按目标格式将所选文件分配到下载清单或本地覆盖文件。

use std::{collections::HashMap, path::{Path, PathBuf}};
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
    for pick in picks.filter(|_| pack != ExportPackType::MML) {
        let path = archive::selected_path(base, &pick.path)?;
        if path.is_dir() {
            result.local.extend(collect_files(&path)?);
            continue;
        }
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
        if can_index && let Some(meta) = export_meta(&path) {
            let item = OnlineFileExport {
                file: path, size: meta.size, pack, url: url.to_string(),
                hash: meta.hash, info: source.cloned(),
            };
            if source.is_some() { result.mods.push(item); }
            else { result.files.push(item); }
        } else { result.local.push(path); }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

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
