//! 应用更新：下载新文件并落地
//!
//! 下载并发与轮询节拍都在这里，进度经事件上报，关窗时按实例收手。

use std::sync::Arc;


use mml_base::file_item::{FileItemObj};
use mml_game::GameInstance;
use tauri::{WebviewWindow};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use tokio::task::JoinSet;

use crate::dtos::{ModUpdateApplyDto, ModUpdateDoneDto, ModUpdateProgressDto};
use crate::log_text::LogText;

use crate::windows::resource::{block_on_instance, parse_instance};
use super::emit::{STAGE_FETCH, STAGE_DOWNLOAD, STAGE_FINISH, emit_resource_mods_update_progress, emit_resource_mods_update_done};
use super::newfile::{fetch_new_files};
use super::prepare::{prepare_one, finish_one};

/// 同时在查的模组数上限
///
/// CurseForge 不限速，串行查几百个要一分多钟；Modrinth 的每分钟配额在客户端内部排队，
/// 并发开多大都压在同一个限速器上，所以这个值只影响 CurseForge 那一半。
pub(super) const MAX_CONCURRENT: usize = 8;

/// 下载阶段多久同步一次下载任务的完成数，单位毫秒
///
/// 下载器没有"每个任务完成多少"的推送，只能查快照；250 毫秒足够跟上界面，
/// 又不至于把任务表的锁打热
pub(super) const DOWNLOAD_SYNC_MS: u64 = 250;

/// 一批模组更新的结果
pub(super) struct UpdateCounts {
    /// 换文件成功的条目数
    pub(super) ok: u32,
    /// 失败的条目数
    pub(super) failed: u32,
}

/// 下载更新：把选中的模组换成平台上的新文件
///
/// **整批合成一个下载任务**：先问平台把这一批的下载地址与哈希全拿到（只准备、不下载），
/// 一次交给下载器跑完，最后逐个收尾 —— 校验、删掉旧文件（进回收站）、写回在线信息与
/// 更新记录。逐个 `await` 单文件下载会把下载器配的线程数白白浪费掉，
/// 「下载」窗口里也会一个任务一个任务地往外冒。
///
/// **旧文件在下载成功之后才删**：下载失败时原文件原样留着，免得更新一下反而没得用；
/// 原本禁用着的模组更新完仍是禁用状态，不会顺手被启用。
///
/// 下载走全局下载器，进度在「下载」窗口可见（这一批只会是**一个**任务）；
/// 本命令校验完参数就返回，全部处理完再发完成事件，前端据此重拉列表。
///
/// - `window`: 发起更新的那个资源窗口，进度与完成事件只发给它
/// - `uuid`: 实例 uuid
/// - `items`: 勾选要更新的条目；空数组什么都不做
///
/// # 返回值
///
/// uuid 非法或实例不存在返回错误串；其余情况立即返回 `Ok(())`，
/// 单条失败只计数、不让一条坏掉整批，结果在完成事件里
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_mods_update_apply(
    window: WebviewWindow,
    uuid: String,
    items: Vec<ModUpdateApplyDto>,
) -> Result<(), String> {
    if items.is_empty() {
        return Ok(());
    }
    let instance = parse_instance(&uuid)?;
    // 关窗时要能收手：任务挂在这个窗口的模型上
    let op = crate::windows::resource::model::window_model(&uuid)?.begin();
    let token = op.token();
    let total = items.len() as u32;
    // 一开始就是"获取中"：这时在问平台，别让界面先喊"下载中"
    emit_resource_mods_update_progress(
        &window,
        ModUpdateProgressDto {
            stage: STAGE_FETCH.to_string(),
            done: 0,
            total: items.len(),
        },
    );

    tauri::async_runtime::spawn(async move {
        // 守卫跟着任务走：任务结束（或被丢掉）它才把令牌从模型里摘掉
        let _op = op;
        let batch_window = window.clone();
        // 内层单独成一个任务：它**panic 了也要把终态事件补出去** ——
        // 前端是在 `await` 这个完成事件，丢了就永远停在"下载中"
        // 用 `tokio::spawn` 而不是 `tauri::async_runtime::spawn`：后者的 JoinError 是
        // `tauri::Error`，拿不到 panic 内容；tokio 的 `JoinError` 能抠出来，日志里才有线索
        let handle = tokio::spawn(async move {
            run_update(instance, items, batch_window, token).await
        });
        let UpdateCounts { ok, failed } = match handle.await {
            Ok(counts) => counts,
            Err(err) => {
                // JoinError 的 Display 只说"task N panicked"，把 panic 的内容也抠出来：
                // 这条日志是定位"为什么卡住"的唯一线索
                let reason = if err.is_panic() {
                    match err.into_panic().downcast::<String>() {
                        Ok(text) => *text,
                        Err(payload) => match payload.downcast::<&'static str>() {
                            Ok(text) => (*text).to_string(),
                            Err(_) => LogText::UnknownPanic.text(),
                        },
                    }
                } else {
                    err.to_string()
                };
                mml_log::error(LogText::ModUpdateAborted { reason }.text());
                // 任务死在半路，只能按"这一批都没换成"收尾，先让界面能落地
                UpdateCounts { ok: 0, failed: total }
            }
        };
        emit_resource_mods_update_done(&window, ModUpdateDoneDto { ok, failed });
    });

    Ok(())
}

