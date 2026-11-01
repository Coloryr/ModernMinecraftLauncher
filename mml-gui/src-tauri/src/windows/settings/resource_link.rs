//! 设置窗口：资源映射的存量处理

use std::sync::atomic::{AtomicBool, Ordering};

use mml_link::IndexKind;
use mml_link::info::{ResourceInfoObj, ResourceType};
use mml_sys::open_helper;
use tauri::{AppHandle, Emitter};

use crate::dtos::{LinkResourceDto, ResourceMapProgressDto, ResourceMapResultDto};
use crate::err_keys;
use crate::listens;
use crate::log_text::LogText;

/// 存量映射是否正在处理
static RUNNING: AtomicBool = AtomicBool::new(false);

/// 存量映射正在处理的守卫，命令退出或 panic 时自动复位
struct RunningGuard;

impl Drop for RunningGuard {
    fn drop(&mut self) {
        RUNNING.store(false, Ordering::SeqCst);
    }
}

/// 存量映射是否正在处理
///
/// # 返回值
///
/// 正在处理时返回 `true`
pub(crate) fn running() -> bool {
    RUNNING.load(Ordering::SeqCst)
}

/// 批量资源映射进度事件
///
/// - `app`: 应用句柄
/// - `dto`: 当前进度
#[gui_macros::emit]
pub fn emit_resource_map_progress(app: &AppHandle, dto: ResourceMapProgressDto) {
    let _ = app.emit(listens::RESOURCE_MAP_PROGRESS, dto);
}

/// 把所有实例里带在线信息的模组 / 资源包搬进公共目录并补硬链接
///
/// 只处理实例里实际存在、带 SHA1 的文件；禁用的文件和缺失的文件跳过。
///
/// # 返回值
///
/// 见 ResourceMapResultDto；资源映射未开启或已有任务在跑时返回错误键
#[gui_macros::ipc_group("settings")]
#[gui_macros::command]
pub async fn settings_map_resources(app: AppHandle) -> Result<ResourceMapResultDto, String> {
    if !mml_link::is_enabled() {
        return Err(err_keys::RESOURCE_LINK_OFF.to_string());
    }
    // 游戏运行时模组文件可能正被读取，搬动它们既慢又容易失败
    if !mml_game::get_running_instances().is_empty() {
        return Err(err_keys::INSTANCE_RUNNING.to_string());
    }
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Err(err_keys::RESOURCE_MAP_RUNNING.to_string());
    }
    // 守卫先建：统计或起线程前 panic / 命令 future 被丢掉也能复位
    let guard = RunningGuard;

    let games = mml_game::get_instances();
    let instances = games.len() as u32;
    let total: u32 = games
        .iter()
        .map(mml_game::game_link::count_instance_resources)
        .sum();
    mml_log::info(LogText::ResourceMapStarted { instances, total }.text());
    emit_resource_map_progress(
        &app,
        ResourceMapProgressDto {
            instances,
            total,
            done: 0,
        },
    );

    let progress_app = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        // 守卫随闭包进入阻塞线程：命令 future 被丢掉时仍会一直挡着退出，直到文件搬完
        let _guard = guard;
        let mut result = ResourceMapResultDto {
            instances,
            ..Default::default()
        };
        let mut done = 0u32;
        for game in &games {
            let count = mml_game::game_link::map_instance_resources_with(game, |info| {
                done += 1;
                mml_log::info(
                    LogText::ResourceMapItem {
                        name: info.name.clone(),
                        done,
                        total,
                    }
                    .text(),
                );
                emit_resource_map_progress(
                    &progress_app,
                    ResourceMapProgressDto {
                        instances,
                        total,
                        done,
                    },
                );
            });
            result.mapped += count.mapped;
            result.failed += count.failed;
            result.skipped += count.skipped;
            // 一个实例处理完落一次盘：被强杀时最多丢这一个实例的索引记录
            mml_link::flush_pending();
        }
        mml_log::info(
            LogText::ResourceMapFinished {
                mapped: result.mapped,
                failed: result.failed,
                skipped: result.skipped,
            }
            .text(),
        );
        result
    })
    .await;
    if result.is_err() {
        RUNNING.store(false, Ordering::SeqCst);
    }
    result.map_err(|err| err.to_string())
}

/// 存量映射是否正在处理，供前端决定关窗提示
///
/// # 返回值
///
/// 正在处理时返回 `true`
#[gui_macros::ipc_group("settings")]
#[gui_macros::command]
pub fn settings_resource_map_running() -> bool {
    running()
}

/// 列出公共目录里已下载的资源
///
/// 读的是索引库里的记录；文件已被删掉的也列出来，标 `exists = false`
///
/// # 返回值
///
/// 按大小降序；索引没打开时为空列表
#[gui_macros::ipc_group("settings")]
#[gui_macros::command]
pub async fn settings_list_link_resources() -> Vec<LinkResourceDto> {
    tauri::async_runtime::spawn_blocking(list_link_resources)
        .await
        .unwrap_or_default()
}

/// 收集公共目录里的资源记录
///
/// # 返回值
///
/// 见 [`LinkResourceDto`]，按大小降序
fn list_link_resources() -> Vec<LinkResourceDto> {
    let Some(store) = mml_link::index_store() else {
        return Vec::new();
    };
    let mut list = Vec::new();
    for (kind, resource, id) in [
        (IndexKind::Mod, ResourceType::Mod, "mod"),
        (IndexKind::ResourcePack, ResourceType::ResourcePack, "resourcepack"),
    ] {
        let dir = match resource {
            ResourceType::Mod => mml_link::get_mod_download_path(),
            ResourceType::ResourcePack => mml_link::get_resource_pack_download_path(),
        };
        let Ok(rows) = store.load(kind) else {
            continue;
        };
        for (pid, payload) in rows {
            let Ok(info) = serde_json::from_slice::<ResourceInfoObj>(&payload) else {
                continue;
            };
            let meta = std::fs::metadata(dir.join(&info.file)).ok();
            list.push(LinkResourceDto {
                kind: id.to_string(),
                pid,
                name: info.name,
                file: info.file,
                hash: info.hash,
                size: meta.as_ref().map(|meta| meta.len()).unwrap_or(0),
                exists: meta.is_some(),
            });
        }
    }
    list.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));
    list
}

/// 在资源管理器里定位一份已下载资源
///
/// - `kind`: 资源类型，`mod` / `resourcepack`
/// - `file`: 公共目录下的文件名，取自 [`settings_list_link_resources`]
///
/// # 返回值
///
/// 类型不认识时返回错误键；文件不在时退而打开所在目录
#[gui_macros::ipc_group("settings")]
#[gui_macros::command]
pub fn settings_open_link_resource(kind: String, file: String) -> Result<(), String> {
    let dir = match kind.as_str() {
        "mod" => mml_link::get_mod_download_path(),
        "resourcepack" => mml_link::get_resource_pack_download_path(),
        _ => return Err(err_keys::LOCAL_FILE_UNKNOWN.to_string()),
    };
    let path = dir.join(&file);
    if path.is_file() {
        open_helper::open_file_with_explorer(&path);
    } else {
        open_helper::open_dir(&dir);
    }
    Ok(())
}

