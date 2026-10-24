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
/// - `picks`: 模组与普通文件的选择，含取消勾选的排除项。
/// - `online`: 文件名对应的在线来源。
/// - `pack`: 导出格式。
/// # 返回值
/// 各类导出条目；路径不可读或含链接时失败。
pub(super) fn collect<'a>(
    base: &Path, picks: impl Iterator<Item = &'a ExportPickDto>,
    online: &HashMap<String, OnlineInfoObj>, pack: ExportPackType,
) -> Result<Selection, String> {
    let picks: Vec<&ExportPickDto> = picks.collect();
    let decisions = decisions_of(base, &picks)?;
    let mut result = Selection::default();
    let mut explicit = HashSet::new();
    // MML（直接压缩）也认勾选：勾了什么就只压什么，见 native_instance_selected
    for pick in picks {
        if pick.exclude { continue; }
        let path = archive::selected_path(base, &pick.path)?;
        // 同一个路径既勾又取消时以判定表里最后一条为准
        if is_excluded(&decisions, &path) { continue; }
        if path.is_dir() {
            let files = collect_files(&path)?;
            result.local.extend(files.into_iter().filter(|file| !is_excluded(&decisions, file)));
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
        // 项目号 / 文件号可以在模组页改：填了以填的为准，留空才用实例里的在线信息
        let modid = pick_id(pick.mod_id.as_deref())
            .or_else(|| source.map(|info| info.modid.clone())).unwrap_or_default();
        let fileid = pick_id(pick.file_id.as_deref())
            .or_else(|| source.map(|info| info.fileid.clone())).unwrap_or_default();
        let indexed = !modid.is_empty() && !fileid.is_empty();
        let can_index = match pack {
            ExportPackType::CurseForge => indexed
                && [&modid, &fileid].iter().all(|id| id.parse::<u64>().is_ok_and(|n| n > 0)),
            ExportPackType::Modrinth => !url.is_empty(),
            _ => false,
        };
        if can_index {
            let meta = export_meta(&path)?;
            // 有项目号 / 文件号才进下载清单，否则按 overrides 整包带着
            let info = indexed.then(|| OnlineInfoObj {
                file: name.to_string(), modid: modid.clone(), fileid: fileid.clone(),
                url: url.to_string(), ..Default::default()
            });
            let item = OnlineFileExport {
                file: path, size: meta.size, pack, url: url.to_string(),
                hash: meta.hash, info,
            };
            if indexed { result.mods.push(item); }
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

/// 前端填的编号：去空白，空串当作没填
///
/// - `id`: 前端给的项目号或文件号
///
/// # 返回值
///
/// 去掉首尾空白后的编号；没填返回 `None`
fn pick_id(id: Option<&str>) -> Option<String> {
    id.map(str::trim).filter(|id| !id.is_empty()).map(str::to_string)
}

/// 勾选判定的判定表：路径 → 是否排除。
///
/// - `base`: 文件树的根。
/// - `picks`: 前端的勾选与取消勾选。
///
/// # 返回值
///
/// 判定表；勾选的条目解析失败时报错，取消勾选的条目可能已被删掉，忽略即可。
fn decisions_of(base: &Path, picks: &[&ExportPickDto]) -> Result<HashMap<PathBuf, bool>, String> {
    let mut decisions = HashMap::new();
    for pick in picks {
        match archive::selected_path(base, &pick.path) {
            Ok(path) => { decisions.insert(path, pick.exclude); }
            Err(_) if pick.exclude => {}
            Err(e) => return Err(e),
        }
    }
    Ok(decisions)
}

/// 路径本身或最近的上级被取消勾选时算排除；越深的判定优先，排除的目录里还能再勾回一条。
///
/// - `decisions`: 判定表，见 `decisions_of`。
/// - `path`: 待判定的绝对路径。
///
/// # 返回值
///
/// 是否排除。
fn is_excluded(decisions: &HashMap<PathBuf, bool>, path: &Path) -> bool {
    for item in path.ancestors() {
        if let Some(exclude) = decisions.get(item) { return *exclude; }
    }
    false
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
        let picks = [ExportPickDto { path: "mod.jar".into(), url: "https://example.invalid/mod.jar".into(), mod_id: None, file_id: None, exclude: false }];
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
            .into_iter().map(|path| ExportPickDto { path: path.into(), url: "".into(), mod_id: None, file_id: None, exclude: false }).collect();
        let result = collect(&dir, picks.iter(), &HashMap::new(), ExportPackType::HMCL).unwrap();
        assert_eq!(result.local, [dir.join("config/nested/settings.toml")]);
        crate::test_support::cleanup(&[&dir]);
    }

    #[test]
    fn excluded_entries_come_out_of_checked_directories() {
        let dir = crate::test_support::temp_dir("export-exclude");
        std::fs::create_dir_all(dir.join(".minecraft/mods")).unwrap();
        std::fs::create_dir_all(dir.join(".minecraft/logs")).unwrap();
        std::fs::write(dir.join(".minecraft/mods/a.jar"), b"a").unwrap();
        std::fs::write(dir.join(".minecraft/logs/x.log"), b"x").unwrap();
        std::fs::write(dir.join(".minecraft/logs/keep.log"), b"k").unwrap();
        let picks = [
            ExportPickDto { path: ".minecraft".into(), url: "".into(), mod_id: None, file_id: None, exclude: false },
            ExportPickDto { path: ".minecraft/logs".into(), url: "".into(), mod_id: None, file_id: None, exclude: true },
            ExportPickDto { path: ".minecraft/logs/keep.log".into(), url: "".into(), mod_id: None, file_id: None, exclude: false },
            // 取消勾选的目录在勾完之后被删掉时，不该让整次导出失败
            ExportPickDto { path: ".minecraft/gone".into(), url: "".into(), mod_id: None, file_id: None, exclude: true },
        ];
        let result = collect(&dir, picks.iter(), &HashMap::new(), ExportPackType::MML).unwrap();
        assert_eq!(result.local, [
            dir.join(".minecraft/logs/keep.log"),
            dir.join(".minecraft/mods/a.jar"),
        ]);
        crate::test_support::cleanup(&[&dir]);
    }

    #[test]
    fn edited_project_and_file_ids_override_the_instance_info() {
        let dir = crate::test_support::temp_dir("export-edited-ids");
        std::fs::write(dir.join("mod.jar"), b"mod").unwrap();
        let online = HashMap::from([("mod.jar".into(), OnlineInfoObj {
            modid: "111".into(), fileid: "222".into(), ..Default::default()
        })]);
        // 改成合法编号：清单用改后的
        let picks = [ExportPickDto {
            path: "mod.jar".into(), url: "".into(),
            mod_id: Some("333".into()), file_id: Some("444".into()), exclude: false,
        }];
        let result = collect(&dir, picks.iter(), &online, ExportPackType::CurseForge).unwrap();
        assert_eq!(result.mods.len(), 1);
        let info = result.mods[0].info.as_ref().unwrap();
        assert_eq!((info.modid.as_str(), info.fileid.as_str()), ("333", "444"));
        // 编号非法：这个格式拼不出清单项，退回 overrides
        let picks = [ExportPickDto {
            path: "mod.jar".into(), url: "".into(),
            mod_id: Some("abc".into()), file_id: Some("444".into()), exclude: false,
        }];
        let result = collect(&dir, picks.iter(), &online, ExportPackType::CurseForge).unwrap();
        assert!(result.mods.is_empty());
        assert_eq!(result.local, [dir.join("mod.jar")]);
        crate::test_support::cleanup(&[&dir]);
    }
    #[test]
    fn indexed_file_is_not_duplicated_in_overrides() {
        let dir = crate::test_support::temp_dir("export-index-overlap");
        std::fs::create_dir(dir.join("mods")).unwrap();
        std::fs::write(dir.join("mods/a.jar"), b"a").unwrap();
        std::fs::write(dir.join("mods/b.jar"), b"b").unwrap();
        let picks: Vec<_> = ["mods", "mods/a.jar", "mods/a.jar"]
            .into_iter().map(|path| ExportPickDto { path: path.into(), url: "https://example.invalid/a".into(), mod_id: None, file_id: None, exclude: false }).collect();
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
        let picks = [ExportPickDto { path: "mod.jar".into(), url: "".into(), mod_id: None, file_id: None, exclude: false }];
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
            let picks = [ExportPickDto { path: "mod.jar".into(), url: url.into(), mod_id: None, file_id: None, exclude: false }];
            let result = collect(&dir, picks.iter(), &online, ExportPackType::Modrinth).unwrap();
            // 只有地址、没有项目号 / 文件号：进 files，清单里的 project 留空
            assert!(result.mods.is_empty());
            assert_eq!(result.files[0].url, if url.is_empty() { "https://example.invalid/saved" } else { url.trim() });
            assert!(result.files[0].info.is_none());
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
