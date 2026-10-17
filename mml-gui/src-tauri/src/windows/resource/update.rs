//! 资源管理窗口的模组更新检查：查有在线信息的模组在平台上有没有新文件
//!
//! 只认实例 `online_info.json` 里记着的模组，按 SHA1 关联，项目号与文件号都非空才算；
//! 手动放进 mods 的包不在表里，无从判断有没有更新，直接跳过。
//!
//! 判定口径：按实例的游戏版本与加载器过滤，取平台上**最新**的那个文件，
//! 文件号与装着的不一样就算有更新。平台在当前版本 / 加载器下一个文件都没有时不报 ——
//! 那说明这个包在本实例上没有可用的新版本，不是漏了。
//!
//! 检查要重扫一遍 mods 目录（在线信息按 SHA1 关联），代价与打开模组分类相同；
//! 之后逐个问平台，进度经 `resource-mods-update-progress` 事件上报。

use std::collections::HashMap;
use std::sync::Arc;

use std::path::{Path, PathBuf};

use mml_base::file_item::{FileItemObj, LaterRun};
use mml_game::GameInstance;
use mml_game::game_mods::{ModObj, add_disable_suffix};
use mml_game::launcher::file_online_info_obj::OnlineInfoObj;
use mml_game::launcher::mod_update_obj::ModUpdateRecordObj;
use mml_game::launcher::{ModPackType, get_source_type};
use mml_game::loader::LoaderType;
use mml_game::{curseforge, modrinth};
use mml_names::names;
use mml_net::curseforge_api::{self, CurseFogreArg, file_obj::CurseForgeFileDataObj};
use mml_net::modrinth_api::{self, version_obj::ModrinthVersionObj};
use tauri::{Emitter, WebviewWindow};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use tokio::task::JoinSet;

use crate::dtos::{
    ModUpdateApplyDto, ModUpdateCheckDto, ModUpdateDoneDto, ModUpdateDto, ModUpdateProgressDto,
    ModUpdateProjectDto, ModUpdateRecordDto,
};
use crate::err_keys;
use crate::listens;

use super::mods::{find_mod, mod_note_key, sha1_of};
use super::{block_on_instance, parse_instance};

/// 同时在查的模组数上限
///
/// CurseForge 不限速，串行查几百个要一分多钟；Modrinth 的每分钟配额在客户端内部排队，
/// 并发开多大都压在同一个限速器上，所以这个值只影响 CurseForge 那一半。
const MAX_CONCURRENT: usize = 8;

/// 进度阶段：检查有没有更新
const STAGE_CHECK: &str = "check";
/// 进度阶段：问平台取下载项
const STAGE_FETCH: &str = "fetch";
/// 进度阶段：真正在下文件
const STAGE_DOWNLOAD: &str = "download";
/// 进度阶段：收尾写回在线信息与记录
const STAGE_FINISH: &str = "finish";

/// 下载阶段多久同步一次下载任务的完成数，单位毫秒
///
/// 下载器没有"每个任务完成多少"的推送，只能查快照；250 毫秒足够跟上界面，
/// 又不至于把任务表的锁打热
const DOWNLOAD_SYNC_MS: u64 = 250;

/// 更新进度事件，事件名 `resource-mods-update-progress`
///
/// - `window`: 发起更新的那个资源窗口，由 Tauri 注入，进度只发给它
/// - `dto`: 进度载荷，已完成与总数
///
/// 检查与下载两个阶段共用：检查逐个问平台可能要一分钟（Modrinth 限 300 次 / 分钟），
/// 下载也要时间，没有进度用户都会以为卡死。
#[gui_macros::emit]
pub fn emit_resource_mods_update_progress(window: &WebviewWindow, dto: ModUpdateProgressDto) {
    let _ = window.emit(listens::RESOURCE_MODS_UPDATE_PROGRESS, dto);
}

/// 更新下载完成事件，事件名 `resource-mods-update-done`
///
/// - `window`: 发起更新的那个资源窗口，由 Tauri 注入
/// - `dto`: 成功与失败的个数
///
/// 下载走全局下载器，进度在「下载」窗口里看得见；这个事件只用来告诉资源窗口
/// "文件已经换过了，可以重拉列表" —— 不重拉的话界面上还是旧文件名、旧版本号。
#[gui_macros::emit]
pub fn emit_resource_mods_update_done(window: &WebviewWindow, dto: ModUpdateDoneDto) {
    let _ = window.emit(listens::RESOURCE_MODS_UPDATE_DONE, dto);
}

/// 一个待检查的模组，即"有在线信息的顶层条目"
struct Target {
    /// 当前条目 uuid，文件路径的 v5
    uuid: String,
    /// 当前文件名，禁用状态下带后缀
    file: String,
    /// 是否被禁用
    disable: bool,
    /// 显示名，元数据缺失时为空
    name: String,
    /// 当前版本号，没写时为空串
    version: String,
    /// 下载源
    source: ModPackType,
    /// 项目编号
    project_id: String,
    /// 当前文件编号
    file_id: String,
}

