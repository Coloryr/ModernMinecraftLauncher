//! 资源映射：下载前接入公共目录，启动前检查并修复硬链接

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use mml_base::file_item::{FileHash, FileItemObj, LaterRun};
use mml_link::LinkPlan;
use mml_link::info::ResourceType;
use mml_names::{
    i18_items::error_type::{CoreResult, ErrorType},
    names,
};
use tokio_util::sync::CancellationToken;

use crate::{GameInstance, launcher::file_online_info_obj::OnlineInfoObj};

/// 解析在线信息对应的实例内路径与资源类型
///
/// - game_path: 实例的 .minecraft 目录
/// - mods: 实例 mods 目录
/// - resourcepacks: 实例 resourcepacks 目录
/// - info: 在线信息
///
/// # 返回值
///
/// 只处理 mods / resourcepacks；其它路径返回 None
fn target_of(
    game_path: &Path,
    mods: &Path,
    resourcepacks: &Path,
    info: &OnlineInfoObj,
) -> Option<(ResourceType, PathBuf)> {
    if info.file.is_empty() || info.path.is_empty() {
        return None;
    }

    let path = Path::new(&info.path);
    let dir = if path.is_absolute() {
        path.to_path_buf()
    } else {
        game_path.join(path)
    };
    let target = dir.join(&info.file);
    let resource = if target.starts_with(mods) {
        ResourceType::Mod
    } else if target.starts_with(resourcepacks) {
        ResourceType::ResourcePack
    } else {
        return None;
    };

    Some((resource, target))
}

/// 实例里是否有该文件的禁用副本
///
/// - target: 正常文件名路径
///
/// # 返回值
///
/// 存在 .disable 或 .disabled 后缀时返回 true
fn disabled_exists(target: &Path) -> bool {
    [names::DISABLE_DOT_EXT, names::DISABLED_DOT_EXT]
        .iter()
        .any(|ext| {
            let path = format!("{}{}", target.to_string_lossy(), ext);
            Path::new(&path).is_file()
        })
}

/// 删除实例内文件后清掉对应映射记录
///
/// 整合包升级移除文件时调用，避免启动检查按旧记录把文件重新链回来。
///
/// - game: 实例句柄
/// - target: 已删除的实例内文件
pub(crate) fn forget_link(game: &GameInstance, target: &Path) {
    let (uuid, base) = {
        let game = game.read().unwrap();
        (game.uuid, game.get_base_path())
    };
    mml_link::remove_link(uuid, &base, target);
}

/// 把一批模组下载项改到公共目录，并返回下载后要补链的计划
///
/// 公共目录已有同一份文件时直接链接并把这些项从下载列表移除；没有时只改下载目标，
/// 下载完由 finish_downloads 补链。
///
/// - game: 实例句柄
/// - items: 下载列表，会被原地修改
///
/// # 返回值
///
/// 需要下载完成后补链的计划
pub(crate) fn prepare_downloads(
    game: &GameInstance,
    items: &mut Vec<FileItemObj>,
) -> Vec<LinkPlan> {
    if !mml_link::is_enabled() {
        return Vec::new();
    }

    let (uuid, base, game_path, mods, resourcepacks, online) = {
        let game = game.read().unwrap();
        (
            game.uuid,
            game.get_base_path(),
            game.get_game_path(),
            game.get_mods_path(),
            game.get_resourcepacks_path(),
            game.read_online_info(),
        )
    };

    let mut by_target: HashMap<PathBuf, (ResourceType, OnlineInfoObj)> = HashMap::new();
    for info in online.values() {
        if let Some((resource, target)) = target_of(&game_path, &mods, &resourcepacks, info) {
            by_target.insert(target, (resource, info.clone()));
        }
    }

    let mut plans = Vec::new();
    let mut kept = Vec::with_capacity(items.len());
    for mut item in items.drain(..) {
        let target = item.file.clone();
        let Some((resource, info)) = by_target.get(&target).cloned() else {
            kept.push(item);
            continue;
        };
        let hash = item
            .hash
            .get_sha1()
            .filter(|hash| !hash.is_empty())
            .unwrap_or_else(|| info.sha1.clone());
        if hash.is_empty() {
            kept.push(item);
            continue;
        }

        match mml_link::plan_download(
            resource,
            uuid,
            &base,
            &target,
            &info.modid,
            &info.fileid,
            &info.name,
            &hash,
            &info.url,
            true,
        ) {
            Some(plan) => {
                item.file = plan.source.clone();
                plans.push(plan);
                kept.push(item);
            }
            // 公共目录已有并链接好，这项不用再下
            None => {
                if !target.exists() {
                    kept.push(item);
                }
            }
        }
    }
    *items = kept;
    plans
}

