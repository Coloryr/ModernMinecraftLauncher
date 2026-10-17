//! 资源管理窗口的重命名：存档 / 资源包 / 光影 / 结构 / 数据包 / 截图
//!
//! 只改**名字**。分类到目录的映射、纯文件名与上级目录名的防穿越校验，都复用
//! `folder.rs` 那套辅助函数（[`resource_file`] / [`datapack_dir`] / [`check_parent`]），
//! 不在这里另写一份 —— 那些校验是 IPC 入参的唯一防线。

use super::{KIND_DATAPACKS, check_parent, datapack_dir, parse_instance, resource_file};
use crate::err_keys;

/// 重命名一个资源文件或目录
///
/// - `uuid`: 实例 uuid
/// - `kind`: 资源类别，取值 `saves` / `resourcepacks` / `screenshots` /
///   `shaderpacks` / `schematics` / `datapacks`
/// - `parent`: 只有 `datapacks` 要传，即存档目录名；其余类别传空串
/// - `name`: 现在的名字，文件名或目录名
/// - `new_name`: 新名字，只给名字本身，不含任何路径
///
/// # 返回值
///
/// 只表示成功 / 失败：类别未知、名字非法、原目标不存在或新名字已被占用时返回对应错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub fn resource_rename(
    uuid: String,
    kind: String,
    parent: String,
    name: String,
    new_name: String,
) -> Result<(), String> {
    let instance = parse_instance(&uuid)?;
    let new_name = new_name.trim().to_string();
    // 新名字也按"纯名字"挡一次，与旧名字同一口径：带分隔符、上跳或空的名字一律拒绝
    check_parent(&new_name)?;
    if new_name == name {
        return Ok(());
    }

    let (old, new) = if kind == KIND_DATAPACKS {
        let dir = datapack_dir(&instance.read().unwrap(), &parent)?;
        check_parent(&name)?;
        (dir.join(&name), dir.join(&new_name))
    } else {
        (
            resource_file(&instance, &kind, &name)?,
            resource_file(&instance, &kind, &new_name)?,
        )
    };

    if !old.exists() {
        return Err(err_keys::FILE_NOT_FOUND.to_string());
    }
    // 覆盖别人的文件不在"重命名"的语义里：让用户自己先挪开
    if new.exists() {
        return Err(err_keys::NAME_TAKEN.to_string());
    }
    std::fs::rename(&old, &new).map_err(|err| err.to_string())?;
    Ok(())
}
