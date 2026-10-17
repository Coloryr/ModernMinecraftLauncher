//! 资源管理窗口的服务器分类：列表 / 添加 / 修改 / 删除
//!
//! 读写 `servers.dat`，即 NBT 格式，由内核 `InstanceSettingObj` 的 server 方法承担。

use std::path::Path;

use crate::dtos::ServerItemDto;
use crate::err_keys;

use super::parse_instance;

// ==================== 服务器 ====================

/// 服务器列表：读 servers.dat，同步纯读
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 服务器条目，图标由 servers.dat 里存的 base64 拼成 data URL；实例不存在或读取失败返回错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub fn resource_list_servers(uuid: String) -> Result<Vec<ServerItemDto>, String> {
    // 读的过程中删实例会被拦住，见 resource::instance_busy
    let _op = super::model::begin_if_open(&uuid);
    let instance = parse_instance(&uuid)?;
    let list = instance
        .read()
        .unwrap()
        .get_server_infos()
        .map_err(|err| err.to_string())?;

    Ok(list
        .iter()
        .map(|item| ServerItemDto {
            name: item.name.clone(),
            ip: item.ip.clone(),
            accept_textures: item.accept_textures,
            // servers.dat 里存的就是 base64 字符串，直接拼 data URL
            icon: match item.icon.as_deref() {
                Some(icon) if !icon.is_empty() => format!("data:image/png;base64,{icon}"),
                _ => String::new(),
            },
        })
        .collect())
}

/// 添加服务器
///
/// - `uuid`: 实例 uuid
/// - `name`: 服务器显示名，空白会被拒绝
/// - `ip`: 服务器地址，空白会被拒绝
///
/// # 返回值
///
/// 只表示成功 / 失败：名字或地址为空返回 `err.nameIp`，其余是写入失败的错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub fn resource_server_add(uuid: String, name: String, ip: String) -> Result<(), String> {
    if name.trim().is_empty() || ip.trim().is_empty() {
        return Err(err_keys::NAME_IP.to_string());
    }
    let instance = parse_instance(&uuid)?;
    instance
        .read()
        .unwrap()
        .add_server(&name, &ip)
        .map_err(|err| err.to_string())
}

/// 编辑服务器：按原 name + ip 定位，替换名字 / 地址 / 资源包接受开关
///
/// - `uuid`: 实例 uuid
/// - `name`: 原服务器显示名，与 `ip` 一起定位
/// - `ip`: 原服务器地址，与 `name` 一起定位
/// - `new_name`: 新显示名，空白会被拒绝
/// - `new_ip`: 新地址，空白会被拒绝
/// - `accept_textures`: 是否接受服务器资源包
///
/// # 返回值
///
/// 只表示成功 / 失败：新名字 / 新地址为空返回 `err.nameIp`，
/// 没找到原条目返回 `err.fileNotFound`，其余是写入失败的错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub fn resource_server_update(
    uuid: String,
    name: String,
    ip: String,
    new_name: String,
    new_ip: String,
    accept_textures: bool,
) -> Result<(), String> {
    if new_name.trim().is_empty() || new_ip.trim().is_empty() {
        return Err(err_keys::NAME_IP.to_string());
    }
    let instance = parse_instance(&uuid)?;
    let game = instance.read().unwrap();
    let mut list = game.get_server_infos().map_err(|err| err.to_string())?;

    let mut found = false;
    for item in list.iter_mut() {
        if item.name == name && item.ip == ip {
            item.name = new_name;
            item.ip = new_ip;
            item.accept_textures = accept_textures;
            found = true;
            break;
        }
    }
    if !found {
        return Err(err_keys::FILE_NOT_FOUND.to_string());
    }

    game.save_servers(&list).map_err(|err| err.to_string())
}

/// 删除服务器
///
/// - `uuid`: 实例 uuid
/// - `name`: 服务器显示名，与 `ip` 一起定位
/// - `ip`: 服务器地址，与 `name` 一起定位
///
/// # 返回值
///
/// 只表示成功 / 失败：实例不存在或删除失败返回错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub fn resource_server_delete(uuid: String, name: String, ip: String) -> Result<(), String> {
    let instance = parse_instance(&uuid)?;
    instance
        .read()
        .unwrap()
        .remove_server(&name, &ip)
        .map_err(|err| err.to_string())
}

/// 导出服务器列表：把实例的 `servers.dat` 复制到指定位置
///
/// - `uuid`: 实例 uuid
/// - `path`: 导出到哪个文件，由前端文件保存框给
///
/// # 返回值
///
/// 只表示成功 / 失败：源文件不存在或复制失败返回错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub fn resource_servers_export(uuid: String, path: String) -> Result<(), String> {
    let instance = parse_instance(&uuid)?;
    let source = instance.read().unwrap().get_servers_file();
    if !source.exists() {
        return Err(err_keys::FILE_NOT_FOUND.to_string());
    }
    std::fs::copy(&source, &path).map_err(|err| err.to_string())?;
    Ok(())
}

/// 导入服务器列表：用选中的 `servers.dat` 替换实例里那份
///
/// **先备份再替换，解析不通过就还原**：导入的是外部文件，格式对不对只有解析一次才知道，
/// 直接把用户原来的列表覆盖掉不太合适。备份落在同目录的 `servers.dat.bak`。
///
/// - `uuid`: 实例 uuid
/// - `path`: 要导入的文件，由前端文件选择框给
///
/// # 返回值
///
/// 只表示成功 / 失败：源文件不存在、复制失败或解析不通过（已还原）返回错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub fn resource_servers_import(uuid: String, path: String) -> Result<(), String> {
    let instance = parse_instance(&uuid)?;
    let target = instance.read().unwrap().get_servers_file();
    let source = Path::new(&path);
    if !source.is_file() {
        return Err(err_keys::FILE_NOT_FOUND.to_string());
    }

    // 备份旧文件；原来没有就不用备份，还原时删掉新文件即可
    let backup = target.with_extension("dat.bak");
    let had_old = target.exists();
    if had_old {
        std::fs::copy(&target, &backup).map_err(|err| err.to_string())?;
    }

    std::fs::copy(source, &target).map_err(|err| err.to_string())?;
    // 解析一次，确认这份文件真的是服务器列表
    if let Err(err) = instance.read().unwrap().get_server_infos() {
        if had_old {
            let _ = std::fs::copy(&backup, &target);
        } else {
            let _ = std::fs::remove_file(&target);
        }
        return Err(err.to_string());
    }
    Ok(())
}
