//! 设置窗口：资源映射的存量处理

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Emitter};

use crate::dtos::{ResourceMapProgressDto, ResourceMapResultDto};
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
