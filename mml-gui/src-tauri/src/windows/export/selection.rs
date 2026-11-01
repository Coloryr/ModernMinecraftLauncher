//! 按目标格式将所选文件分配到下载清单或本地覆盖文件。

use super::{archive, list::export_meta};
use crate::dtos::export_dto::ExportPickDto;
use mml_base::file_item::FileHash;
use mml_game::{
    game_export::{ExportPackType, OnlineFileExport},
    launcher::file_online_info_obj::OnlineInfoObj,
};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

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
#[cfg(test)]
pub(super) fn collect<'a>(
    base: &Path,
    picks: impl Iterator<Item = &'a ExportPickDto>,
    online: &HashMap<String, OnlineInfoObj>,
    pack: ExportPackType,
) -> Result<Selection, String> {
    collect_reporting(base, picks, online, pack, &|_| {})
}

/// 同 [`collect`]，每开始处理一条勾选回报一次进度
///
/// 模组要算哈希，几百个文件读下来要一会儿，界面上得看得见在动。
///
/// - `progress`: 处理到第几条，从 0 开始
pub(super) fn collect_reporting<'a>(
    base: &Path,
    picks: impl Iterator<Item = &'a ExportPickDto>,
    online: &HashMap<String, OnlineInfoObj>,
    pack: ExportPackType,
    progress: &dyn Fn(usize),
) -> Result<Selection, String> {
    let picks: Vec<&ExportPickDto> = picks.collect();
    let decisions = decisions_of(base, &picks)?;
    let mut result = Selection::default();
    let mut explicit = HashSet::new();
    // MML（直接压缩）也认勾选：勾了什么就只压什么，见 native_instance_selected
    for (index, pick) in picks.into_iter().enumerate() {
        progress(index);
        if pick.exclude {
            continue;
        }
        let path = archive::selected_path(base, &pick.path)?;
        // 同一个路径既勾又取消时以判定表里最后一条为准；但模组页明确填了编号 / 地址的
        // 条目例外：文件树那边的排除只表示"不按文件打包"，不能让它连清单一起落空
        let explicit_source = !pick.url.trim().is_empty()
            || pick
                .mod_id
                .as_deref()
                .is_some_and(|id| !id.trim().is_empty())
            || pick
                .file_id
                .as_deref()
                .is_some_and(|id| !id.trim().is_empty());
        if !explicit_source && is_excluded(&decisions, &path) {
            continue;
        }
        if path.is_dir() {
            let files = archive::files_under(&path)?;
            result.local.extend(
                files
                    .into_iter()
                    .filter(|file| !is_excluded(&decisions, file)),
            );
            continue;
        }
        if !explicit.insert(path.clone()) {
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
        } else {
            pick.url.trim()
        };
        // 项目号 / 文件号可以在模组页改：填了以填的为准，留空才用实例里的在线信息
        let modid = pick_id(pick.mod_id.as_deref())
            .or_else(|| source.map(|info| info.modid.clone()))
            .unwrap_or_default();
        let fileid = pick_id(pick.file_id.as_deref())
            .or_else(|| source.map(|info| info.fileid.clone()))
            .unwrap_or_default();
        let indexed = !modid.is_empty() && !fileid.is_empty();
        let can_index = match pack {
            ExportPackType::CurseForge => {
                indexed
                    && [&modid, &fileid]
                        .iter()
                        .all(|id| id.parse::<u64>().is_ok_and(|n| n > 0))
            }
            ExportPackType::Modrinth => !url.is_empty(),
            _ => false,
        };
        if can_index {
            // CurseForge 的清单只写项目号 / 文件号，不用读内容：省掉整份 SHA1 + SHA512
            let (size, hash) = if pack == ExportPackType::CurseForge {
                let size = std::fs::metadata(&path)
                    .map(|meta| meta.len() as usize)
                    .unwrap_or(0);
                (size, FileHash::Sha1Sha512(String::new(), String::new()))
            } else {
                let meta = export_meta(&path)?;
                (meta.size, meta.hash)
            };
            // 有项目号 / 文件号才进下载清单，否则按 overrides 整包带着
            let info = indexed.then(|| OnlineInfoObj {
                file: name.to_string(),
                modid: modid.clone(),
                fileid: fileid.clone(),
                url: url.to_string(),
                ..Default::default()
            });
            let item = OnlineFileExport {
                file: path,
                size,
                pack,
                url: url.to_string(),
                hash,
                info,
            };
            if indexed {
                result.mods.push(item);
            } else {
                result.files.push(item);
            }
        } else {
            result.local.push(path);
        }
    }
    // 文件既能在模组页选中，也能从目录展开；下载项优先，每个路径只导出一次。
    let indexed: HashSet<_> = result
        .mods
        .iter()
        .chain(&result.files)
        .map(|item| &item.file)
        .collect();
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
    id.map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
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
            Ok(path) => {
                decisions.insert(path, pick.exclude);
            }
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
        if let Some(exclude) = decisions.get(item) {
            return *exclude;
        }
    }
    false
}

#[cfg(test)]
#[path = "../../../tests/unit/windows_export_selection_tests.rs"]
mod tests;
