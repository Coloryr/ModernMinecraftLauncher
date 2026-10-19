//! 依赖树与「相关的模组」两个命令
//!
//! 都读窗口 model 里的扫描结果，不重新扫盘。

use std::collections::{HashSet};

use mml_game::game_mods::{ModObj};
use mml_game::loader::LoaderType;

use crate::dtos::{ModDepTreeDto};
use crate::err_keys;

use crate::windows::resource::model::window_model;
use crate::windows::resource::mods::{mod_dependencies};
use crate::windows::resource::{block_on_instance, parse_instance};
use super::draft::{modid_of};
use super::build::{build_tree};


/// 要和这几个模组一起改的那些条目
///
/// - `disable = true`：还有哪些**启用**的模组依赖它们 —— 少了前置轻则功能缺失、重则起不来
/// - `disable = false`：它们要求的强制前置里，现在被禁用着的那些
///
/// - `list`: 窗口模型里那份扫描结果
/// - `loader`: 实例的加载器
/// - `picked`: 这次要改的模组 uuid
/// - `disable`: true = 这次是禁用
///
/// # 返回值
///
/// 要一起改的条目 uuid，按列表顺序；不含 `picked` 自己
pub(super) fn related_mods(
    list: &[ModObj],
    loader: LoaderType,
    picked: &[String],
    disable: bool,
) -> Vec<String> {
    let picked_set: HashSet<String> = picked.iter().cloned().collect();
    // 这次动的这些模组自己的 modid（小写）：禁用时按它找依赖方
    let mut own_ids: HashSet<String> = HashSet::new();
    for item in list {
        if !picked_set.contains(&item.uuid.to_string()) {
            continue;
        }
        for info in item.info.iter() {
            if !info.mod_id.is_empty() {
                own_ids.insert(info.mod_id.to_lowercase());
            }
        }
    }

    if disable {
        return list
            .iter()
            .filter(|item| !item.disable && !item.fail)
            .filter(|item| !picked_set.contains(&item.uuid.to_string()))
            .filter(|item| {
                mod_dependencies(item, loader)
                    .iter()
                    .any(|dep| !dep.optional && own_ids.contains(&dep.mod_id.to_lowercase()))
            })
            .map(|item| item.uuid.to_string())
            .collect();
    }

    // 启用：把它们要求的强制前置收集起来，再挑出现在被禁用着的那些
    let mut wanted: HashSet<String> = HashSet::new();
    for item in list {
        if !picked_set.contains(&item.uuid.to_string()) {
            continue;
        }
        for dep in mod_dependencies(item, loader) {
            if !dep.optional {
                wanted.insert(dep.mod_id.to_lowercase());
            }
        }
    }
    if wanted.is_empty() {
        return Vec::new();
    }
    list.iter()
        .filter(|item| item.disable && !picked_set.contains(&item.uuid.to_string()))
        .filter(|item| wanted.contains(&modid_of(item).to_lowercase()))
        .map(|item| item.uuid.to_string())
        .collect()
}

/// 模组依赖图：从窗口模型那份扫描结果建图
///
/// **不重扫 mods 目录**，节点与连线都由后端算好，前端只负责布局与画。
///
/// - `uuid`: 实例 uuid
/// - `mod_uuid`: 图根那个模组的 uuid，由前端从列表项里取
///
/// # 返回值
///
/// 见 [`ModDepTreeDto`]；实例不存在、模组列表还没加载、或列表里没有这个模组时返回错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_mods_dep_tree(
    uuid: String,
    mod_uuid: String,
) -> Result<ModDepTreeDto, String> {
    let model = window_model(&uuid)?;
    let instance = parse_instance(&uuid)?;

    block_on_instance(instance, move |game| {
        model
            .with_cache(|cache| build_tree(&game, &cache.list, &mod_uuid))
            .ok_or_else(|| err_keys::MODS_NOT_LOADED.to_string())?
    })
    .await?
}

/// 要和这几个模组一起改的条目：给"一起禁用 / 一起启用"那个确认框用
///
/// 关系在后端算（见 [`related_mods`]），前端拿到 uuid 去自己的列表里对。
///
/// - `uuid`: 实例 uuid
/// - `mods`: 这次要改的模组 uuid
/// - `disable`: true = 这次是禁用，false = 启用
///
/// # 返回值
///
/// 要一起改的条目 uuid，按列表顺序；实例不存在或模组列表还没加载时返回错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_mods_related_mods(
    uuid: String,
    mods: Vec<String>,
    disable: bool,
) -> Result<Vec<String>, String> {
    let model = window_model(&uuid)?;
    let instance = parse_instance(&uuid)?;

    block_on_instance(instance, move |game| {
        model
            .with_cache(|cache| related_mods(&cache.list, game.loader, &mods, disable))
            .ok_or_else(|| err_keys::MODS_NOT_LOADED.to_string())
    })
    .await?
}