/// 补完一批下载计划的硬链接
///
/// 下载成功就直接链接；下载失败但实例里已有同哈希文件时，把它复制回公共目录再链接。
///
/// - plans: prepare_downloads 返回的计划
pub(crate) fn finish_downloads(plans: Vec<LinkPlan>) {
    for plan in plans {
        let result = if plan.source.is_file() {
            mml_link::finish_plan(&plan)
        } else {
            mml_link::adopt_existing(&plan)
        };
        if let Err(err) = result {
            mml_log::error_type(err);
        }
    }
}

/// 单个实例的批量映射结果
#[derive(Clone, Copy, Debug, Default)]
pub struct MapResourceCount {
    /// 映射成功的文件数
    pub mapped: u32,
    /// 映射失败的文件数
    pub failed: u32,
    /// 没有实例文件、禁用或缺少哈希而跳过的文件数
    pub skipped: u32,
}

/// 统计一个实例里可处理的在线资源条目数
///
/// 与 map_instance_resources_with 的遍历口径一致，供批量处理进度条使用。
///
/// - game: 实例句柄
///
/// # 返回值
///
/// 该实例里能被 target_of 定位到的条目数
pub fn count_instance_resources(game: &GameInstance) -> u32 {
    if !mml_link::is_enabled() {
        return 0;
    }

    let (game_path, mods, resourcepacks, online) = {
        let game = game.read().unwrap();
        (
            game.get_game_path(),
            game.get_mods_path(),
            game.get_resourcepacks_path(),
            game.read_online_info(),
        )
    };

    online
        .values()
        .filter(|info| target_of(&game_path, &mods, &resourcepacks, info).is_some())
        .count() as u32
}

/// 把一个实例里所有带在线信息的模组 / 资源包搬进公共目录并补硬链接
///
/// 只处理实例里实际存在的文件；禁用文件、缺少 SHA1 的条目跳过。
///
/// - game: 实例句柄
///
/// # 返回值
///
/// 见 MapResourceCount
pub fn map_instance_resources(game: &GameInstance) -> MapResourceCount {
    map_instance_resources_with(game, |_| {})
}

/// 带逐条回调的实例批量映射
///
/// - game: 实例句柄
/// - on_item: 每处理完一个在线资源条目调用一次，供批量命令刷新进度并写日志
///
/// # 返回值
///
/// 见 MapResourceCount
pub fn map_instance_resources_with<F>(game: &GameInstance, mut on_item: F) -> MapResourceCount
where
    F: FnMut(&OnlineInfoObj),
{
    if !mml_link::is_enabled() {
        return MapResourceCount::default();
    }

    let (uuid, base, game_path, mods, resourcepacks, online) = {
        let game = game.read().unwrap();
        (
            game.uuid,
            game.get_base_path(),
            game.get_game_path(),
            game.get_mods_path(),
            game.get_resourcepacks_path(),
            game.read_online_info(),
        )
    };

    let mut count = MapResourceCount::default();
    for info in online.values() {
        let Some((resource, target)) = target_of(&game_path, &mods, &resourcepacks, info) else {
            continue;
        };
        if !target.is_file() || disabled_exists(&target) || info.sha1.is_empty() {
            count.skipped += 1;
            on_item(info);
            continue;
        }
        match mml_link::map_existing(
            resource,
            uuid,
            &base,
            &target,
            &info.modid,
            &info.fileid,
            &info.name,
            &info.sha1,
            &info.url,
        ) {
            Ok(()) => count.mapped += 1,
            Err(err) => {
                mml_log::error_type(err);
                count.failed += 1;
            }
        }
        on_item(info);
    }

    count
}