/// 挑出有在线信息、可以检查更新的模组
///
/// 只收顶层条目：内置 jar-in-jar 没有独立文件，也没有自己的在线信息。
///
/// - `list`: 模组扫描结果
/// - `online`: 实例的在线信息表，键是游戏路径，值里的 SHA1 用来和扫描结果对上
///
/// # 返回值
///
/// 待检查的模组，按扫描顺序；项目号或文件号缺一项的都跳过
fn collect_targets(list: &[ModObj], online: &HashMap<String, OnlineInfoObj>) -> Vec<Target> {
    // 在线信息表按项目号 / 路径索引，这里要按 SHA1 反查，先建一份索引
    let online_of: HashMap<&str, &OnlineInfoObj> = online
        .values()
        .filter(|info| !info.sha1.is_empty())
        .map(|info| (info.sha1.as_str(), info))
        .collect();

    list.iter()
        .filter_map(|item| {
            let info = online_of.get(sha1_of(&item.hash).as_str())?;
            if info.modid.is_empty() || info.fileid.is_empty() {
                return None;
            }
            let meta = item.info.first();
            Some(Target {
                uuid: item.uuid.to_string(),
                file: item
                    .file
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default(),
                disable: item.disable,
                name: meta.map(|item| item.name.clone()).unwrap_or_default(),
                version: meta
                    .and_then(|item| item.version.clone())
                    .unwrap_or_default(),
                source: get_source_type(&info.modid, &info.fileid),
                project_id: info.modid.clone(),
                file_id: info.fileid.clone(),
            })
        })
        .collect()
}

