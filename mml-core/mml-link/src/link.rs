//! 建立与解除链接
//!
//! 硬链接落盘、记录写入，以及删除链接后的清理。
use std::path::{Path, PathBuf};

use mml_names::i18_items::error_type::{
    CoreResult, DataNotFoundData, DownloadFileHashErrorData, ErrorType,
};
use mml_sys::path_helper;
use uuid::Uuid;

use crate::info::{LinkInfoObj, ResourceInfoObj, ResourceType};

use super::*;

/// 尝试把公共文件硬链接到实例
///
/// 目标已存在时不覆盖，交给调用方走正常下载与收尾流程。
///
/// - `resource`: 资源类型
/// - `instance`: 实例 uuid
/// - `instance_base`: 实例根目录，用来记相对路径
/// - `target`: 实例内目标文件
/// - `pid`: 项目 ID
/// - `fid`: 文件 ID
/// - `name`: 显示名
/// - `hash`: 文件 SHA1
/// - `url`: 下载地址
///
/// # 返回值
///
/// 成功复用并完成映射时返回 `true`；公共存储没有该文件或目标已存在时返回 `false`
pub fn try_link(
    resource: ResourceType,
    instance: Uuid,
    instance_base: &Path,
    target: &Path,
    pid: &str,
    fid: &str,
    name: &str,
    hash: &str,
    url: &str,
) -> CoreResult<bool> {
    let Some(hash) = normalize_hash(hash) else {
        return Ok(false);
    };
    let Some(source) = find_file(resource, &hash) else {
        return Ok(false);
    };
    if !hash_matches(&source, &hash) {
        return Ok(false);
    }
    let source = prefer_source(resource, &hash, target, source);
    if target.exists() {
        return Ok(false);
    }

    let is_link = link_file(&source, target)?;
    ensure_link_data(instance, instance_base);
    add_download_info(
        resource,
        ResourceInfoObj {
            pid: pid.to_string(),
            name: name.to_string(),
            hash,
            file: source
                .file_name()
                .and_then(|file| file.to_str())
                .unwrap_or_default()
                .to_string(),
            url: url.to_string(),
        },
        pid.to_string(),
    );
    record_link(instance, instance_base, target, pid, fid, is_link);
    save_link_data(instance, instance_base);
    // 单条手动链接：立刻落盘，别等下一次批量收尾
    flush_link_data();
    Ok(true)
}

/// 下载完成后登记公共文件并链接到实例
///
/// - `resource`: 资源类型
/// - `instance`: 实例 uuid
/// - `instance_base`: 实例根目录，用来记相对路径
/// - `target`: 实例内目标文件
/// - `source`: 已下载到公共目录的文件
/// - `pid`: 项目 ID
/// - `fid`: 文件 ID
/// - `name`: 显示名
/// - `hash`: 文件 SHA1
/// - `url`: 下载地址
///
/// # 返回值
///
/// 链接或复制失败时返回对应错误
pub fn finish_download(
    resource: ResourceType,
    instance: Uuid,
    instance_base: &Path,
    target: &Path,
    source: &Path,
    pid: &str,
    fid: &str,
    name: &str,
    hash: &str,
    url: &str,
) -> CoreResult<()> {
    // 下载器已经校验过；这里再确认一次，避免把坏文件记进索引
    let Some(hash) = normalize_hash(hash) else {
        return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
    };
    if !hash_matches(source, &hash) {
        return Err(ErrorType::DownloadFileHashError(
            DownloadFileHashErrorData {
                file: source.to_path_buf(),
                now: String::new(),
                hash,
            },
        ));
    }

    let is_link = link_file(source, target)?;
    ensure_link_data(instance, instance_base);
    add_download_info(
        resource,
        ResourceInfoObj {
            pid: pid.to_string(),
            name: name.to_string(),
            hash,
            file: source
                .file_name()
                .and_then(|file| file.to_str())
                .unwrap_or_default()
                .to_string(),
            url: url.to_string(),
        },
        pid.to_string(),
    );
    record_link(instance, instance_base, target, pid, fid, is_link);
    save_link_data(instance, instance_base);
    Ok(())
}

