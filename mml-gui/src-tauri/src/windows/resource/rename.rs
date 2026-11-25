//! 资源管理窗口的重命名：存档 / 资源包 / 光影 / 结构 / 数据包 / 截图
//!
//! 只改**名字**。分类到目录的映射、纯文件名与上级目录名的防穿越校验，都复用
//! `folder.rs` 那套辅助函数（[`resource_file`] / [`datapack_dir`] / [`check_parent`]），
//! 不在这里另写一份 —— 那些校验是 IPC 入参的唯一防线。

use super::{KIND_DATAPACKS, check_parent, datapack_dir, find_save, parse_instance, resource_file};
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
#[gui_macros::command]
pub async fn resource_rename(
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
    // 再按文件名规则挡一次：Windows 非法字符、保留名、结尾的点在这里就报清楚，
    // 别等 fs::rename 抛一句系统错误
    mml_game::check_name(&new_name).map_err(|e| e.to_string())?;
    if new_name == name {
        return Ok(());
    }

    let is_datapacks = kind == KIND_DATAPACKS;
    let save_instance = if is_datapacks { Some(instance.clone()) } else { None };
    let (old, new) = if is_datapacks {
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

    // 数据包的启用状态按 `file/<文件名>` 记在 level.dat 的 Enabled / Disabled 里；
    // 只改磁盘名会让改完的包掉回“未启用”，这里把旧登记名一起换掉。
    if let Some(instance) = save_instance {
        let save = find_save(instance, &parent).await?;
        let old_key = format!("file/{name}");
        let new_key = format!("file/{new_name}");
        tauri::async_runtime::spawn_blocking(move || {
            let mut save = save;
            save.rename_datapack(&old_key, &new_key)
                .map_err(|err| err.to_string())
        })
        .await
        .map_err(|err| err.to_string())??;
    }
    Ok(())
}