/// 模组更新检查：查有在线信息的模组在平台上有没有新文件
///
/// **不重扫 mods 目录**：用的是列表命令留在窗口模型里的那份扫描结果，
/// 检查只是拿它逐个问平台，点下去立刻就有总数。
///
/// - `window`: 发起检查的那个资源窗口，由 Tauri 注入，进度只发给它
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 见 [`ModUpdateCheckDto`]；实例不存在或模组列表还没加载时返回错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_mods_update_check(
    window: WebviewWindow,
    uuid: String,
) -> Result<ModUpdateCheckDto, String> {
    let model = super::model::window_model(&uuid)?;
    // 手动指定的下载源在实例的图形设置里，先读出来；没指定的按项目号 / 文件号形态判定
    let sources = {
        let instance = parse_instance(&uuid)?;
        let game = instance.read().unwrap();
        crate::gui_setting::load(&game).mods.mod_source
    };
    let Some((targets, version, loader)) = model.with_cache(|cache| {
        let mut targets = collect_targets(&cache.list, &cache.online);
        for target in targets.iter_mut() {
            if let Some(source) = sources
                .get(mod_note_key(&target.file))
                .filter(|s| !s.is_empty())
            {
                target.source = ModPackType::from_string(source);
            }
        }
        (targets, cache.version.clone(), cache.loader)
    }) else {
        // 模组列表还没加载过：没有"现有数据"可查，别去扫一遍凑数
        return Err(err_keys::MODS_NOT_LOADED.to_string());
    };

    // 关窗时要能收手：任务挂在这个窗口的模型上，见 super::model
    let op = model.begin();
    let token = op.token();

    let total = targets.len();
    // 先把总数报出去：前端据此显示 `0/N`，否则第一次进度回来前只有一个转圈
    emit_resource_mods_update_progress(
        &window,
        ModUpdateProgressDto {
            stage: STAGE_CHECK.to_string(),
            done: 0,
            total,
        },
    );

    let mut updates = Vec::new();
    let mut failed = 0usize;
    // 只留第一条错误：整片失败多半是网络 / 接口的问题，逐条记会把日志刷满
    let mut first_error: Option<String> = None;

    // 配额等待在客户端内部串行，这里只限制同时压在 CurseForge 上的请求数
    let sem = Arc::new(Semaphore::new(MAX_CONCURRENT));
    let mut set = JoinSet::new();
    for target in targets {
        let sem = sem.clone();
        let version = version.clone();
        set.spawn(async move {
            let _permit = sem.acquire().await.ok();
            check_one(&target, &version, loader).await
        });
    }

    let mut done = 0usize;
    loop {
        tokio::select! {
            // 窗口关了：剩下的不再等；丢下 JoinSet 会把没跑完的请求一起 abort
            _ = token.cancelled() => return Err(err_keys::CANCELLED.to_string()),
            joined = set.join_next() => {
                let Some(joined) = joined else { break };
                done += 1;
                emit_resource_mods_update_progress(
                    &window,
                    ModUpdateProgressDto {
                        stage: STAGE_CHECK.to_string(),
                        done,
                        total,
                    },
                );
                match joined {
                    Ok(Ok(Some(update))) => updates.push(update),
                    Ok(Ok(None)) => {}
                    // 单个模组查失败不影响其它：记个数，整份结果照常返回
                    Ok(Err(err)) => {
                        failed += 1;
                        first_error.get_or_insert(err);
                    }
                    Err(err) => {
                        failed += 1;
                        first_error.get_or_insert(err.to_string());
                    }
                }
            }
        }
    }

    if let Some(err) = first_error {
        mml_log::error(format!("模组更新检查：{failed} 个查询失败，首个：{err}"));
    }

    Ok(ModUpdateCheckDto {
        checked: total as u32,
        failed: failed as u32,
        updates,
    })
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
    let op = super::model::window_model(&uuid)?.begin();
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
        let (ok, failed) = match handle.await {
            Ok(counts) => counts,
            Err(err) => {
                // JoinError 的 Display 只说"task N panicked"，把 panic 的内容也抠出来：
                // 这条日志是定位"为什么卡住"的唯一线索
                let reason = if err.is_panic() {
                    match err.into_panic().downcast::<String>() {
                        Ok(text) => *text,
                        Err(payload) => match payload.downcast::<&'static str>() {
                            Ok(text) => (*text).to_string(),
                            Err(_) => String::from("未知 panic"),
                        },
                    }
                } else {
                    err.to_string()
                };
                mml_log::error(format!("模组更新异常结束：{reason}"));
                // 任务死在半路，只能按"这一批都没换成"收尾，先让界面能落地
                (0, total)
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
/// `(成功数, 失败数)`
async fn run_update(
    instance: GameInstance,
    items: Vec<ModUpdateApplyDto>,
    window: WebviewWindow,
    token: CancellationToken,
) -> (u32, u32) {
    let total = items.len();
    let mut ok = 0u32;
    let mut failed = 0u32;
    mml_log::info(format!("模组更新开始：{total} 条"));

    // ① 准备这一批的下载项：平台数据**批量取**（见 fetch_new_files），本地拼装再并发做。
    //
    // 以前每个模组都问一次平台，实测每次 1~2 秒，几十上百个就是好几分钟；
    // 期间一个文件都没开始下，界面只有一条滚动的进度条，看起来就是"卡在下载中"
    let dir = match block_on_instance(instance.clone(), |game| game.get_mods_path()).await {
        Ok(dir) => dir,
        Err(err) => {
            mml_log::error(format!("模组更新失败：取 mods 目录失败 {err}"));
            return (0, total as u32);
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
                        mml_log::error(format!("模组更新失败 {project_id}：{err}"));
                    }
                    Err(err) => {
                        failed += 1;
                        mml_log::error(format!("模组更新准备异常：{err}"));
                    }
                }
            }
        }
    }

    // ② 合成一个下载任务跑完：并发由下载器按配置的线程数决定。
    //    窗口关了就把令牌一取消，下载器会收手，这一批不再往下走
    let batch = pending.len();
    mml_log::info(format!("模组更新：{batch}/{total} 条可下载"));
    if batch > 0 && !token.is_cancelled() {
        let objs: Vec<FileItemObj> = pending.iter().map(|entry| entry.obj.clone()).collect();
        mml_log::info(format!("模组更新：开始下载 {batch} 个文件"));
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
        mml_log::info(format!("模组更新：下载结束 {outcome:?}"));
    }

    // ③ 逐个收尾：**下成了的才换**，没下成或被取消的那几条原文件一个字不动
    for (index, entry) in pending.into_iter().enumerate() {
        let project_id = entry.project_id.clone();
        if entry.obj.check_hash() {
            match finish_one(&instance, entry) {
                Ok(()) => ok += 1,
                Err(err) => {
                    failed += 1;
                    mml_log::error(format!("模组更新失败 {project_id}：{err}"));
                }
            }
        } else {
            failed += 1;
            mml_log::error(format!("模组更新失败 {project_id}：下载的文件校验不过"));
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

    mml_log::info(format!("模组更新：收尾完成 ok={ok} failed={failed}"));
    (ok, failed)
}

/// 下载阶段把下载任务的完成数同步给资源窗口
///
/// - `window`: 进度事件只发给它
/// - `token`: 窗口的取消令牌，取消即停
/// - `id_rx`: 任务编号，下载任务入队后由启动方填进来；0 表示还没拿到
async fn poll_download(
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

/// 读模组更新记录
///
/// 记录落在实例目录的 `mod_update.json`，按项目编号分表，由「下载更新」与「替换版本」
/// 在换文件成功后追加。
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 见 [`ModUpdateProjectDto`]；**每个项目内部新的在前，项目之间按各自最新一条倒序**。
/// 实例不存在返回错误串，从没更新过时返回空表
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_mods_update_log(uuid: String) -> Result<Vec<ModUpdateProjectDto>, String> {
    let instance = parse_instance(&uuid)?;
    let list = instance.read().unwrap().read_mod_update();

    let mut groups: Vec<ModUpdateProjectDto> = list
        .into_iter()
        .map(|(project_id, item)| {
            // 文件里是旧的在前，给界面倒过来
            let mut update: Vec<ModUpdateRecordDto> = item
                .update
                .into_iter()
                .map(|record| ModUpdateRecordDto {
                    file: record.file,
                    old_version: record.old_version,
                    version: record.version,
                    old_file_id: record.old_file_id,
                    file_id: record.file_id,
                    time: record.time.timestamp_millis(),
                    sha1: record.sha1,
                })
                .collect();
            update.reverse();
            ModUpdateProjectDto {
                project_id,
                name: item.name,
                update,
            }
        })
        .collect();
    // 项目之间按最新一条的时间倒序；没有记录的项目排最后
    groups.sort_by_key(|group| {
        std::cmp::Reverse(group.update.first().map_or(0, |item| item.time))
    });

    Ok(groups)
}

/// 删掉一条模组更新记录
///
/// 下标在该项目的更新历史里数，按 [`resource_mods_update_log`] 的**返回顺序**，
/// 即 0 是最近那一条。记录没有唯一 id，用下标定位；弹窗一次读一份、删完立刻重读，
/// 不存在并发改表。
///
/// - `uuid`: 实例 uuid
/// - `project_id`: 项目编号，即 `mod_update.json` 里的键
/// - `index`: 该项目内的记录下标，从 0 起，新的在前
///
/// # 返回值
///
/// 实例不存在或写盘失败返回错误串；项目不存在或下标越界时什么都不做
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_mods_update_log_remove(
    uuid: String,
    project_id: String,
    index: usize,
) -> Result<(), String> {
    let instance = parse_instance(&uuid)?;
    let game = instance.read().unwrap();
    let mut list = game.read_mod_update();
    let Some(entry) = list.get_mut(&project_id) else {
        return Ok(());
    };
    if index >= entry.update.len() {
        return Ok(());
    }
    // 界面按"新的在前"给下标，文件里是旧的在前
    let at = entry.update.len() - 1 - index;
    entry.update.remove(at);
    // 这个项目没有记录了就整项摘掉，别在界面上留一个空组
    let empty = entry.update.is_empty();
    if empty {
        list.remove(&project_id);
    }

    game.save_mod_update(&list).map_err(|err| err.to_string())
}

/// 一批更新里的一条：待下载的新文件 + 下完之后收尾要用的信息
///
/// 下载是**整批**一个任务，所以准备与收尾分成两步，中间隔着那一次下载，
/// 见 [`resource_mods_update_apply`]。
struct PendingUpdate {
    /// 待下载的新文件
    obj: FileItemObj,
    /// 要删掉的旧文件
    old: PathBuf,
    /// 旧文件是不是禁用着；是的话新文件也补上禁用后缀
    disable: bool,
    /// 项目编号，写在线信息与更新记录都用它
    project_id: String,
    /// 模组名字，记进更新记录
    name: String,
    /// 要写回的在线信息
    info: OnlineInfoObj,
    /// 要追加的更新记录
    record: ModUpdateRecordObj,
}

/// 批量取回来的新文件数据，按来源分家
///
/// 两份结构不一样，但下一步都只是"造下载项 + 造在线信息"，在这里收一下，
/// 逐条准备时就不用再管来源的差别
enum NewFileData {
    /// CurseForge 的文件信息
    CurseForge(CurseForgeFileDataObj),
    /// Modrinth 的版本信息
    Modrinth(ModrinthVersionObj),
}

/// 新文件号 → 平台数据
type NewFileMap = HashMap<String, NewFileData>;

/// 这一批要问的文件号，按来源分好
struct NewFileIds {
    /// CurseForge 的数字文件号，去重升序
    curseforge: Vec<u64>,
    /// Modrinth 的版本号，去重升序
    modrinth: Vec<String>,
}

/// 一次批量问多少个文件号
///
/// Modrinth 的版本号拼在 URL 上，太多会让请求行过长；CurseForge 虽然走 POST 体，
/// 但接口对 `fileIds` 的长度也有上限，两边都按这个值分批，别赌上限
const FILE_IDS_PER_REQUEST: usize = 50;

/// 把这一批条目按来源分成两组文件号，各自去重
///
/// - `items`: 这一批要更新的条目
///
/// # 返回值
///
/// 见 [`NewFileIds`]；来源不认识的条目直接忽略
fn split_new_file_ids(items: &[ModUpdateApplyDto]) -> NewFileIds {
    let mut curseforge = Vec::new();
    let mut modrinth = Vec::new();
    for item in items {
        match ModPackType::from_string(&item.source) {
            // 数字号解析不了的先不收：逐条准备时会报"文件号不合法"
            ModPackType::CurseForge => {
                if let Ok(id) = item.new_file_id.parse::<u64>() {
                    curseforge.push(id);
                }
            }
            ModPackType::Modrinth => modrinth.push(item.new_file_id.clone()),
            _ => {}
        }
    }
    curseforge.sort_unstable();
    curseforge.dedup();
    modrinth.sort();
    modrinth.dedup();
    NewFileIds { curseforge, modrinth }
}

/// 一次问回这一批的新文件
///
/// 下载更新动辄几十上百个模组，逐个问平台就是几十上百次往返，实测每次 1~2 秒；
/// 两个接口都支持按文件号批量取，所以按来源各发一次请求。
///
/// - `items`: 这一批要更新的条目
///
/// # 返回值
///
/// 新文件号 → 平台数据；某一类请求失败时只记日志、返回这一类的空表，
/// 由逐条准备报"文件找不到"，不连累另一类
async fn fetch_new_files(items: &[ModUpdateApplyDto]) -> NewFileMap {
    let NewFileIds {
        curseforge,
        modrinth,
    } = split_new_file_ids(items);
    let mut map = NewFileMap::new();

    for chunk in curseforge.chunks(FILE_IDS_PER_REQUEST) {
        match curseforge_api::get_files(chunk.to_vec()).await {
            Ok(list) => {
                for data in list {
                    map.insert(data.id.to_string(), NewFileData::CurseForge(data));
                }
            }
            Err(err) => mml_log::error(format!("批量取 CurseForge 文件失败：{err}")),
        }
    }

    for chunk in modrinth.chunks(FILE_IDS_PER_REQUEST) {
        match modrinth_api::get_versions(chunk.to_vec()).await {
            Ok(list) => {
                for data in list {
                    map.insert(data.id.clone(), NewFileData::Modrinth(data));
                }
            }
            Err(err) => mml_log::error(format!("批量取 Modrinth 版本失败：{err}")),
        }
    }

    map
}

/// 单独问一个文件：批量取没拿到时的退路
///
/// - `source`: 下载源
/// - `item`: 要更新的条目
///
/// # 返回值
///
/// 见 [`NewFileData`]；文件号非法或平台没有这个文件时返回错误串
async fn fetch_one(source: ModPackType, item: &ModUpdateApplyDto) -> Result<NewFileData, String> {
    match source {
        ModPackType::CurseForge => {
            let fid: u64 = item
                .new_file_id
                .parse()
                .map_err(|_| err_keys::BAD_FILE_ID.to_string())?;
            let mut list = curseforge_api::get_files(vec![fid])
                .await
                .map_err(|err| err.to_string())?;
            let data = list
                .pop()
                .ok_or_else(|| err_keys::FILE_NOT_FOUND.to_string())?;
            Ok(NewFileData::CurseForge(data))
        }
        ModPackType::Modrinth => {
            let data = modrinth_api::get_version(&item.project_id, &item.new_file_id)
                .await
                .map_err(|err| err.to_string())?;
            Ok(NewFileData::Modrinth(data))
        }
        _ => Err(err_keys::SOURCE_TYPE.to_string()),
    }
}

/// 准备一条更新：把平台数据拼成下载项，并攒好收尾要用的信息
///
/// **不碰网络**：平台数据由 [`fetch_new_files`] 批量取好，这里只做本地拼装。
///
/// - `instance`: 实例句柄
/// - `dir`: mods 目录，调用方取一次共用
/// - `item`: 要更新的那一条
/// - `data`: 这条的新文件平台数据；批量取那一趟没拿到时是 `None`
///
/// # 返回值
///
/// 旧文件找不到、平台数据缺失、文件号非法返回错误串
async fn prepare_one(
    instance: GameInstance,
    dir: &Path,
    item: &ModUpdateApplyDto,
    data: Option<NewFileData>,
) -> Result<PendingUpdate, String> {
    let source = ModPackType::from_string(&item.source);
    if !matches!(source, ModPackType::CurseForge | ModPackType::Modrinth) {
        return Err(err_keys::SOURCE_TYPE.to_string());
    }

    // 旧文件按 uuid 定位；顺带记下它是不是禁用着，更新完要接着禁用
    let old = find_mod(instance.clone(), &item.uuid).await?;

    // 批量那一趟没拿到：退回逐个取。慢一点，但一次批量请求出问题不会把整批
    // 都判成"文件找不到"，批量只当加速用
    let data = match data {
        Some(data) => data,
        None => fetch_one(source, item).await?
    };

    let (mut obj, info) = match data {
        NewFileData::CurseForge(mut data) => {
            let obj = curseforge::make_file_item_obj(&mut data, dir);
            let info = curseforge::make_file_online_info_obj(&mut data, names::GAME_MODS_DIR);
            (obj, info)
        }
        NewFileData::Modrinth(data) => {
            // Modrinth 偶尔给一个 files 都不带的版本；两个构造函数都会直接取 files[0]
            if data.files.is_empty() {
                return Err(err_keys::FILE_NOT_FOUND.to_string());
            }
            let obj = modrinth::make_download_obj(&data, dir);
            let info = modrinth::make_file_online_obj(&data, names::GAME_MODS_DIR);
            (obj, info)
        }
    };

    // 换之前那一份的文件号：覆盖在线信息之前先取出来，给更新记录用
    let old_file_id = {
        let game = instance.read().unwrap();
        game.read_online_info()
            .get(&item.project_id)
            .map(|data| data.fileid.clone())
            .unwrap_or_default()
    };
    // 记录在这一步就拼好：等下载完再回头问平台是多余的，那时在线信息也已经被覆盖
    let record = ModUpdateRecordObj {
        file: info.file.clone(),
        old_version: item.old_version.clone(),
        version: item.new_version.clone(),
        old_file_id,
        file_id: info.fileid.clone(),
        time: chrono::Local::now().fixed_offset(),
        sha1: info.sha1.clone(),
    };

    // 换旧文件交给下载器的后续操作：**下完一个换一个**，不用等整批下完，
    // mods 里也不会新旧同名文件同时存在。同名的不交给它 —— 那会把刚下好的这份挪走
    if old.file != obj.file {
        obj.later = LaterRun::ReplaceOld(old.file.clone());
    }

    Ok(PendingUpdate {
        obj,
        old: old.file,
        disable: old.disable,
        project_id: item.project_id.clone(),
        name: item.name.clone(),
        info,
        record,
    })
}

/// 收尾一条更新：删旧文件、补禁用后缀、写回在线信息与更新记录
///
/// 调用方**先确认新文件已经下好**，这里不再碰网络。
///
/// - `instance`: 实例句柄
/// - `entry`: 准备阶段攒下的那条
///
/// # 返回值
///
/// 删旧文件或写盘失败返回错误串
fn finish_one(instance: &GameInstance, entry: PendingUpdate) -> Result<(), String> {
    // 正常情况旧文件已由下载器的 `LaterRun::ReplaceOld` 换掉（下完一个换一个），
    // 这里只是兜底；同名时 `stale_file` 本来就返回 `None`
    if let Some(stale) = stale_file(&entry.old, &entry.obj.file) {
        old_delete(stale)?;
    }
    // 原本禁用着：给新文件补上后缀，别让更新把用户禁用的模组启用了
    if entry.disable {
        add_disable_suffix(&entry.obj.file).map_err(|err| err.to_string())?;
    }

    let game = instance.read().unwrap();
    let mut online = game.read_online_info();
    online.insert(entry.project_id.clone(), entry.info);
    game.save_online_info(&online);

    // 记一条更新记录：从哪个版本换到哪个版本、两边的文件号、时间与新文件的 SHA1。
    // 记录是附属品，写失败只记日志，不让这次更新算失败
    if let Err(err) = game.append_mod_update(&entry.project_id, &entry.name, entry.record) {
        mml_log::error_type(err);
    }
    Ok(())
}

/// 更新完之后要删掉的旧文件，新旧同一个路径时返回 `None`
///
/// 平台换了内容却没改名是有的：这时下载已经覆盖了同一个路径，
/// 再删就是把刚下好的那份删掉。
///
/// - `old`: 旧文件路径
/// - `new`: 新文件路径
///
/// # 返回值
///
/// 要删的旧文件；新旧同一个路径时没有
fn stale_file<'a>(old: &'a Path, new: &Path) -> Option<&'a Path> {
    (!same_path(old, new)).then_some(old)
}

