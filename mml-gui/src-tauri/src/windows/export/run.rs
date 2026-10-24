//! 执行导出
//!
//! 在线导出按项目编号拉文件，普通导出直接打包本地目录；
//! 全程报进度，跑着的时候不重复入队。

use std::{path::{PathBuf}, sync::{Arc, Mutex, atomic::{AtomicBool, AtomicUsize, Ordering}}};

use crate::err_keys;

use mml_base::{archives::{ArchiveType, IBaseArchiveGui}};
use mml_game::game_export::{ExportArg, ExportPackType};
use tauri::{AppHandle};

use crate::{dtos::export_dto::{ExportConfigDto, ExportProgressDto}};

use crate::windows::instance_snapshot;
use super::list::{EXPORT_RUNNING, emit_export_progress, online_of};

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

/// 进度回调：内核 `IBaseArchiveGui` → `export-progress` 事件
pub(super) struct ExportProgressGui {
    /// 应用句柄，事件经它转发到前端
    pub(super) app: AppHandle,
    /// 导出目标文件名，随每条进度一起下发
    pub(super) file: String,
    /// 总计条目数，由 `start` 记下：更新事件也带着它，前端才能算百分比
    pub(super) total: AtomicUsize,
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
pub fn export_run(app: AppHandle, uuid: String, config: ExportConfigDto) -> Result<(), String> {
    let permit = ExportPermit::acquire(&EXPORT_RUNNING)?;

    let game = instance_snapshot(&uuid)?;
    let server_pack = config.pack == "server";
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
    let online = online_of(&game);

    // 直接压缩的勾选相对实例目录（含 .minecraft 那一段），其余格式相对游戏目录
    let pick_base = if pack == ExportPackType::MML { game.get_base_path() } else { base.clone() };
    let selected = super::selection::collect(&pick_base, config.mods.iter().chain(config.files.iter()), &online, pack)?;

    // 取消令牌：关窗时前端调 export_cancel 置位，写文件的循环看到就收手
    let cancel = Arc::new(AtomicBool::new(false));
    *EXPORT_CANCEL.lock().unwrap() = Some(cancel.clone());

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
        gui: Some(Arc::new(ExportProgressGui {
            app: app.clone(),
            file: config.file.clone(),
            total: AtomicUsize::new(0),
        })),
    };

    let save_file = config.file.clone();
    // 同步命令跑在主线程上，没有 tokio reactor：必须借 tauri 的运行时
    tauri::async_runtime::spawn(async move {
        let _permit = permit;
        let res = if pack == ExportPackType::MML {
            let flag = cancel.clone();
            tokio::task::spawn_blocking(move || {
                if server_pack {
                    super::server_pack::export_pack(&game, &arg.file, config.include_launcher, &arg.gui, &flag)
                } else if arg.select.is_empty() {
                    // 一个文件都没勾：保持老行为，整份实例压缩
                    super::archive::native_instance(&game, &arg.file, &arg.gui, &flag)
                } else {
                    super::archive::native_instance_selected(&game, &arg.select, &arg.file, &arg.gui, &flag)
                }
            })
                .await.map_err(|e| e.to_string()).and_then(|result| result)
        } else {
            crate::windows::export_file::write_async(std::path::Path::new(&save_file), |file| async move {
                let mut arg = arg;
                arg.file = file;
                game.export(arg).await.map_err(|e| e.to_string())
            }).await
        };
        // 任务收尾：摘掉取消令牌，别让下一轮导出看见上一轮的取消位
        {
            let mut slot = EXPORT_CANCEL.lock().unwrap();
            if slot.as_ref().is_some_and(|flag| Arc::ptr_eq(flag, &cancel)) { *slot = None; }
        }
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
            if !running() { break; }
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
        self.total.store(total, Ordering::Relaxed);
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
        emit_export_progress(
            &self.app,
            ExportProgressDto {
                state: "running".to_string(),
                now: current as u64,
                total: self.total.load(Ordering::Relaxed) as u64,
                text: filename.unwrap_or_default(),
                error: None,
                file: self.file.clone(),
            },
        );
    }

    /// 打包完成：终态由 `export_run` 统一发，这里不做处理
    fn done(&self) {}

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