/// 完成一次下载计划：把公共目录里的文件链接到实例
///
/// - `plan`: plan_download 返回的计划
///
/// # 返回值
///
/// 链接或复制失败时返回对应错误
pub fn finish_plan(plan: &LinkPlan) -> CoreResult<()> {
    finish_download(
        plan.resource,
        plan.instance,
        &plan.base,
        &plan.target,
        &plan.source,
        &plan.pid,
        &plan.fid,
        &plan.name,
        &plan.hash,
        &plan.url,
    )
}

/// 把实例内一个已经存在的文件映射进公共目录
///
/// 用于更新这类先下到实例、再收尾的链路：公共目录有同哈希文件就补链，没有就把实例文件
/// 复制过去再链接。实例里没有文件时返回错误。
///
/// - resource: 资源类型
/// - instance: 实例 uuid
/// - instance_base: 实例根目录
/// - target: 实例内文件
/// - pid / fid / name / hash / url: 资源元数据
///
/// # 返回值
///
/// 映射失败时返回对应错误
#[allow(clippy::too_many_arguments)]
pub fn map_existing(
    resource: ResourceType,
    instance: Uuid,
    instance_base: &Path,
    target: &Path,
    pid: &str,
    fid: &str,
    name: &str,
    hash: &str,
    url: &str,
) -> CoreResult<()> {
    if !is_enabled() {
        return Ok(());
    }
    let Some(hash) = normalize_hash(hash) else {
        return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
    };
    let target_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(name);
    let source = if let Some(source) = find_file(resource, &hash) {
        if hash_matches(&source, &hash) {
            prefer_source(resource, &hash, target, source)
        } else {
            let _ = path_helper::delete(&source);
            download_path(resource, &hash, target_name)
                .ok_or(ErrorType::DataNotFound(DataNotFoundData::Info))?
        }
    } else {
        download_path(resource, &hash, target_name)
            .ok_or(ErrorType::DataNotFound(DataNotFoundData::Info))?
    };

    let plan = LinkPlan {
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
    };
    if plan.source.is_file() {
        finish_plan(&plan)
    } else {
        adopt_existing(&plan)
    }
}

/// 把旧的 `<sha1>.扩展名` 公共文件改成实例里的真实文件名
///
/// 只改文件名以哈希命名的旧文件；已有真实目标名时保持原样。
///
/// - resource: 资源类型
/// - hash: 小写 SHA1
/// - target: 实例内目标文件，用来取真实文件名
/// - source: 当前公共文件
///
/// # 返回值
///
/// 迁移后或保持不变的公共文件路径
pub(super) fn prefer_source(
    resource: ResourceType,
    hash: &str,
    target: &Path,
    source: PathBuf,
) -> PathBuf {
    let Some(name) = target.file_name().and_then(|name| name.to_str()) else {
        return source;
    };
    let Some(name) = file_name(name) else {
        return source;
    };
    if source.file_stem().and_then(|stem| stem.to_str()) != Some(hash) {
        return source;
    }

    let preferred = resource.download_dir().join(&name);
    if preferred == source || preferred.exists() {
        return source;
    }
    if let Err(err) = path_helper::move_file(&source, &preferred) {
        mml_log::error_type(err);
        return source;
    }

    if let Some(old) = source.file_name().and_then(|name| name.to_str()) {
        rename_index_file(resource, old, &name);
    }
    preferred
}

/// 把索引里指向旧文件名的记录改成新文件名
///
/// - resource: 资源类型
/// - old: 旧公共文件名
/// - new: 新公共文件名
pub(super) fn rename_index_file(resource: ResourceType, old: &str, new: &str) {
    let Some(store) = INDEX.get() else {
        return;
    };
    if let Err(err) = store.rename_file(resource.kind(), old, new) {
        log_index_error(err);
    }
}