/// 两个路径是不是同一个文件，只比字面、不碰文件系统
///
/// Windows 上大小写不敏感：新旧文件名只差大小写时是**同一个文件**，
/// 下载已经把它换了内容，此时再按"旧路径"删一次就会把刚下好的那份删掉。
///
/// - `old`: 旧文件路径
/// - `new`: 新文件路径
///
/// # 返回值
///
/// 是同一个文件返回 `true`
fn same_path(old: &Path, new: &Path) -> bool {
    if cfg!(windows) {
        old.to_string_lossy().to_lowercase() == new.to_string_lossy().to_lowercase()
    } else {
        old == new
    }
}

/// 把旧文件移进回收站
///
/// - `path`: 旧文件路径
///
/// # 返回值
///
/// 删除失败返回错误串
fn old_delete(path: &Path) -> Result<(), String> {
    ModObj {
        file: path.to_path_buf(),
        ..Default::default()
    }
    .delete()
    .map_err(|err| err.to_string())
}

/// 查一个模组，判断装上的是不是最新
///
/// - `target`: 待检查的模组
/// - `version`: 实例的游戏版本
/// - `loader`: 实例的加载器
///
/// # 返回值
///
/// 有更新时是那条新文件；已是最新，或平台在当前版本 / 加载器下没有文件时返回 `Ok(None)`；
/// 请求失败返回 `Err`
async fn check_one(
    target: &Target,
    version: &str,
    loader: LoaderType,
) -> Result<Option<ModUpdateDto>, String> {
    let latest = match target.source {
        ModPackType::CurseForge => latest_curseforge(&target.project_id, version, loader).await?,
        ModPackType::Modrinth => latest_modrinth(&target.project_id, version, loader).await?,
        // 其余来源没有查询接口，也不该带着在线信息出现在这里
        _ => return Ok(None),
    };
    let Some(latest) = latest else {
        return Ok(None);
    };
    // 文件号一样就是同一个文件；平台已按版本与加载器过滤，剩下的都是本实例能装的
    if latest.id == target.file_id {
        return Ok(None);
    }
    Ok(Some(ModUpdateDto {
        uuid: target.uuid.clone(),
        file: target.file.clone(),
        disable: target.disable,
        name: target.name.clone(),
        version: target.version.clone(),
        source: target.source.to_string(),
        project_id: target.project_id.clone(),
        file_id: target.file_id.clone(),
        new_file_id: latest.id,
        new_version: latest.version,
        new_file: latest.file,
        date: latest.date,
    }))
}

