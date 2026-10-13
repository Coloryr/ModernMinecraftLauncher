//! 资源管理窗口的结构文件分类：列表 / 详情 / 删除
//!
//! 结构类型由内核按文件内容识别，取值 litematica / sponge / 原版 nbt。

use crate::dtos::{SchematicBlockDto, SchematicDetailDto, SchematicItemDto};
use mml_game::game_schematics::{SchematicObj, SchematicType, read_schematic_file};
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

/// 结构文件详情：逐种方块的数量都在这儿
///
/// 只读这一个文件，不重扫目录：`read_schematic_file` 是单文件读取器，
/// 而目录扫描要把每个结构都解析一遍，点一次详情不值当。
///
/// - `uuid`: 实例 uuid
/// - `file`: 结构文件名，不是完整路径
///
/// # 返回值
///
/// 见 [`SchematicDetailDto`]；文件名非法返回 `err.fileName`，打开失败返回内核错误串。
/// 解析失败不返回错误，而是 `fail` 为真、其余字段为默认值，与列表条目同一口径
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_schematic_detail(
    uuid: String,
    file: String,
) -> Result<SchematicDetailDto, String> {
    let instance = parse_instance(&uuid)?;
    let path = resource_file(&instance, KIND_SCHEMATICS, &file)?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_default();

    // 解一个结构可能不小：丢到阻塞线程，别占住 async runtime。
    // 解析失败按"读不出来"处理，与列表条目一样标 `fail`，不把内核错误串弹给用户
    let parsed = tauri::async_runtime::spawn_blocking(move || -> Result<Option<SchematicObj>, String> {
        let mut stream = path_helper::open_read(&path).map_err(|err| err.to_string())?;
        Ok(read_schematic_file(&mut stream, SchematicType::from_ext(&ext)).ok())
    })
    .await
    .map_err(|err| err.to_string())??;

    let obj = parsed.unwrap_or_else(|| SchematicObj {
        fail: true,
        ..Default::default()
    });

    // 逐种方块按数量从多到少；同数量的按 ID 排，保证每次打开顺序一致
    let mut blocks: Vec<SchematicBlockDto> = obj
        .blocks
        .iter()
        .map(|(id, count)| SchematicBlockDto {
            id: id.clone(),
            count: *count,
        })
        .collect();
    blocks.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.id.cmp(&b.id)));

    Ok(SchematicDetailDto {
        file: name,
        name: obj.name,
        author: obj.author,
        description: obj.description,
        type_name: schematic_type_name(&obj.schematic_type).to_string(),
        width: obj.width,
        height: obj.height,
        length: obj.length,
        block_count: obj.block_count,
        block_types: obj.block_types,
        blocks,
        fail: obj.fail,
    })
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
