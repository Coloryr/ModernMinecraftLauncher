//! 启动时的旧数据升级：状态、进度事件与查询 / 停止命令
//!
//! 升级在核心加载之前跑，界面用 [`UpgradeStatusDto`] 画"数据更新"进度。
//! 前端可能比升级晚挂载，所以状态放静态表里，由 `window_upgrade_status` 补读。
//! 用户可以随时叫停：旧文件留着，下次启动接着导（已经写进去的记录按主键覆盖）。

use std::sync::{
    LazyLock, Mutex,
    atomic::{AtomicBool, Ordering},
};

use mml_names::i18_items::info_type::InfoType;
use tauri::{AppHandle, Emitter};

use crate::dtos::UpgradeStatusDto;
use crate::log_text::NotifyText;
use crate::notify;
use crate::listens;

/// 用户请求停止
static CANCEL: AtomicBool = AtomicBool::new(false);

/// 升级进度
#[derive(Debug, Default)]
struct UpgradeState {
    /// 是否正在升级
    running: bool,
    /// 是否已经结束
    done: bool,
    /// 是否被用户叫停
    cancelled: bool,
    /// 已写入条数
    now: u32,
    /// 总条数
    total: u32,
}

static STATE: LazyLock<Mutex<UpgradeState>> = LazyLock::new(Default::default);

/// 当前状态
fn status() -> UpgradeStatusDto {
    let state = STATE.lock().unwrap();
    UpgradeStatusDto {
        running: state.running,
        done: state.done,
        cancelled: state.cancelled,
        now: state.now,
        total: state.total,
    }
}

/// 写状态并广播
///
/// - `app`: 应用句柄
/// - `state`: 新状态
fn set_state(app: &AppHandle, state: UpgradeState) {
    let dto = {
        let mut guard = STATE.lock().unwrap();
        guard.running = state.running;
        guard.done = state.done;
        guard.cancelled = state.cancelled;
        guard.now = state.now;
        guard.total = state.total;
        UpgradeStatusDto {
            running: guard.running,
            done: guard.done,
            cancelled: guard.cancelled,
            now: guard.now,
            total: guard.total,
        }
    };
    emit_upgrade_progress(app, dto);
}

/// 升级进度事件：前端据此画进度条
///
/// - `app`: 应用句柄
/// - `data`: 当前状态
#[gui_macros::emit]
pub fn emit_upgrade_progress(app: &AppHandle, data: UpgradeStatusDto) {
    let _ = app.emit(listens::UPGRADE_PROGRESS, data);
}

/// 升级是否在跑
///
/// # 返回值
///
/// 正在升级返回 `true`；关窗保护据此拒绝退出
pub fn running() -> bool {
    STATE.lock().unwrap().running
}

/// 是否有待升级的旧数据
///
/// # 返回值
///
/// 旧 JSON 索引还在时返回 `true`
pub fn pending() -> bool {
    mml_upgrade::pending(&mml_base::get_base_dir())
}

/// 读取当前升级状态
///
/// # 返回值
///
/// 见 [`UpgradeStatusDto`]；没有升级任务时 `done` 为 `true`
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub fn window_upgrade_status() -> UpgradeStatusDto {
    let mut dto = status();
    if !STATE.lock().unwrap().running && !dto.done {
        // 还没开始也没跑完：说明没有待升级的数据
        dto.done = true;
    }
    dto
}

/// 停止本次升级；已导入的记录保留，下次启动接着导
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub fn window_cancel_upgrade() {
    CANCEL.store(true, Ordering::SeqCst);
}

/// 有旧数据才跑一次升级
///
/// 升级在阻塞线程里做，进度回调按 100 毫秒节流后发往前端
///
/// - `app`: 应用句柄，事件发给它
pub async fn run(app: AppHandle) {
    let base = mml_base::get_base_dir();
    if !mml_upgrade::pending(&base) {
        set_state(
            &app,
            UpgradeState {
                done: true,
                ..Default::default()
            },
        );
        return;
    }

    CANCEL.store(false, Ordering::SeqCst);
    set_state(
        &app,
        UpgradeState {
            running: true,
            ..Default::default()
        },
    );

    let worker = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let Some(store) = mml_link::index_store() else {
            return Err(String::from("index store not open"));
        };
        let mut last = std::time::Instant::now();
        let report = mml_upgrade::migrate(&base, store, |now, total| {
            // 逐条发事件会把 webview 淹掉：按时间节流，收尾那一条一定发
            if now != total && last.elapsed().as_millis() < 100 {
                return !CANCEL.load(Ordering::SeqCst);
            }
            last = std::time::Instant::now();
            set_state(
                &worker,
                UpgradeState {
                    running: true,
                    now: now as u32,
                    total: total as u32,
                    ..Default::default()
                },
            );
            !CANCEL.load(Ordering::SeqCst)
        });
        Ok(report)
    })
    .await;

    match result {
        Ok(Ok(report)) => {
            // 升级是一次性的：完成时给一条系统通知，用户切走了也能看到
            if !report.cancelled {
                notify::notify(
                    &app,
                    NotifyText::UpgradeDone {
                        mods: report.mods,
                        resourcepacks: report.resourcepacks,
                    },
                );
            }
            set_state(
                &app,
                UpgradeState {
                    done: true,
                    cancelled: report.cancelled,
                    ..Default::default()
                },
            );
        }
        Ok(Err(err)) => {
            mml_log::info_type(InfoType::DownloadIndexImportFailed {
                file: String::from("download_index"),
                error: err,
            });
            set_state(
                &app,
                UpgradeState {
                    done: true,
                    ..Default::default()
                },
            );
        }
        Err(err) => {
            mml_log::info_type(InfoType::DownloadIndexImportFailed {
                file: String::from("download_index"),
                error: err.to_string(),
            });
            set_state(
                &app,
                UpgradeState {
                    done: true,
                    ..Default::default()
                },
            );
        }
    }
}
