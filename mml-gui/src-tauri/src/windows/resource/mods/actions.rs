//! 模组的启用、禁用、删除与在线信息
//!
//! 每个命令都先按实例加锁，再改磁盘并回写窗口 model。


use mml_game::game_mods::{DependantProblem, DependantProblemKind, ModDuplicate, mods_dep_test};
use mml_game::launcher::file_online_info_obj::OnlineInfoObj;
use mml_names::names;

use crate::dtos::{DepProblemDto, DepProblemKindDto, ModDuplicateDto, ModDuplicateFileDto, ModRenameDto};

use crate::err_keys;

use crate::windows::resource::model::{window_model};
use crate::windows::resource::{block_on_instance, parse_instance};

// ==================== 模组 ====================
use super::item::{mod_note_key, sha1_of, find_mod, mod_rename_dto};


/// 启用模组，去掉 .disable / .disabled 后缀
///
/// - `uuid`: 实例 uuid
/// - `mod_uuid`: 模组 uuid，由文件路径算出
///
/// # 返回值
///
/// **改名后的新身份**，前端拿它就地更新那一行，不重扫整个 mods 目录；
/// 找不到模组返回 `err.fileNotFound`，其余是改名失败的错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_mod_enable(uuid: String, mod_uuid: String) -> Result<ModRenameDto, String> {
    let instance = parse_instance(&uuid)?;
    let obj = find_mod(instance.clone(), &mod_uuid).await?;
    let new_path = obj.enable().map_err(|err| err.to_string())?;
    // 窗口模型里那一条跟着改名：文件名、启用状态、uuid 都变了
    if let Ok(model) = window_model(&uuid) {
        model.rename_mod(&mod_uuid, &new_path, false);
    }
    mod_rename_dto(instance, new_path).await
}

/// 禁用模组，追加 .disable 后缀
///
/// - `uuid`: 实例 uuid
/// - `mod_uuid`: 模组 uuid，由文件路径算出
///
/// # 返回值
///
/// 改名后的新身份，同上；已禁用或文件不存在时返回错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_mod_disable(uuid: String, mod_uuid: String) -> Result<ModRenameDto, String> {
    let instance = parse_instance(&uuid)?;
    let obj = find_mod(instance.clone(), &mod_uuid).await?;
    let new_path = obj.disable().map_err(|err| err.to_string())?;
    // 窗口模型里那一条跟着改名，同 enable
    if let Ok(model) = window_model(&uuid) {
        model.rename_mod(&mod_uuid, &new_path, true);
    }
    mod_rename_dto(instance, new_path).await
}

/// 删除模组，文件进回收站
///
/// - `uuid`: 实例 uuid
/// - `mod_uuid`: 模组 uuid，由文件路径算出
///
/// # 返回值
///
/// 只表示成功 / 失败：找不到模组返回 `err.fileNotFound`，其余是删除失败的错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_delete_mod(uuid: String, mod_uuid: String) -> Result<(), String> {
    let instance = parse_instance(&uuid)?;
    let obj = find_mod(instance, &mod_uuid).await?;
    obj.delete().map_err(|err| err.to_string())?;
    // 窗口模型里那一条也去掉，别让更新检查再去查一个已经不存在的包
    if let Ok(model) = window_model(&uuid) {
        model.remove_mod(&mod_uuid);
    }
    Ok(())
}

/// 模组依赖检测：检查各包声明的强制前置是否齐全、版本是否满足
///
/// **不重扫 mods 目录**：依赖信息只存在于扫描结果里，而窗口模型已经存了一份
/// （见 [`crate::windows::resource::model`]），这里直接拿它算，纯内存、点下去就出结果。
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 逐条问题，见 [`DepProblemDto`]；没有问题时空数组；
/// 实例不存在或模组列表还没加载时返回错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_mods_dep_test(uuid: String) -> Result<Vec<DepProblemDto>, String> {
    let model = window_model(&uuid)?;
    let instance = parse_instance(&uuid)?;

    block_on_instance(instance, move |game| {
        let Some(result) = model.with_cache(|cache| mods_dep_test(&game, &cache.list)) else {
            return Err(err_keys::MODS_NOT_LOADED.to_string());
        };
        let test = result.map_err(|err| err.to_string())?;
        Ok(test.problems.into_iter().map(dep_problem_dto).collect())
    })
    .await?
}