/// 下载失败但实例里已有同哈希文件时，把它复制进公共目录再补链
///
/// - `plan`: 本次下载计划
///
/// # 返回值
///
/// 目标不存在或哈希不符时返回错误
pub fn adopt_existing(plan: &LinkPlan) -> CoreResult<()> {
    if !plan.target.is_file() || !hash_matches(&plan.target, &plan.hash) {
        return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
    }
    if let Some(parent) = plan.source.parent() {
        path_helper::create_dir_all(parent)?;
    }
    // 搬运而不是复制：实例文件先移进公共目录，再硬链接回原位
    path_helper::move_file(&plan.target, &plan.source)?;
    finish_plan(plan)
}

/// 把公共文件链接到实例，硬链接失败时回退为复制
///
/// - `source`: 公共文件
/// - `target`: 实例内目标文件
///
/// # 返回值
///
/// 成功时返回是否真的建成了硬链接；目标是已存在文件时会先移入回收站
pub(super) fn link_file(source: &Path, target: &Path) -> CoreResult<bool> {
    if let Some(parent) = target.parent() {
        path_helper::create_dir_all(parent)?;
    }
    if target.exists() {
        path_helper::move_to_trash(target)?;
    }

    match path_helper::hard_link(source, target) {
        Ok(()) => Ok(true),
        Err(_) => {
            path_helper::copy_file(source, target)?;
            Ok(false)
        }
    }
}

/// 确保实例映射已从实例目录读进内存
///
/// 记录文件随实例走；第一次写入前不读的话，保存会用空表覆盖已有记录。
///
/// - `instance`: 实例 uuid
/// - `base`: 实例根目录
pub(super) fn ensure_link_data(instance: Uuid, base: &Path) {
    let mut data = link_data().write().unwrap();
    if !data.contains_key(&instance) {
        let list = read_link_data(base);
        data.insert(instance, list);
    }
}

/// 记录实例内的一条映射
///
/// - `instance`: 实例 uuid
/// - `instance_base`: 实例根目录
/// - `target`: 实例内目标文件
/// - `pid`: 项目 ID
/// - `fid`: 文件 ID
/// - `is_link`: 是否真的建成了硬链接
pub(super) fn record_link(
    instance: Uuid,
    instance_base: &Path,
    target: &Path,
    pid: &str,
    fid: &str,
    is_link: bool,
) {
    let path = target
        .strip_prefix(instance_base)
        .unwrap_or(target)
        .to_string_lossy()
        .replace('\\', "/");

    let mut data = link_data().write().unwrap();
    let list = data.entry(instance).or_default();
    // 同一项目换文件（更新）时只保留新路径，旧路径的检查记录要一起清掉
    list.retain(|item| item.pid != pid || item.path == path);
    if let Some(old) = list.iter_mut().find(|item| item.path == path) {
        old.pid = pid.to_string();
        old.fid = fid.to_string();
        old.is_link = is_link;
    } else {
        list.push(LinkInfoObj {
            is_link,
            pid: pid.to_string(),
            fid: fid.to_string(),
            path,
        });
    }
}

/// 删除实例内一条映射记录
///
/// 用户删除资源时调用：否则下次启动检查会按旧记录把它重新链回来。
///
/// - instance: 实例 uuid
/// - instance_base: 实例根目录
/// - target: 已删除的实例内文件
pub fn remove_link(instance: Uuid, instance_base: &Path, target: &Path) {
    let path = target
        .strip_prefix(instance_base)
        .unwrap_or(target)
        .to_string_lossy()
        .replace('\\', "/");

    ensure_link_data(instance, instance_base);
    let mut data = link_data().write().unwrap();
    if let Some(list) = data.get_mut(&instance) {
        list.retain(|item| item.path != path);
    }

    drop(data);
    save_link_data(instance, instance_base);
    // 删除要立刻生效：否则强杀后旧记录还在，下次启动又把它链回去
    flush_link_data();
}
