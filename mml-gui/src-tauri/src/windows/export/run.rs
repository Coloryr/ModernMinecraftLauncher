//! 执行导出
//!
//! 在线导出按项目编号拉文件，普通导出直接打包本地目录；
//! 全程报进度，跑着的时候不重复入队。

use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use crate::{err_keys, log_text::LogText};

use mml_base::archives::{ArchiveType, IBaseArchiveGui};
use mml_game::game_export::{ExportArg, ExportPackType};
use tauri::AppHandle;

use crate::dtos::export_dto::{ExportConfigDto, ExportProgressDto};

use super::list::{EXPORT_RUNNING, emit_export_progress, online_of};
use crate::windows::instance_snapshot;

/// 当前导出任务的取消令牌；置位后写文件的循环会在下一条收手
static EXPORT_CANCEL: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

/// 是否正在导出，关窗保护与取消命令都用它
///
/// # 返回值
///
/// 有任务在跑时为 `true`
pub(crate) fn running() -> bool {
    EXPORT_RUNNING.load(Ordering::SeqCst)
}

/// 校验失败或后台任务退出时都释放导出占用。
struct ExportPermit<'a>(&'a AtomicBool);

impl<'a> ExportPermit<'a> {
    /// 抢占导出名额
    ///
    /// - `flag`: 导出占用标志，守卫 Drop 时复位
    ///
    /// # 返回值
    ///
    /// 抢到名额返回守卫；已有导出在跑返回 `err.exportRunning`
    fn acquire(flag: &'a AtomicBool) -> Result<Self, String> {
        flag.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map(|_| Self(flag))
            .map_err(|_| err_keys::EXPORT_RUNNING.to_string())
    }
}

impl Drop for ExportPermit<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// 一次导出里三个环节的耗时，单位秒
struct ExportSpans {
    /// 建包与清单，从任务开始到第一条进度
    open: f64,
    /// 写条目，从第一条进度到最后一条
    write: f64,
    /// 收尾，从最后一条进度到结束
    finish: f64,
}

/// 进度时间戳：导出结束后据此拆出建包、写入、收尾三段
pub(super) struct ExportClock {
    /// 任务开始的时刻
    started: Instant,
    /// 第一条进度：建包与清单已经写完
    first: Mutex<Option<Instant>>,
    /// 最后一条进度
    last: Mutex<Option<Instant>>,
    /// 最近一次进度报的条目总数
    total: AtomicUsize,
}

impl ExportClock {
    fn new() -> Self {
        Self {
            started: Instant::now(),
            first: Mutex::new(None),
            last: Mutex::new(None),
            total: AtomicUsize::new(0),
        }
    }

    /// 任务开始的时刻
    fn started(&self) -> Instant {
        self.started
    }

    /// 收到 `start`：建包与清单已写完，记下第一条进度
    ///
    /// - `total`: 本次要写的条目总数
    fn mark_start(&self, total: usize) {
        self.total.store(total, Ordering::Relaxed);
        let mut first = self.first.lock().unwrap();
        if first.is_none() {
            *first = Some(Instant::now());
        }
    }

    /// 收到一条文件进度
    fn mark_update(&self) {
        *self.last.lock().unwrap() = Some(Instant::now());
    }

    /// 进度里记下的条目总数
    fn total(&self) -> usize {
        self.total.load(Ordering::Relaxed)
    }

    /// 按结束时刻拆出建包、写入、收尾，单位秒
    ///
    /// - `end`: 导出的结束时刻
    ///
    /// # 返回值
    ///
    /// 见 [`ExportSpans`]
    fn spans(&self, end: Instant) -> ExportSpans {
        let first = self.first.lock().unwrap().unwrap_or(end);
        let last = self.last.lock().unwrap().unwrap_or(first);
        ExportSpans {
            open: (first - self.started).as_secs_f64(),
            write: (last - first).as_secs_f64(),
            finish: (end - last).as_secs_f64(),
        }
    }
}