/// 平台上的一个候选新文件
pub(super) struct LatestFile {
    /// 文件编号
    pub(super) id: String,
    /// 版本号；CurseForge 没有这个字段，取文件的显示名
    pub(super) version: String,
    /// 文件名
    pub(super) file: String,
    /// 发布时间，ISO 8601
    pub(super) date: String,
}

/// 取 CurseForge 上最新的那个文件
///
/// - `pid`: 项目编号
/// - `version`: 游戏版本
/// - `loader`: 加载器
///
/// # 返回值
///
/// 见 [`LatestFile`]；当前版本 / 加载器下一个文件都没有时返回 `Ok(None)`
async fn latest_curseforge(
    pid: &str,
    version: &str,
    loader: LoaderType,
) -> Result<Option<LatestFile>, String> {
    let list = curseforge_api::get_files_page(CurseFogreArg {
        id: Some(pid.to_string()),
        version: Some(version.to_string()),
        loader: curseforge::to_loader_id(&loader),
        ..Default::default()
    })
    .await
    .map_err(|err| err.to_string())?;

    Ok(newest_curseforge(list.data))
}

/// 在 CurseForge 的文件列表里挑发布时间最晚的那个
///
/// 不直接取第一个：接口没在文档里承诺排序，按 `fileDate` 挑才不赌它。
///
/// - `list`: 接口返回的那一页文件
///
/// # 返回值
///
/// 见 [`LatestFile`]；空列表返回 `None`
pub(super) fn newest_curseforge(list: Vec<CurseForgeFileDataObj>) -> Option<LatestFile> {
    list.into_iter()
        .max_by(|a, b| a.file_date.cmp(&b.file_date))
        .map(|item| LatestFile {
            id: item.id.to_string(),
            version: item.display_name,
            file: item.file_name,
            date: item.file_date,
        })
}

