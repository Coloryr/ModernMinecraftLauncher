//! 资源管理窗口的光影包分类：列表 / 选择 / 删除
//!
//! 列表显示名取自光影包内的语言文件；"当前选中"记在 `InstanceSettingObj`。

use std::path::Path;

use crate::dtos::ShaderItemDto;
use crate::err_keys;
use mml_sys::path_helper;

use super::{KIND_SHADERPACKS, block_on_instance, parse_instance, resource_file};

// ==================== 光影包 ====================

/// 光影包列表：解析 zip 内语言文件，并按 options.txt 的 shaderPack 标出启用中的包
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 光影包列表，`selected` 标记当前启用中的那个；实例不存在或解析失败返回错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_list_shaderpacks(uuid: String) -> Result<Vec<ShaderItemDto>, String> {
    // 读的过程中删实例会被拦住，见 resource::instance_busy
    let _op = super::model::begin_if_open(&uuid);
    let instance = parse_instance(&uuid)?;

    block_on_instance(instance, |game| {
        let list =
            tokio::runtime::Handle::current().block_on(async { game.get_shaderpacks().await });
        let selected = game
            .get_minecraft_options()
            .ok()
            .and_then(|opts| opts.get("shaderPack").cloned())
            .unwrap_or_default();
        (list, selected)
    })
    .await
    .map(|(list, selected)| {
        list.iter()
            .filter_map(|item| {
                let file = item.file.file_name()?.to_string_lossy().to_string();
                if file.is_empty() {
                    return None;
                }
                Some(ShaderItemDto {
                    selected: selected == file,
                    name: if item.name.is_empty() {
                        file.clone()
                    } else {
                        item.name.clone()
                    },
                    file,
                    comment: item.comment.clone(),
                })
            })
            .collect()
    })
}

/// 启用 / 停用光影包：写 options.txt 的 shaderPack 键，None = OFF 停用
///
/// - `uuid`: 实例 uuid
/// - `file`: 光影包文件名；`None` 表示写 `OFF` 停用，有值时必须是纯文件名
///
/// # 返回值
///
/// 只表示成功 / 失败：文件名非法返回 `err.fileName`，其余是读写 options.txt 的错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_shader_set(uuid: String, file: Option<String>) -> Result<(), String> {
    if let Some(file) = file.as_ref() {
        let path = Path::new(file);
        if file.is_empty() || path.file_name() != Some(path.as_os_str()) {
            return Err(err_keys::FILE_NAME.to_string());
        }
    }
    let instance = parse_instance(&uuid)?;

    block_on_instance(instance, move |game| {
        let mut opts = game
            .get_minecraft_options()
            .map_err(|err| err.to_string())?;
        opts.insert(
            "shaderPack".to_string(),
            file.unwrap_or_else(|| "OFF".to_string()),
        );
        game.save_minecraft_options(&opts)
            .map_err(|err| err.to_string())
    })
    .await?
}

/// 删除光影包，文件进回收站
///
/// - `uuid`: 实例 uuid
/// - `file`: 光影包文件名，不是完整路径
///
/// # 返回值
///
/// 只表示成功 / 失败：文件名非法返回 `err.fileName`，其余是删除失败的错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_delete_shaderpack(uuid: String, file: String) -> Result<(), String> {
    let instance = parse_instance(&uuid)?;
    let file = resource_file(&instance, KIND_SHADERPACKS, &file)?;
    path_helper::move_to_trash(&file).map_err(|err| err.to_string())
}
