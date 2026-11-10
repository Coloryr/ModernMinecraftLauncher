//! 下载计划
//!
//! 查已有文件、算目标路径，决定这次下载是复用还是新下。
use std::{
    fs,
    path::{Path, PathBuf},
};

use mml_sys::path_helper;
use uuid::Uuid;

use crate::info::{LinkInfoObj, ResourceInfoObj, ResourceType};

use super::*;

/// 记录一份公共文件
///
/// - `resource`: 资源类型
/// - `info`: 文件信息，`hash` 必须是非空 SHA1
/// - `pid`: 项目 ID，同一项目可以有多个版本
pub fn add_download_info(resource: ResourceType, mut info: ResourceInfoObj, pid: String) {
    let Some(hash) = normalize_hash(&info.hash) else {
        return;
    };
    info.pid = pid.clone();
    info.hash = hash;
    if let Some(file) = Path::new(&info.file)
        .file_name()
        .and_then(|file| file.to_str())
    {
        info.file = file.to_string();
    }

    // 单条写进索引库，反查键由库自己维护；旧版本记录留在主索引里，点查不会互相覆盖
    save_entry(resource, &pid, &info);
}

/// 获取实例的映射记录
///
/// - `uuid`: 实例 uuid
/// - `instance_base`: 实例根目录，记录文件就在它下面
///
/// # 返回值
///
/// 该实例记录过的映射；没有记录时为空列表
pub fn get_instance_link_data(uuid: Uuid, instance_base: &Path) -> Vec<LinkInfoObj> {
    if let Some(list) = link_data().read().unwrap().get(&uuid) {
        return list.clone();
    }

    let list = read_link_data(instance_base);
    link_data().write().unwrap().insert(uuid, list.clone());
    list
}

/// 按项目 ID 取一条公共索引记录
///
/// 同一项目可能记着多个版本，取最后写入的那条。
///
/// - resource: 资源类型
/// - pid: 项目 ID
///
/// # 返回值
///
/// 找到时返回索引记录；否则 None
pub fn get_download_info(resource: ResourceType, pid: &str) -> Option<ResourceInfoObj> {
    let store = INDEX.get()?;
    let (_, payload) = match store.latest_of(resource.kind(), pid) {
        Ok(row) => row?,
        Err(err) => {
            log_index_error(err);
            return None;
        }
    };
    serde_json::from_slice(&payload).ok()
}

/// 公共存储中是否已有该 SHA1 的文件
///
/// - `resource`: 资源类型
/// - `hash`: 文件 SHA1，大小写不敏感
///
/// # 返回值
///
/// 找到且校验通过时返回公共文件路径
pub fn find_file(resource: ResourceType, hash: &str) -> Option<PathBuf> {
    let hash = normalize_hash(hash)?;
    if let Some(store) = INDEX.get() {
        match store.find(resource.kind(), &hash) {
            Ok(Some((_, payload))) => {
                if let Ok(item) = serde_json::from_slice::<ResourceInfoObj>(&payload) {
                    let file = resource.download_dir().join(&item.file);
                    // 索引是下载完成时记的，文件在就当作有效；每次启动都对几百个模组重算
                    // SHA1 太慢，坏文件留给实际加载报错
                    if file.is_file() {
                        return Some(file);
                    }
                }
            }
            Ok(None) => {}
            Err(err) => log_index_error(err),
        }
    }

    // 索引丢失或用户手动放进公共目录：按"哈希.扩展名"再扫一遍
    let dir = fs::read_dir(resource.download_dir()).ok()?;
    for entry in dir.flatten() {
        let file = entry.path();
        if !file.is_file() {
            continue;
        }
        let Some(name) = file.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.len() >= hash.len()
            && name
                .get(..hash.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(&hash))
            && hash_matches(&file, &hash)
        {
            return Some(file);
        }
    }

    None
}