/// 取 Modrinth 上最新的那个版本
///
/// - `pid`: 项目编号
/// - `version`: 游戏版本
/// - `loader`: 加载器
///
/// # 返回值
///
/// 见 [`LatestFile`]；当前版本 / 加载器下一个版本都没有时返回 `Ok(None)`
async fn latest_modrinth(
    pid: &str,
    version: &str,
    loader: LoaderType,
) -> Result<Option<LatestFile>, String> {
    let loader = modrinth::to_loader_id(&loader);
    let list = modrinth_api::get_file_versions(pid, Some(version), loader.as_deref())
        .await
        .map_err(|err| err.to_string())?;

    Ok(newest_modrinth(list))
}

/// 在 Modrinth 的版本列表里挑发布时间最晚的那个
///
/// - `list`: 接口返回的版本列表
///
/// # 返回值
///
/// 见 [`LatestFile`]；空列表返回 `None`，版本里一个文件都没带时文件名是空串
pub(super) fn newest_modrinth(list: Vec<ModrinthVersionObj>) -> Option<LatestFile> {
    list.into_iter()
        .max_by(|a, b| a.date_published.cmp(&b.date_published))
        .map(|item| {
            // 取 primary 那一份；Modrinth 偶尔给一个主文件都不带的版本，退回第一个
            let file = item
                .files
                .iter()
                .find(|file| file.primary)
                .or_else(|| item.files.first())
                .map(|file| file.filename.clone())
                .unwrap_or_default();
            LatestFile {
                id: item.id,
                version: item.version_number,
                file,
                date: item.date_published,
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mml_base::file_item::FileHash;
    use mml_game::game_mods::ModItemObj;
    use mml_net::modrinth_api::version_obj::ModrinthVersionFileObj;
    use std::path::PathBuf;

    /// 造一个顶层模组条目，只填判定用得到的字段
    fn mod_item(sha1: &str, name: &str, version: &str) -> ModObj {
        ModObj {
            hash: FileHash::Sha1(sha1.to_string()),
            file: PathBuf::from(format!("{name}.jar")),
            info: vec![ModItemObj {
                name: name.to_string(),
                version: Some(version.to_string()),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    /// 造一条在线信息
    fn online(sha1: &str, modid: &str, fileid: &str) -> OnlineInfoObj {
        OnlineInfoObj {
            sha1: sha1.to_string(),
            modid: modid.to_string(),
            fileid: fileid.to_string(),
            ..Default::default()
        }
    }

    /// 造一个 CurseForge 文件
    fn cf_file(id: u64, date: &str) -> CurseForgeFileDataObj {
        CurseForgeFileDataObj {
            id,
            display_name: format!("file {id}"),
            file_name: format!("file-{id}.jar"),
            file_date: date.to_string(),
            ..Default::default()
        }
    }

    /// 造一个 Modrinth 版本，文件列表按 `(文件名, 是否 primary)` 给
    fn mr_version(id: &str, date: &str, files: &[(&str, bool)]) -> ModrinthVersionObj {
        ModrinthVersionObj {
            id: id.to_string(),
            version_number: format!("v-{id}"),
            date_published: date.to_string(),
            files: files
                .iter()
                .map(|(name, primary)| ModrinthVersionFileObj {
                    filename: name.to_string(),
                    primary: *primary,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }

    /// 只收有在线信息的模组：没在表里的、项目号或文件号缺一项的都要跳过
    #[test]
    fn collect_targets_keeps_online_only() {
        let list = vec![
            mod_item("aa", "sodium", "0.5.0"),
            mod_item("bb", "manual", "1.0"),
            mod_item("cc", "half", "1.0"),
        ];
        let mut table: HashMap<String, OnlineInfoObj> = HashMap::new();
        table.insert("100".into(), online("aa", "100", "200"));
        // bb 是手动放进去的，表里没有它的 SHA1
        // cc 只有项目号、没有文件号：认不出装的是哪个文件
        table.insert("101".into(), online("cc", "101", ""));

        let targets = collect_targets(&list, &table);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].file, "sodium.jar");
        assert_eq!(targets[0].name, "sodium");
        assert_eq!(targets[0].version, "0.5.0");
        assert_eq!(targets[0].project_id, "100");
        assert_eq!(targets[0].file_id, "200");
        // 项目号与文件号都是纯数字 → CurseForge
        assert_eq!(targets[0].source, ModPackType::CurseForge);
    }

    /// 没算出 SHA1 的文件不参与检查
    #[test]
    fn collect_targets_skips_without_hash() {
        let mut item = mod_item("dd", "nohash", "1.0");
        item.hash = FileHash::None;
        let mut table: HashMap<String, OnlineInfoObj> = HashMap::new();
        table.insert("102".into(), online("", "102", "103"));

        assert!(collect_targets(&[item], &table).is_empty());
    }

    /// 造一条「下载更新」的申请，只填批量取文件号用得到的字段
    fn apply_item(source: &str, project_id: &str, new_file_id: &str) -> ModUpdateApplyDto {
        ModUpdateApplyDto {
            uuid: String::new(),
            source: source.to_string(),
            project_id: project_id.to_string(),
            new_file_id: new_file_id.to_string(),
            name: String::new(),
            old_version: String::new(),
            new_version: String::new(),
        }
    }

    /// 批量取文件号：按来源分两组、各自去重；数字号解析不了的和陌生来源都跳过
    #[test]
    fn split_new_file_ids_groups_sources() {
        let items = vec![
            apply_item("curseforge", "100", "300"),
            apply_item("curseforge", "100", "300"),
            apply_item("curseforge", "101", "301"),
            // 数字号解析不了：留给逐条准备报 BAD_FILE_ID
            apply_item("curseforge", "102", "not-a-number"),
            apply_item("modrinth", "aaa", "xyz"),
            apply_item("modrinth", "aaa", "xyz"),
            // 陌生来源：不取
            apply_item("other", "1", "2"),
        ];

        let ids = split_new_file_ids(&items);
        assert_eq!(ids.curseforge, vec![300, 301]);
        assert_eq!(ids.modrinth, vec![String::from("xyz")]);
    }

    /// CurseForge：挑 `fileDate` 最晚的那个，不按列表顺序
    #[test]
    fn newest_curseforge_picks_latest_date() {
        let list = vec![
            cf_file(1, "2024-01-01T00:00:00Z"),
            cf_file(3, "2024-03-01T00:00:00Z"),
            cf_file(2, "2024-02-01T00:00:00Z"),
        ];

        let latest = newest_curseforge(list).unwrap();
        assert_eq!(latest.id, "3");
        assert_eq!(latest.file, "file-3.jar");
        assert_eq!(latest.version, "file 3");
        assert_eq!(latest.date, "2024-03-01T00:00:00Z");
    }

    /// CurseForge：空列表没有候选
    #[test]
    fn newest_curseforge_empty() {
        assert!(newest_curseforge(Vec::new()).is_none());
    }

    /// Modrinth：挑 `date_published` 最晚的那个，文件名取 primary 那一份
    #[test]
    fn newest_modrinth_picks_latest_date() {
        let list = vec![
            mr_version("a", "2024-01-01T00:00:00Z", &[("old.jar", true)]),
            mr_version(
                "c",
                "2024-03-01T00:00:00Z",
                &[("first.jar", false), ("new.jar", true)],
            ),
            mr_version("b", "2024-02-01T00:00:00Z", &[("mid.jar", true)]),
        ];

        let latest = newest_modrinth(list).unwrap();
        assert_eq!(latest.id, "c");
        assert_eq!(latest.version, "v-c");
        assert_eq!(latest.file, "new.jar");
        assert_eq!(latest.date, "2024-03-01T00:00:00Z");
    }

    /// Modrinth：没有 primary 时退回第一个文件，空列表没有候选
    #[test]
    fn newest_modrinth_without_primary() {
        let list = vec![mr_version(
            "a",
            "2024-01-01T00:00:00Z",
            &[("only.jar", false)],
        )];
        assert_eq!(newest_modrinth(list).unwrap().file, "only.jar");

        assert!(newest_modrinth(Vec::new()).is_none());
    }

    /// 新旧文件同名时不删旧文件：下载已经覆盖了同一个路径
    #[test]
    fn stale_file_skips_same_path() {
        let same = Path::new("mods/sodium.jar");
        assert!(stale_file(same, same).is_none());

        let old = Path::new("mods/sodium-0.5.jar");
        let new = Path::new("mods/sodium-0.6.jar");
        assert_eq!(stale_file(old, new), Some(old));
    }

    /// 只差大小写的两个名字在 Windows 上是同一个文件，同样不能删
    #[test]
    fn stale_file_skips_case_only_change() {
        let old = Path::new("mods/Sodium.jar");
        let new = Path::new("mods/sodium.jar");

        if cfg!(windows) {
            assert!(stale_file(old, new).is_none());
        } else {
            assert_eq!(stale_file(old, new), Some(old));
        }
    }
}
