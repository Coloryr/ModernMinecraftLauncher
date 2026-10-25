//! 本地已装的整合包
//!
//! 从实例目录里的整合包文件反查项目编号，供列表标出「已安装」；
//! 只读，目录不存在时按空列表处理。

use std::collections::HashSet;

use crate::windows::modpack_task::DOWNLOAD_NOW;

use super::search::PackFileId;

/// 已安装过的整合包 pid 集合，整页列表共用一次构建
///
/// 逐项调用会让每个列表项各自遍历一遍实例表并加一次锁；一页 20 项就是 20 遍。
///
/// # 返回值
///
/// 已安装整合包的项目 ID 集合；非整合包实例不计入
pub(crate) fn installed_modpack_pids() -> HashSet<String> {
    mml_game::get_instances()
        .iter()
        .filter_map(|item| {
            let temp = item.read().unwrap();
            if temp.is_modpack {
                temp.pid.clone()
            } else {
                None
            }
        })
        .collect()
}

/// 已安装过的整合包 pid + fid 组合集合，版本列表共用
///
/// # 返回值
///
/// 已安装整合包的项目 ID 与文件 ID 组合；缺 pid 或 fid 的实例跳过
pub(crate) fn installed_modpack_files() -> HashSet<PackFileId> {
    mml_game::get_instances()
        .iter()
        .filter_map(|item| {
            let temp = item.read().unwrap();
            if !temp.is_modpack {
                return None;
            }
            match (temp.pid.clone(), temp.fid.clone()) {
                (Some(pid), Some(fid)) => Some(PackFileId { pid, fid }),
                _ => None,
            }
        })
        .collect()
}

/// 正在安装的 pid 集合，供列表角标显示该项目任一版本在装
///
/// # 返回值
///
/// 有安装任务在跑的项目 ID 集合
pub(crate) fn running_pids() -> HashSet<String> {
    DOWNLOAD_NOW
        .read()
        .unwrap()
        .keys()
        .map(|key| key.pid.clone())
        .collect()
}

/// 正在安装的 pid + fid 组合集合，供版本列表角标
///
/// # 返回值
///
/// 有安装任务在跑的项目 ID 与文件 ID 组合
pub(crate) fn running_files() -> HashSet<PackFileId> {
    DOWNLOAD_NOW
        .read()
        .unwrap()
        .keys()
        .map(|key| PackFileId {
            pid: key.pid.clone(),
            fid: key.fid.clone(),
        })
        .collect()
}