/// 公共存储中的下载目标路径
///
/// 文件名沿用原始文件名，同一个文件在多个实例间共用一个路径。
/// 同名文件内容不同时加 SHA1 前 8 位后缀，不覆盖已有文件。
///
/// - `resource`: 资源类型
/// - `hash`: 文件 SHA1
/// - `name`: 原始文件名
///
/// # 返回值
///
/// 哈希非法或目录创建失败时返回 `None`
pub fn download_path(resource: ResourceType, hash: &str, name: &str) -> Option<PathBuf> {
    let hash = normalize_hash(hash)?;
    let file_name = file_name(name).unwrap_or_else(|| hash.clone());
    let base = resource.download_dir().join(file_name);
    if let Some(parent) = base.parent()
        && let Err(err) = path_helper::create_dir_all(parent)
    {
        mml_log::error_type(err);
        return None;
    }

    // 同名文件内容不同：保留原来的文件，给新文件加哈希后缀，不能覆盖别人的模组
    if !base.exists() || hash_matches(&base, &hash) {
        return Some(base);
    }
    Some(unique_file(&base, &hash))
}

/// 为一次新下载准备公共目录路径
///
/// 公共目录已有该文件时直接链接到实例并把目标从下载列表里去掉；没有时返回一份
/// 下载计划，调用方把下载目标改成 plan.source，下载完成后调用 finish_plan。
///
/// - resource: 资源类型
/// - instance: 实例 uuid
/// - instance_base: 实例根目录
/// - target: 实例内目标文件
/// - pid / fid / name / hash / url: 资源元数据
/// - verify_target: 目标已存在时是否核对哈希；启动前检查可传 false，避免每次启动重算全部文件
///
/// # 返回值
///
/// 需要下载时返回计划；公共目录已有并完成链接、或目标已存在且无需处理时返回 None
#[allow(clippy::too_many_arguments)]
pub fn plan_download(
    resource: ResourceType,
    instance: Uuid,
    instance_base: &Path,
    target: &Path,
    pid: &str,
    fid: &str,
    name: &str,
    hash: &str,
    url: &str,
    verify_target: bool,
) -> Option<LinkPlan> {
    if !is_enabled() {
        return None;
    }
    let hash = normalize_hash(hash)?;

    if let Some(source) = find_file(resource, &hash) {
        // 索引是下载完成时记的；文件真被改坏时删掉重下，别把坏文件硬链接出去
        if !hash_matches(&source, &hash) {
            let _ = path_helper::delete(&source);
            return Some(LinkPlan {
                resource,
                instance,
                base: instance_base.to_path_buf(),
                target: target.to_path_buf(),
                source,
                pid: pid.to_string(),
                fid: fid.to_string(),
                name: name.to_string(),
                hash,
                url: url.to_string(),
            });
        }

        let source = prefer_source(resource, &hash, target, source);
        if target.exists() {
            // 实例里是同一份文件就不用动；内容不同则补一份链接计划，让调用方覆盖它
            if !verify_target || hash_matches(target, &hash) {
                return None;
            }
            return Some(LinkPlan {
                resource,
                instance,
                base: instance_base.to_path_buf(),
                target: target.to_path_buf(),
                source,
                pid: pid.to_string(),
                fid: fid.to_string(),
                name: name.to_string(),
                hash,
                url: url.to_string(),
            });
        }

        if let Err(err) = try_link(
            resource,
            instance,
            instance_base,
            target,
            pid,
            fid,
            name,
            &hash,
            url,
        ) {
            mml_log::error_type(err);
        }
        return None;
    }

    let target_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(name);
    let source = download_path(resource, &hash, target_name)?;
    Some(LinkPlan {
        resource,
        instance,
        base: instance_base.to_path_buf(),
        target: target.to_path_buf(),
        source,
        pid: pid.to_string(),
        fid: fid.to_string(),
        name: name.to_string(),
        hash,
        url: url.to_string(),
    })
}