/// 进度回调：内核 `IBaseArchiveGui` → `export-progress` 事件
pub(super) struct ExportProgressGui {
    /// 应用句柄，事件经它转发到前端
    pub(super) app: AppHandle,
    /// 导出目标文件名，随每条进度一起下发
    pub(super) file: String,
    /// 时间戳：各环节耗时与总条目数都记在里面
    pub(super) clock: Arc<ExportClock>,
    /// 取消令牌：内核逐条写入时问它要不要停
    pub(super) cancel: Arc<AtomicBool>,
    /// 上一条进度事件的时刻，用来限流
    pub(super) last_emit: Mutex<Option<Instant>>,
}

/// 发起导出：后台任务，进度经 export-progress 事件上报
///
/// 勾选条目的去向按格式分：
/// - CurseForge：正整数项目号与文件号写进 manifest，其余整份进 overrides。
///   它的清单格式里没有下载地址，所以前端填的 URL 在这个格式下用不上
/// - Modrinth：填了下载地址的写进 index 的 files，有在线来源的再补上项目号；
///   没填地址的和目录展开出来的文件一并进 overrides
///
/// - `app`: 应用句柄，事件与后台任务都用它
/// - `uuid`: 实例 uuid
/// - `config`: 导出设置，含目标文件名、格式与勾选的模组 / 文件
///
/// # 返回值
///
/// 任务已开始返回 `Ok(())`；已有导出在跑、uuid 非法、格式或文件名为空返回错误文案
#[gui_macros::ipc_group("export")]
#[gui_macros::command]
pub async fn export_run(
    app: AppHandle,
    uuid: String,
    config: ExportConfigDto,
) -> Result<(), String> {
    let began = Instant::now();
    let permit = ExportPermit::acquire(&EXPORT_RUNNING)?;

    let server_pack = config.pack == "server";
    // 服务器包可以一次带多个实例；其余格式始终只用传进来的那一个
    let uuids = if server_pack && !config.instances.is_empty() {
        config.instances.clone()
    } else {
        vec![uuid.clone()]
    };
    // 导出期间这些实例都算被占用：整个过程都在读它们的模组与配置，删到一半会读到空目录
    let tasks = uuids
        .iter()
        .map(|item| crate::windows::instance_task::begin(item))
        .collect::<Result<Vec<_>, _>>()?;
    let games = uuids
        .iter()
        .map(|item| instance_snapshot(item))
        .collect::<Result<Vec<_>, _>>()?;
    let game = games[0].clone();
    let pack = match config.pack.as_str() {
        "curseforge" => ExportPackType::CurseForge,
        "modrinth" => ExportPackType::Modrinth,
        "mmc" => ExportPackType::MMC,
        "hmcl" => ExportPackType::HMCL,
        "mml" | "server" => ExportPackType::MML,
        _ => {
            return Err(err_keys::EXPORT_PACK_TYPE.to_string());
        }
    };
    // 压缩档位对整份导出生效：本地 zip 与内核写包读的是同一个设置
    mml_base::archives::set_zip_level(match config.compress.as_str() {
        "normal" => mml_base::archives::ZipLevel::Normal,
        "solid" => mml_base::archives::ZipLevel::Solid,
        _ => mml_base::archives::ZipLevel::Fast,
    });

    if config.file.trim().is_empty() {
        // 这里校验的是**导出文件名**，原来报的是 err.serverEmpty，即服务器为空，
        // 前端弹出来的文案牛头不对马嘴
        return Err(err_keys::FILE_NAME.to_string());
    }

    super::archive::check_destination(std::path::Path::new(&config.file), &game.get_base_path())?;
    let base = game.get_game_path();

    // 取消令牌：关窗时前端调 export_cancel 置位，写文件的循环看到就收手
    let cancel = Arc::new(AtomicBool::new(false));
    *EXPORT_CANCEL.lock().unwrap() = Some(cancel.clone());
    let clock = Arc::new(ExportClock::new());
    let level = config.compress.clone();
    let gui = Arc::new(ExportProgressGui {
        app: app.clone(),
        file: config.file.clone(),
        clock: clock.clone(),
        cancel: cancel.clone(),
        last_emit: Mutex::new(None),
    });

    let online = online_of(&game);
    // 直接压缩的勾选相对实例目录（含 .minecraft 那一段），其余格式相对游戏目录
    let pick_base = if pack == ExportPackType::MML {
        game.get_base_path()
    } else {
        base.clone()
    };
    // 收集勾选要遍历目录、给在线模组算哈希，大实例上要读几百 MB：放到阻塞线程，并逐条报进度
    let picks: Vec<_> = config.mods.iter().chain(&config.files).cloned().collect();
    let total_picks = picks.len();
    let prepare_gui = gui.clone();
    let selected = tokio::task::spawn_blocking(move || {
        super::selection::collect_reporting(&pick_base, picks.iter(), &online, pack, &|index| {
            prepare_gui.prepare(index, total_picks);
        })
    })
    .await
    .map_err(|e| e.to_string())??;
    let prepared = began.elapsed();
    let server_options = config.server.clone();

    let arg = ExportArg {
        file: PathBuf::from(&config.file),
        pack,
        archive: ArchiveType::Zip,
        mods: selected.mods,
        files: selected.files,
        unselect: Vec::new(),
        select: selected.local,
        cancel: Some(cancel.clone()),
        name: config.name,
        author: config.author,
        version: config.version,
        summary: config.summary,
        gui: Some(gui),
    };

    let save_file = config.file.clone();
    // 同步命令跑在主线程上，没有 tokio reactor：必须借 tauri 的运行时
    tauri::async_runtime::spawn(async move {
        let _permit = permit;
        // 守卫跟着后台任务活：导出结束时才把实例从"占用中"放出来
        let _tasks = tasks;
        let res = if pack == ExportPackType::MML {
            let flag = cancel.clone();
            tokio::task::spawn_blocking(move || {
                if server_pack {
                    super::server_pack::export_pack(
                        &games,
                        &arg.file,
                        config.include_launcher,
                        &server_options,
                        &arg.gui,
                        &flag,
                    )
                } else if arg.select.is_empty() {
                    // 一个文件都没勾：保持老行为，整份实例压缩
                    super::archive::native_instance(&game, &arg.file, &arg.gui, &flag)
                } else {
                    super::archive::native_instance_selected(
                        &game,
                        &arg.select,
                        &arg.file,
                        &arg.gui,
                        &flag,
                    )
                }
            })
            .await
            .map_err(|e| e.to_string())
            .and_then(|result| result)
        } else {
            crate::windows::export_file::write_async(
                std::path::Path::new(&save_file),
                |file| async move {
                    let mut arg = arg;
                    arg.file = file;
                    game.export(arg).await.map_err(|e| e.to_string())
                },
            )
            .await
        };
        // 任务收尾：摘掉取消令牌，别让下一轮导出看见上一轮的取消位
        {
            let mut slot = EXPORT_CANCEL.lock().unwrap();
            if slot.as_ref().is_some_and(|flag| Arc::ptr_eq(flag, &cancel)) {
                *slot = None;
            }
        }
        // 耗时统计：直接压缩的四段明细在 write_zip 里已经打过，这里只补其它格式的
        let end = Instant::now();
        if pack != ExportPackType::MML {
            let spans = clock.spans(end);
            mml_log::info(
                LogText::ExportPackDetail {
                    open: spans.open,
                    write: spans.write,
                    finish: spans.finish,
                    commit: None,
                    entries: clock.total(),
                }
                .text(),
            );
        }
        mml_log::info(
            LogText::ExportTime {
                level,
                prepare: prepared.as_secs_f64(),
                pack: (end - clock.started()).as_secs_f64(),
                total: began.elapsed().as_secs_f64(),
                file: save_file.clone(),
            }
            .text(),
        );

        let event = match res {
            Ok(()) => ExportProgressDto {
                state: "done".to_string(),
                now: 0,
                total: 0,
                text: String::new(),
                error: None,
                file: save_file.clone(),
            },
            Err(e) => ExportProgressDto {
                state: "failed".to_string(),
                now: 0,
                total: 0,
                text: String::new(),
                error: Some(e.to_string()),
                file: save_file.clone(),
            },
        };
        emit_export_progress(&app, event);
    });

    Ok(())
}