/// 内核的依赖问题 → 跨 IPC 的 DTO
///
/// - `problem`: 内核侧的一条问题
///
/// # 返回值
///
/// 见 [`DepProblemDto`]
pub(super) fn dep_problem_dto(problem: DependantProblem) -> DepProblemDto {
    DepProblemDto {
        modid: problem.modid,
        kind: match problem.kind {
            DependantProblemKind::Missing => DepProblemKindDto::Missing,
            DependantProblemKind::Mismatch => DepProblemKindDto::Mismatch,
            DependantProblemKind::Disabled => DepProblemKindDto::Disabled,
        },
        required: problem.required.to_string(),
        installed: problem.installed,
        required_by: problem.required_by,
    }
}

/// 内核的重复项 → 跨 IPC 的 DTO
///
/// - `item`: 内核侧的一个重复 modid
///
/// # 返回值
///
/// 见 [`ModDuplicateDto`]
pub(super) fn mod_duplicate_dto(item: ModDuplicate) -> ModDuplicateDto {
    ModDuplicateDto {
        modid: item.modid,
        files: item
            .files
            .into_iter()
            .map(|file| ModDuplicateFileDto {
                file: file.file,
                version: file.version,
            })
            .collect(),
    }
}

/// 手动改一条模组的在线信息：下载源 / 项目编号 / 文件编号
///
/// 三样东西落两处：**项目号与文件号**写 `online_info.json`（更新检查按它比对平台），
/// **下载源**写实例的 `gui_setting.json`（`Mod.ModSource`，平时按项目号 / 文件号的形态判定，
/// 两边都有反例，所以留了能手动盖住的地方）。
///
/// - `uuid`: 实例 uuid
/// - `mod_uuid`: 模组的 uuid，即文件路径的 v5
/// - `source`: 下载源，取值 `curseforge` / `modrinth` / 空串（等于不指定）
/// - `project_id`: 项目编号；空串表示这条模组没有在线信息
/// - `file_id`: 文件编号
///
/// # 返回值
///
/// 只表示成功 / 失败：模组不在列表里、下载源取值不认识或写盘失败返回错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_mod_online_set(
    uuid: String,
    mod_uuid: String,
    source: String,
    project_id: String,
    file_id: String,
) -> Result<(), String> {
    let source = source.trim().to_string();
    if !matches!(source.as_str(), "" | "curseforge" | "modrinth") {
        return Err(err_keys::SOURCE_TYPE.to_string());
    }
    let project_id = project_id.trim().to_string();
    let file_id = file_id.trim().to_string();

    let model = crate::windows::resource::model::window_model(&uuid)?;
    // 拿不到这个条目就说不清改的是哪一个：列表还没加载或 uuid 已经过期
    let Some((file, sha1)) = model
        .with_cache(|cache| {
            cache.list.iter().find_map(|item| {
                (item.uuid.to_string() == mod_uuid).then(|| {
                    (
                        item.file
                            .file_name()
                            .map(|name| name.to_string_lossy().to_string())
                            .unwrap_or_default(),
                        sha1_of(&item.hash),
                    )
                })
            })
        })
        .flatten()
    else {
        return Err(err_keys::MODS_NOT_LOADED.to_string());
    };

    let instance = parse_instance(&uuid)?;
    let game = instance.read().unwrap();

    // 1) 在线信息：按 SHA1 删掉旧条目（旧键是旧项目号），再按新项目号写入
    let mut table = game.read_online_info();
    table.retain(|_, item| item.sha1 != sha1);
    let info = (!project_id.is_empty()).then(|| OnlineInfoObj {
        path: names::GAME_MODS_DIR.to_string(),
        // 显示名沿用模组列表里的那份，这里不知道也不需要知道
        name: String::new(),
        file: file.clone(),
        sha1: sha1.clone(),
        url: match source.as_str() {
            "modrinth" => format!("https://modrinth.com/mod/{project_id}"),
            "curseforge" => format!("https://www.curseforge.com/projects/{project_id}"),
            _ => String::new(),
        },
        modid: project_id.clone(),
        fileid: file_id.clone(),
    });
    if let Some(info) = &info {
        table.insert(project_id.clone(), info.clone());
    }
    game.save_online_info(&table);

    // 2) 下载源：进实例的 gui_setting，空串等于把这条覆盖删掉
    let key = mod_note_key(&file).to_string();
    let mut setting = crate::gui_setting::load(&game);
    if source.is_empty() {
        setting.mods.mod_source.remove(&key);
    } else {
        setting.mods.mod_source.insert(key, source);
    }
    crate::gui_setting::save(&game, &setting);
    drop(game);

    // 3) 窗口模型里那份同步换掉：更新检查直接读它，别回磁盘再读一遍
    model.set_online(&sha1, &project_id, info.unwrap_or_default());
    Ok(())
}