/// 启动前检查实例资源的硬链接
///
/// 在线信息里的模组 / 资源包逐个检查：公共目录有文件就补链接，没有就按记录的下载地址
/// 重新下载；下载失败时若实例里已有同哈希文件，复制回公共目录兜底。
///
/// - game: 实例句柄
/// - cancel: 启动取消令牌
///
/// # 返回值
///
/// 取消返回 TaskCancel；缺失文件无法下载且实例里也没有时返回 DownloadFileFail
pub async fn check_instance_links(
    game: &GameInstance,
    cancel: CancellationToken,
) -> CoreResult<()> {
    if !mml_link::is_enabled() {
        return Ok(());
    }

    let (uuid, base, game_path, mods, resourcepacks, online) = {
        let game = game.read().unwrap();
        (
            game.uuid,
            game.get_base_path(),
            game.get_game_path(),
            game.get_mods_path(),
            game.get_resourcepacks_path(),
            game.read_online_info(),
        )
    };

    let links = mml_link::get_instance_link_data(uuid, &base);
    let mut by_target: HashMap<PathBuf, OnlineInfoObj> = HashMap::new();
    for info in online.values() {
        if let Some((_, target)) = target_of(&game_path, &mods, &resourcepacks, info) {
            by_target.insert(target, info.clone());
        }
    }

    let mut plans = Vec::new();
    let mut items = Vec::new();
    let mut failed = false;
    for link in links {
        if link.path.is_empty() {
            continue;
        }
        let target = base.join(&link.path);
        let resource = if target.starts_with(&mods) {
            ResourceType::Mod
        } else if target.starts_with(&resourcepacks) {
            ResourceType::ResourcePack
        } else {
            continue;
        };
        if !target.is_file() && disabled_exists(&target) {
            continue;
        }

        // 在线信息按项目 ID 找；项目号变了就按实例内路径兜底，再不行查公共索引
        let meta = if let Some(info) = online
            .get(&link.pid)
            .cloned()
            .or_else(|| by_target.get(&target).cloned())
        {
            (
                link.pid.clone(),
                info.fileid.clone(),
                info.name.clone(),
                info.sha1.clone(),
                info.url.clone(),
            )
        } else if let Some(info) = mml_link::get_download_info(resource, &link.pid) {
            (
                info.pid.clone(),
                link.fid.clone(),
                info.name,
                info.hash,
                info.url,
            )
        } else {
            continue;
        };
        let (pid, fid, name, hash, url) = meta;
        if hash.is_empty() {
            continue;
        }

        let Some(plan) = mml_link::plan_download(
            resource, uuid, &base, &target, &pid, &fid, &name, &hash, &url, false,
        ) else {
            if !target.is_file() {
                failed = true;
            }
            continue;
        };

        // 公共目录已有文件、只是实例里没有或内容不对：直接补链，不需要再下载
        if plan.source.is_file() {
            if let Err(err) = mml_link::finish_plan(&plan) {
                mml_log::error_type(err);
                if !target.is_file() {
                    failed = true;
                }
            }
            continue;
        }

        if url.is_empty() {
            // 没有下载地址：只能拿实例里现有的同哈希文件兜底
            if let Err(err) = mml_link::adopt_existing(&plan) {
                mml_log::error_type(err);
                if !target.is_file() {
                    failed = true;
                }
            }
            continue;
        }

        items.push(FileItemObj {
            url: url.clone(),
            name: name.clone(),
            file: plan.source.clone(),
            hash: FileHash::Sha1(hash.clone()),
            later: LaterRun::None,
        });
        plans.push(plan);
    }

    if !items.is_empty() {
        let outcome = mml_downloader::start_download_task_cancellable(items, cancel).await;
        if outcome.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }
    }

    for plan in plans {
        let result = if plan.source.is_file() {
            mml_link::finish_plan(&plan)
        } else {
            mml_link::adopt_existing(&plan)
        };
        if let Err(err) = result {
            mml_log::error_type(err);
            if !plan.target.is_file() {
                failed = true;
            }
        }
    }

    if failed {
        return Err(ErrorType::DownloadFileFail);
    }

    Ok(())
}