/// 取消正在跑的导出
///
/// 置位后等任务收尾再返回：调用方接着要关窗口，关窗保护要等取消生效才放行。
///
/// # 返回值
///
/// 没有导出在跑时立即返回；等满 5 秒还没停也不报错，任务会在处理完当前文件后收手
#[gui_macros::ipc_group("export")]
#[gui_macros::command]
pub async fn export_cancel() -> Result<(), String> {
    if let Some(flag) = EXPORT_CANCEL.lock().unwrap().clone() {
        flag.store(true, Ordering::SeqCst);
    }
    tauri::async_runtime::spawn_blocking(|| {
        // 一个文件内部的复制打断不了，等它写完再看一次
        for _ in 0..100 {
            if !running() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_permit_releases_after_error() {
        let busy = AtomicBool::new(false);
        let failed = || -> Result<(), String> {
            let _permit = ExportPermit::acquire(&busy)?;
            assert!(ExportPermit::acquire(&busy).is_err());
            Err("invalid path".to_string())
        };
        assert!(failed().is_err());
        assert!(ExportPermit::acquire(&busy).is_ok());
    }
}

impl IBaseArchiveGui for ExportProgressGui {
    /// 开始打包：发一条 `running` 且进度归零的事件
    ///
    /// - `total`: 总计需要打包的文件数
    fn start(&self, total: usize) {
        self.clock.mark_start(total);
        emit_export_progress(
            &self.app,
            ExportProgressDto {
                state: "running".to_string(),
                now: 0,
                total: total as u64,
                text: String::new(),
                error: None,
                file: self.file.clone(),
            },
        );
    }

    /// 单个文件打包完成：推进进度并带上当前文件名
    ///
    /// - `filename`: 当前文件名
    /// - `current`: 已处理数量
    fn update(&self, filename: Option<String>, current: usize) {
        self.clock.mark_update();
        // 条目上千时逐条发事件会把主线程压在序列化与重渲染上，压到 25 条/秒
        if !self.emit_due() {
            return;
        }
        emit_export_progress(
            &self.app,
            ExportProgressDto {
                state: "running".to_string(),
                now: current as u64,
                total: self.clock.total() as u64,
                text: filename.unwrap_or_default(),
                error: None,
                file: self.file.clone(),
            },
        );
    }

    /// 打包完成：终态由 `export_run` 统一发，这里不做处理
    fn done(&self) {}

    /// 关窗取消时叫停批量写入，不用等整批写完
    ///
    /// # 返回值
    ///
    /// 取消位置位后返回 `true`
    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// 文件名含非法字符时一律同意替换，导出侧不拦
    ///
    /// - `_name`: 原始文件名，这里不看
    ///
    /// # 返回值
    ///
    /// 恒为 `true`，即同意替换
    fn file_rename(&self, _name: &str) -> bool {
        true
    }
}

/// 进度事件的最小间隔
const EMIT_INTERVAL: Duration = Duration::from_millis(40);

impl ExportProgressGui {
    /// 准备阶段的进度：只推进数字，界面沿用固定的「正在准备」文案
    ///
    /// 这一段在算模组哈希，没有文件名可报，所以 `text` 留空。
    ///
    /// - `now`: 已处理的勾选条数
    /// - `total`: 勾选总条数
    fn prepare(&self, now: usize, total: usize) {
        if !self.emit_due() {
            return;
        }
        emit_export_progress(
            &self.app,
            ExportProgressDto {
                state: "running".to_string(),
                now: now as u64,
                total: total as u64,
                text: String::new(),
                error: None,
                file: self.file.clone(),
            },
        );
    }

    /// 这一条进度该不该发出去
    ///
    /// # 返回值
    ///
    /// 距上一条超过 [`EMIT_INTERVAL`] 时返回 `true`
    fn emit_due(&self) -> bool {
        let mut last = self.last_emit.lock().unwrap();
        if last.is_some_and(|last| last.elapsed() < EMIT_INTERVAL) {
            return false;
        }
        *last = Some(Instant::now());
        true
    }
}
