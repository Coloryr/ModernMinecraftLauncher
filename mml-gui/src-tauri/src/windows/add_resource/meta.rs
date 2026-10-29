//! 添加资源窗口的元数据查询：下载源 / 资源分类 / 排序方式 / 游戏版本
//!
//! 都是"问内核要一份静态列表"的薄命令，游戏版本列表走内核缓存。

use std::collections::HashMap;

use crate::err_keys;
use mml_game::launcher::{FileType, ModPackType};
use mml_game::{curseforge, modrinth};
use mml_net::curseforge_api::CurseForgeSortType;
use mml_net::modrinth_api::ModrinthSortType;

/// 获取下载源，即添加资源窗口支持的资源站
///
/// # 返回值
///
/// 下载源的内部名列表，Modrinth 在前、CurseForge 在后；目前不会失败
#[gui_macros::ipc_group("add_resource")]
#[gui_macros::command]
pub fn add_resource_source_type() -> Result<Vec<String>, String> {
    Ok(vec![
        ModPackType::Modrinth.to_string(),
        ModPackType::CurseForge.to_string(),
    ])
}

/// 获取分组，即资源分类下拉框的候选
///
/// - `source`: 下载源，取 `curseforge` 或 `modrinth`
/// - `file_type`: 资源类型
///
/// # 返回值
///
/// 分类名到显示名的映射；CurseForge 的数据包走固定 categoryId、不支持再按分类过滤，
/// 这里返回空表；类型非法或源不受支持时返回 `Err`
#[gui_macros::ipc_group("add_resource")]
#[gui_macros::command]
pub async fn add_resource_categories(
    source: String,
    file_type: String,
) -> Result<HashMap<String, String>, String> {
    let source = ModPackType::from_string(&source);
    let Some(file_type) = FileType::from_string(&file_type) else {
        return Err(String::from(err_keys::FILE_TYPE_NOT_FOUND));
    };

    match source {
        // CurseForge 的数据包走固定 categoryId，不支持再按分类过滤
        ModPackType::CurseForge if file_type == FileType::DataPacks => Ok(HashMap::new()),
        ModPackType::CurseForge => curseforge::get_categories(file_type)
            .await
            .map_err(|err| err.to_string()),
        ModPackType::Modrinth => modrinth::get_categories(file_type)
            .await
            .map_err(|err| err.to_string()),
        _ => Err(String::from(err_keys::SOURCE_TYPE)),
    }
}

/// 获取排序方式
///
/// - `source`: 下载源，取 `curseforge` 或 `modrinth`
///
/// # 返回值
///
/// 该源支持的排序方式内部名列表；源不受支持时返回 `Err`
#[gui_macros::ipc_group("add_resource")]
#[gui_macros::command]
pub fn add_resource_sort_type(source: String) -> Result<Vec<String>, String> {
    let source = ModPackType::from_string(&source);

    match source {
        ModPackType::CurseForge => Ok(vec![
            CurseForgeSortType::Popularity.to_string(),
            CurseForgeSortType::Featured.to_string(),
            CurseForgeSortType::LastUpdated.to_string(),
            CurseForgeSortType::Name.to_string(),
            CurseForgeSortType::TotalDownloads.to_string(),
        ]),
        ModPackType::Modrinth => Ok(vec![
            ModrinthSortType::Relevance.to_string(),
            ModrinthSortType::Downloads.to_string(),
            ModrinthSortType::Follows.to_string(),
            ModrinthSortType::Newest.to_string(),
            ModrinthSortType::Updated.to_string(),
        ]),
        _ => Err(String::from(err_keys::SOURCE_TYPE)),
    }
}

/// 获取支持的游戏版本列表
///
/// - `source`: 下载源，取 `curseforge` 或 `modrinth`
///
/// # 返回值
///
/// 该源支持的游戏版本字符串列表，走内核缓存；源不受支持时返回 `Err`
#[gui_macros::ipc_group("add_resource")]
#[gui_macros::command]
pub async fn add_resource_game_versions(source: String) -> Result<Vec<String>, String> {
    let source = ModPackType::from_string(&source);

    match source {
        ModPackType::CurseForge => curseforge::get_game_versions()
            .await
            .map_err(|err| err.to_string()),
        ModPackType::Modrinth => modrinth::get_game_versions()
            .await
            .map_err(|err| err.to_string()),
        _ => Err(err_keys::SOURCE_TYPE.to_string()),
    }
}