/// 跑完这一批更新：准备 → 一个下载任务 → 逐个收尾
///
/// - `instance`: 实例句柄
/// - `items`: 要更新的条目
/// - `window`: 进度事件只发给它
/// - `token`: 窗口的取消令牌，交给下载器，关窗即取消整批
///
/// # 返回值
///
/// 见 [`UpdateCounts`]
pub(super) async fn run_update(
    instance: GameInstance,
    items: Vec<ModUpdateApplyDto>,
    window: WebviewWindow,
    token: CancellationToken,
) -> UpdateCounts {
    let total = items.len();
    let mut ok = 0u32;
    let mut failed = 0u32;
    mml_log::info(LogText::ModUpdateStarted { total }.text());

    // ① 准备这一批的下载项：平台数据**批量取**（见 fetch_new_files），本地拼装再并发做。
    //
    // 以前每个模组都问一次平台，实测每次 1~2 秒，几十上百个就是好几分钟；
    // 期间一个文件都没开始下，界面只有一条滚动的进度条，看起来就是"卡在下载中"
    let dir = match block_on_instance(instance.clone(), |game| game.get_mods_path()).await {
        Ok(dir) => dir,
        Err(err) => {
            mml_log::error(
                LogText::ModUpdateModsDirFailed {
                    error: err.to_string(),
                }
                .text(),
            );
            return UpdateCounts {
                ok: 0,
                failed: total as u32,
            };
        }
    };
    let mut new_files = fetch_new_files(&items).await;

    let sem = Arc::new(Semaphore::new(MAX_CONCURRENT));
    let mut set = JoinSet::new();
    for item in &items {
        let sem = sem.clone();
        let item = item.clone();
        let instance = instance.clone();
        let dir = dir.clone();
        // 数据按条目取走，拼装时不用再克隆一份
        let data = new_files.remove(&item.new_file_id);
        set.spawn(async move {
            let _permit = sem.acquire().await.ok();
            let project_id = item.project_id.clone();
            (project_id, prepare_one(instance, &dir, &item, data).await)
        });
    }

    let mut pending = Vec::with_capacity(total);
    loop {
        tokio::select! {
            // 窗口关了：剩下的不再等，丢下 JoinSet 会把没跑完的请求一起 abort
            _ = token.cancelled() => break,
            joined = set.join_next() => {
                let Some(joined) = joined else { break };
                match joined {
                    // 只数**准备成功**的那几条：这个数正是接下来要下多少个文件，
                    // 这一条进度与「下载」窗口里那个任务的文件数才对得上
                    Ok((_, Ok(entry))) => {
                        pending.push(entry);
                        emit_resource_mods_update_progress(
                            &window,
                            ModUpdateProgressDto {
                                stage: STAGE_FETCH.to_string(),
                                done: pending.len(),
                                total,
                            },
                        );
                    }
                    Ok((project_id, Err(err))) => {
                        failed += 1;
                        // 失败原因逐条记：用户报"更新不动"时这是唯一的线索
                        mml_log::error(
                            LogText::ModUpdateItemFailed {
                                project_id,
                                error: err.to_string(),
                            }
                            .text(),
                        );
                    }
                    Err(err) => {
                        failed += 1;
                        mml_log::error(
                            LogText::ModUpdatePrepareFailed {
                                error: err.to_string(),
                            }
                            .text(),
                        );
                    }
                }
            }
        }
    }

    // ② 合成一个下载任务跑完：并发由下载器按配置的线程数决定。
    //    窗口关了就把令牌一取消，下载器会收手，这一批不再往下走
    let batch = pending.len();
    mml_log::info(LogText::ModUpdateDownloadable { count: batch, total }.text());
    if batch > 0 && !token.is_cancelled() {
        let objs: Vec<FileItemObj> = pending.iter().map(|entry| entry.obj.clone()).collect();
        mml_log::info(LogText::ModUpdateDownloadStarted { count: batch }.text());
        // 下载阶段跟着下载任务的**真实完成数**走：这样资源窗口那条进度与「下载」窗口
        // 里那个任务是同一个数。下载器没有"每个任务完成多少"的推送，只能查快照
        let (id_tx, id_rx) = tokio::sync::watch::channel(0u64);
        let sync = {
            let window = window.clone();
            let token = token.clone();
            tauri::async_runtime::spawn(poll_download(window, token, id_rx))
        };
        let outcome =
            mml_downloader::start_download_task_reporting(objs, token.clone(), move |id| {
                let _ = id_tx.send(id);
            })
            .await;
        sync.abort();
        mml_log::info(
            LogText::ModUpdateDownloadFinished {
                outcome: format!("{outcome:?}"),
            }
            .text(),
        );
    }

    // ③ 逐个收尾：**下成了的才换**，没下成或被取消的那几条原文件一个字不动
    for (index, entry) in pending.into_iter().enumerate() {
        let project_id = entry.project_id.clone();
        if entry.obj.check_hash() {
            match finish_one(&instance, entry) {
                Ok(()) => ok += 1,
                Err(err) => {
                    failed += 1;
                    mml_log::error(
                        LogText::ModUpdateItemFailed {
                            project_id,
                            error: err.to_string(),
                        }
                        .text(),
                    );
                }
            }
        } else {
            failed += 1;
            mml_log::error(LogText::ModUpdateHashMismatch { project_id }.text());
        }
        emit_resource_mods_update_progress(
            &window,
            ModUpdateProgressDto {
                stage: STAGE_FINISH.to_string(),
                done: index + 1,
                total: batch,
            },
        );
    }

    mml_log::info(LogText::ModUpdateFinished { ok, failed }.text());
    UpdateCounts { ok, failed }
}

/// 下载阶段把下载任务的完成数同步给资源窗口
///
/// - `window`: 进度事件只发给它
/// - `token`: 窗口的取消令牌，取消即停
/// - `id_rx`: 任务编号，下载任务入队后由启动方填进来；0 表示还没拿到
pub(super) async fn poll_download(
    window: WebviewWindow,
    token: CancellationToken,
    mut id_rx: tokio::sync::watch::Receiver<u64>,
) {
    loop {
        tokio::select! {
            _ = token.cancelled() => return,
            _ = tokio::time::sleep(std::time::Duration::from_millis(DOWNLOAD_SYNC_MS)) => {}
        }

        let id = *id_rx.borrow_and_update();
        if id == 0 {
            continue;
        }
        // 任务从列表里消失就说明结束了：完成数已经报过，剩下的交给收尾阶段
        let Some(task) = mml_downloader::get_tasks().into_iter().find(|task| task.id == id)
        else {
            return;
        };
        emit_resource_mods_update_progress(
            &window,
            ModUpdateProgressDto {
                stage: STAGE_DOWNLOAD.to_string(),
                done: task.completed,
                total: task.total,
            },
        );
    }
}
