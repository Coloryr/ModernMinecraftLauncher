//! 资源管理窗口的结构文件分类：列表 / 删除
//!
//! 结构类型由内核按文件内容识别，取值 litematica / sponge / 原版 nbt。

use crate::dtos::SchematicItemDto;
use mml_game::game_schematics::SchematicType;
use mml_sys::path_helper;

use super::{KIND_SCHEMATICS, block_on_instance, parse_instance, resource_file};

// ==================== 结构文件 ====================

/// 结构类型标签
///
/// - `schematic_type`: 内核识别出来的结构类型
///
/// # 返回值
///
/// 前端按 i18n 显示的稳定枚举名，取值 `Minecraft` / `Litematic` / `WorldEdit` / `Create`
fn schematic_type_name(schematic_type: &SchematicType) -> &'static str {
    match schematic_type {
        SchematicType::Minecraft => "Minecraft",
        SchematicType::Litematic => "Litematic",
        SchematicType::WorldEdit => "WorldEdit",
        SchematicType::Create => "Create",
    }
}

/// 结构文件列表：按扩展名解析 NBT
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 结构文件列表；取不到文件名的条目已被过滤，实例不存在或解析失败返回错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_list_schematics(uuid: String) -> Result<Vec<SchematicItemDto>, String> {
    let instance = parse_instance(&uuid)?;

    let list = block_on_instance(instance, |game| {
        tokio::runtime::Handle::current().block_on(async { game.get_schematics().await })
    })
    .await?;

    Ok(list
        .iter()
        .filter_map(|item| {
            let file = item.path.file_name()?.to_string_lossy().to_string();
            if file.is_empty() {
                return None;
            }
            Some(SchematicItemDto {
                type_name: schematic_type_name(&item.schematic_type).to_string(),
                file,
                name: item.name.clone(),
                author: item.author.clone(),
                description: item.description.clone(),
                width: item.width,
                height: item.height,
                length: item.length,
                block_count: item.block_count,
                block_types: item.block_types,
                fail: item.fail,
            })
        })
        .collect())
}

/// 删除结构文件，文件进回收站
///
/// - `uuid`: 实例 uuid
/// - `file`: 结构文件名，不是完整路径
///
/// # 返回值
///
/// 只表示成功 / 失败：文件名非法返回 `err.fileName`，其余是内核错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_delete_schematic(uuid: String, file: String) -> Result<(), String> {
    let instance = parse_instance(&uuid)?;
    let file = resource_file(&instance, KIND_SCHEMATICS, &file)?;
    path_helper::move_to_trash(&file).map_err(|err| err.to_string())
}
