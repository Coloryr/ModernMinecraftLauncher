//! 资源管理窗口的截图分类：列表 / 删除 / 清空
//!
//! 从 `resource/mod.rs` 拆出来的**试点**：命令搬进子模块后，靠一条分组标记属性把组键钉回
//! `resource`，标记值填原始组名 resource，前端 `commands.resource.*` 一个调用点都不用改；
//! 生成器侧见 ipc-gen/src/scan.rs 的 `group_of`。

use mml_sys::path_helper;

use crate::dtos::ScreenshotItemDto;

use super::{KIND_SCREENSHOTS, parse_instance, resource_file};

// ==================== 截图 ====================

/// 截图列表
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 截图文件名列表；实例不存在返回 `err.gameNotFound`
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub fn resource_list_screenshots(uuid: String) -> Result<Vec<ScreenshotItemDto>, String> {
    // 读的过程中删实例会被拦住，见 resource::instance_busy
    let _op = super::model::begin_if_open(&uuid);
    let instance = parse_instance(&uuid)?;
    let list = instance.read().unwrap().get_screenshots();

    Ok(list
        .iter()
        .map(|item| ScreenshotItemDto {
            name: item.name.clone(),
        })
        .collect())
}

/// 删除截图，文件进回收站
///
/// - `uuid`: 实例 uuid
/// - `name`: 截图文件名，不是完整路径
///
/// # 返回值
///
/// 只表示成功 / 失败：文件名非法返回 `err.fileName`，其余是删除失败的错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_delete_screenshot(uuid: String, name: String) -> Result<(), String> {
    let instance = parse_instance(&uuid)?;
    let file = resource_file(&instance, KIND_SCREENSHOTS, &name)?;
    path_helper::move_to_trash(&file).map_err(|err| err.to_string())
}

/// 清空全部截图，文件进回收站，文件多时耗时
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 只表示成功 / 失败：实例不存在或删除失败返回错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_clear_screenshots(uuid: String) -> Result<(), String> {
    let instance = parse_instance(&uuid)?;
    tauri::async_runtime::spawn_blocking(move || {
        instance
            .read()
            .unwrap()
            .clear_screenshots()
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}
