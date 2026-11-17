//! 设置窗口：整理已下载资源列表里缺失的项目名

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use mml_link::IndexKind;
use mml_link::info::{ResourceInfoObj, ResourceType};
use mml_net::{curseforge_api, modrinth_api};
use tauri::{AppHandle, Emitter};
use tokio::sync::Semaphore;

use crate::dtos::{LinkResourceTidyProgressDto, LinkResourceTidyResultDto};
use crate::err_keys;
use crate::listens;

/// 项目名整理是否正在处理
static TIDY_RUNNING: AtomicBool = AtomicBool::new(false);

/// 项目名整理正在处理的守卫，命令退出或 panic 时自动复位
struct TidyGuard;

impl Drop for TidyGuard {
    fn drop(&mut self) {
        TIDY_RUNNING.store(false, Ordering::SeqCst);
    }
}

/// 整理项目名进度事件
///
/// - `app`: 应用句柄
/// - `dto`: 当前进度
#[gui_macros::emit]
pub fn emit_link_resource_tidy_progress(app: &AppHandle, dto: LinkResourceTidyProgressDto) {
    let _ = app.emit(listens::LINK_RESOURCE_TIDY_PROGRESS, dto);
}

/// CurseForge 单次批量查询的项目数上限，单位个
const CURSEFORGE_BATCH: usize = 50;
/// Modrinth 同时查询的项目数上限
const MODRINTH_CONCURRENCY: usize = 6;

/// 项目所在的平台
#[derive(Clone, Copy)]
enum ProjectSource {
    /// CurseForge
    CurseForge,
    /// Modrinth
    Modrinth,
}

/// 待补全项目名的项目
struct MissingProject {
    /// 项目 ID
    pid: String,
    /// 项目所在平台
    source: ProjectSource,
}

/// 整理已下载资源的项目名
///
/// 只查索引里还没有项目名的项目：CurseForge 走批量接口，Modrinth 并发查详情；
/// 查到的名字写回索引，前端刷新列表即可看到
///
/// # 返回值
///
/// 见 [`LinkResourceTidyResultDto`]；已在整理时返回错误键
#[gui_macros::ipc_group("settings")]
#[gui_macros::command]
pub async fn settings_tidy_link_resources(
    app: AppHandle,
) -> Result<LinkResourceTidyResultDto, String> {
    if TIDY_RUNNING.swap(true, Ordering::SeqCst) {
        return Err(err_keys::RESOURCE_TIDY_RUNNING.to_string());
    }
    let _guard = TidyGuard;

    let missing = tauri::async_runtime::spawn_blocking(collect_missing_projects)
        .await
        .unwrap_or_default();
    let total = missing.len() as u32;
    let mut result = LinkResourceTidyResultDto {
        total,
        ..Default::default()
    };
    let mut done = 0u32;
    emit_link_resource_tidy_progress(&app, LinkResourceTidyProgressDto { done, total });

    let mut curseforge = Vec::new();
    let mut modrinth = Vec::new();
    for item in missing {
        match item.source {
            ProjectSource::CurseForge => curseforge.push(item.pid),
            ProjectSource::Modrinth => modrinth.push(item.pid),
        }
    }

    for chunk in curseforge.chunks(CURSEFORGE_BATCH) {
        let mut ids = Vec::with_capacity(chunk.len());
        for pid in chunk {
            match pid.parse::<u64>() {
                Ok(id) => ids.push(id),
                Err(_) => result.failed += 1,
            }
        }
        if !ids.is_empty() {
            let wanted = ids.len() as u32;
            match curseforge_api::get_mods_info(ids).await {
                Ok(page) => {
                    let mut got = 0u32;
                    for item in page.data {
                        let project = item.name.trim();
                        if project.is_empty() {
                            continue;
                        }
                        remember_project(&item.id.to_string(), project);
                        got += 1;
                    }
                    result.filled += got;
                    result.failed += wanted.saturating_sub(got);
                }
                Err(_) => result.failed += wanted,
            }
        }
        done += chunk.len() as u32;
        emit_link_resource_tidy_progress(&app, LinkResourceTidyProgressDto { done, total });
    }

    let semaphore = Arc::new(Semaphore::new(MODRINTH_CONCURRENCY));
    let mut tasks = tokio::task::JoinSet::new();
    for pid in modrinth {
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|err| err.to_string())?;
        tasks.spawn(async move {
            let _permit = permit;
            let name = modrinth_api::get_project(&pid)
                .await
                .map(|project| project.title)
                .unwrap_or_default();
            (pid, name)
        });
    }
    while let Some(joined) = tasks.join_next().await {
        done += 1;
        match joined {
            Ok((pid, name)) if !name.trim().is_empty() => {
                remember_project(&pid, name.trim());
                result.filled += 1;
            }
            _ => result.failed += 1,
        }
        emit_link_resource_tidy_progress(&app, LinkResourceTidyProgressDto { done, total });
    }

    // 项目名是一次性补写，整理完直接落盘，别等下次退出
    mml_link::flush_pending();
    Ok(result)
}

/// 收集索引里还没有项目名的项目
///
/// # 返回值
///
/// 去重后的项目；同一个项目同时有模组与资源包时只查一次
fn collect_missing_projects() -> Vec<MissingProject> {
    let Some(store) = mml_link::index_store() else {
        return Vec::new();
    };
    let mut missing: HashMap<String, ProjectSource> = HashMap::new();
    for kind in [IndexKind::Mod, IndexKind::ResourcePack] {
        let known = store.projects(kind).unwrap_or_default();
        let Ok(rows) = store.load(kind) else {
            continue;
        };
        for (pid, payload) in rows {
            if known.contains_key(&pid) || missing.contains_key(&pid) {
                continue;
            }
            let source = match serde_json::from_slice::<ResourceInfoObj>(&payload) {
                Ok(info) => project_source(&info.url, &pid),
                Err(_) => project_source("", &pid),
            };
            missing.insert(pid, source);
        }
    }
    missing
        .into_iter()
        .map(|(pid, source)| MissingProject { pid, source })
        .collect()
}

/// 判断项目来自哪个平台
///
/// - `url`: 记录里的下载地址，可能为空
/// - `pid`: 项目 ID
///
/// # 返回值
///
/// 地址认得出来就按地址，否则纯数字按 CurseForge、其余按 Modrinth
fn project_source(url: &str, pid: &str) -> ProjectSource {
    let url = url.to_lowercase();
    if url.contains("modrinth") {
        ProjectSource::Modrinth
    } else if url.contains("forgecdn") || url.contains("curseforge") {
        ProjectSource::CurseForge
    } else if pid.parse::<u64>().is_ok() {
        ProjectSource::CurseForge
    } else {
        ProjectSource::Modrinth
    }
}

/// 记下项目名，模组与资源包两个索引都写一份
///
/// - `pid`: 项目 ID
/// - `project`: 项目名
fn remember_project(pid: &str, project: &str) {
    for resource in [ResourceType::Mod, ResourceType::ResourcePack] {
        mml_link::set_project_name(resource, pid, project);
    }
}
