//! 资源管理窗口的「打开文件夹」：按类别打开实例资源目录，或选中其中的某个文件

use mml_sys::{open_helper, path_helper};

use super::{KIND_DATAPACKS, datapack_dir, instance_dir, parse_instance, resource_file};

// ==================== 打开文件夹 ====================

/// 打开资源目录：资源管理器定位到目录本身，或其中的某个文件
///
/// - `uuid`: 实例 uuid
/// - `kind`: 资源类别，取值 `mods` / `resourcepacks` / `saves` / `screenshots` /
///   `shaderpacks` / `schematics` / `servers` / `datapacks`
/// - `name`: 要定位的文件名；`None` 或空白表示打开目录本身，目录不存在则先创建
/// - `parent`: 只有 `datapacks` 类别要传，即存档目录名，落点 `saves/<parent>/datapacks`
///
/// # 返回值
///
/// 只表示成功 / 失败：目标不存在返回 `err.fileNotFound`，
/// 类别未知 / 名字不是纯文件名返回对应错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub fn resource_open_folder(
    uuid: String,
    kind: String,
    name: Option<String>,
    parent: Option<String>,
) -> Result<(), String> {
    let instance = parse_instance(&uuid)?;
    let dir = if kind == KIND_DATAPACKS {
        // parent 的合法性，即纯目录名，在 datapack_dir 里挡一次
        let parent = parent.unwrap_or_default();
        datapack_dir(&instance.read().unwrap(), &parent)?
    } else {
        instance_dir(&instance, &kind)?
    };

    let target = match name.as_deref() {
        Some(name) if !name.trim().is_empty() => resource_file(&instance, &kind, name)?,
        _ => {
            if !dir.exists() {
                path_helper::create_dir_all(&dir).map_err(|err| err.to_string())?;
            }
            dir
        }
    };
    if !target.exists() {
        return Err("err.fileNotFound".to_string());
    }

    open_helper::open_file_with_explorer(&target);
    Ok(())
}
