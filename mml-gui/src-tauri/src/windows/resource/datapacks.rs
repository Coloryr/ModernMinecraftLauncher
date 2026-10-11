//! 资源管理窗口的数据包分类，也就是存档子页：列表 / 启用禁用 / 删除
//!
//! 数据包挂在**某个存档**下的 `saves/<dir>/datapacks`，所以每条命令都要先按目录名
//! 定位存档，走 `find_save`。

use crate::dtos::DataPackItemDto;
use mml_sys::path_helper;

use super::{find_save, parse_instance};

// ==================== 数据包，存档子页 ====================

/// 存档的数据包列表，get_datapacks 同步 + rayon 解 zip，放阻塞线程
///
/// - `uuid`: 实例 uuid
/// - `dir`: 存档目录名，即 saves 下的一级目录
///
/// # 返回值
///
/// 该存档的数据包列表；实例 / 存档找不到或读取失败返回错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_list_datapacks(
    uuid: String,
    dir: String,
) -> Result<Vec<DataPackItemDto>, String> {
    let instance = parse_instance(&uuid)?;
    let save = find_save(instance, &dir).await?;

    let packs = tauri::async_runtime::spawn_blocking(move || {
        save.get_datapacks().map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())??;

    Ok(packs
        .iter()
        .map(|item| DataPackItemDto {
            file: item
                .path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            name: item.name.clone(),
            description: item.description.clone(),
            pack_format: item.pack_format,
            enable: item.enable,
        })
        .collect())
}

/// 切换数据包启用状态；change_data_pack 对传入的包做状态翻转，传单个即 toggle
///
/// - `uuid`: 实例 uuid
/// - `dir`: 存档目录名，即 saves 下的一级目录
/// - `name`: 数据包名，即 `pack.mcmeta` 里的 `name`，忽略大小写匹配
///
/// # 返回值
///
/// 只表示成功 / 失败：找不到同名包返回 `err.fileNotFound`，其余是内核错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_datapack_toggle(
    uuid: String,
    dir: String,
    name: String,
) -> Result<(), String> {
    let instance = parse_instance(&uuid)?;
    let save = find_save(instance, &dir).await?;

    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let mut save = save;
        let packs = save.get_datapacks().map_err(|err| err.to_string())?;
        let pack = packs
            .into_iter()
            .find(|item| item.name.eq_ignore_ascii_case(&name))
            .ok_or_else(|| "err.fileNotFound".to_string())?;
        save.change_data_pack(&vec![pack])
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

/// 删除数据包：清 level.dat 引用后把文件 / 目录一并进回收站
///
/// - `uuid`: 实例 uuid
/// - `dir`: 存档目录名，即 saves 下的一级目录
/// - `name`: 数据包名，即 `pack.mcmeta` 里的 `name`，忽略大小写匹配
///
/// # 返回值
///
/// 只表示成功 / 失败：找不到同名包返回 `err.fileNotFound`，其余是内核错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_datapack_delete(
    uuid: String,
    dir: String,
    name: String,
) -> Result<(), String> {
    let instance = parse_instance(&uuid)?;
    let save = find_save(instance, &dir).await?;

    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let mut save = save;
        let packs = save.get_datapacks().map_err(|err| err.to_string())?;
        let pack = packs
            .into_iter()
            .find(|item| item.name.eq_ignore_ascii_case(&name))
            .ok_or_else(|| "err.fileNotFound".to_string())?;
        let path = pack.path.clone();
        save.delete_datapack(&vec![pack])
            .map_err(|err| err.to_string())?;
        path_helper::move_to_trash(&path).map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}
