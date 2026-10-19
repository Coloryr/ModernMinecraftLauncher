//! 迁移窗口的命令
//!
//! 窗口自己不做迁移，只把「问一次 / 跳过 / 开始」翻译成内核调用；
//! 检测结果与进度都由 `detect` / `check` 的事件推给前端。

use crate::log_text::LogText;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Emitter};

use mml_base::get_base_dir;

use crate::dtos::{ColorMcInfoDto, ColorMcProgressDto, ColorMcReportDto};
use crate::err_keys;
use crate::listens;


use super::detect::{asked, detect, info, mark};
use super::migrate::migrate;


/// 迁移是否正在进行，防重入：迁移期间再点一次不该并发跑第二遍
pub(crate) static MIGRATING: AtomicBool = AtomicBool::new(false);

/// 迁移进度事件，即 `colormc-progress`
///
/// - `app`: 窗口句柄
/// - `data`: 本次进度，含阶段 / 已完成数 / 总数 / 文字
#[gui_macros::emit]
pub fn emit_colormc_progress(app: &AppHandle, data: ColorMcProgressDto) {
    let _ = app.emit(listens::COLORMC_PROGRESS, data);
}

/// 推一次进度
///
/// `app` 为 `None` 时不推，测试里没有界面句柄 —— 进度是"有界面就报"的东西，
/// 不该为了测试往引擎里塞一个假的 AppHandle，所以它一路是 `Option`。
///
/// - `app`: 窗口句柄，`None` 表示不推
/// - `stage`: 阶段名，取 `scan` / `copy` / `move` / `check`
/// - `done`: 已完成数
/// - `total`: 总数，`0` 表示未知
/// - `text`: 当前处理的条目相对路径
pub(crate) fn progress(app: Option<&AppHandle>, stage: &str, done: u32, total: u32, text: &str) {
    let Some(app) = app else {
        return;
    };
    emit_colormc_progress(
        app,
        ColorMcProgressDto {
            stage: stage.to_string(),
            done,
            total,
            text: text.to_string(),
        },
    );
}

/// 检查本机有没有可迁移的 ColorMC 数据；**问过了就返回 `None`**
///
/// 主窗口启动后调一次；返回 `Some` 时前端弹"要不要把数据搬过来"的三选一弹窗。
///
/// # 返回值
///
/// 探测到的 ColorMC 工作目录信息，含路径 / 来源 / 实例数 / 顶层条目；
/// 已经问过、没探测到、或探测结果就是运行目录时返回 `None`
#[gui_macros::ipc_group("colormc")]
#[gui_macros::command]
pub fn window_check_colormc() -> Option<ColorMcInfoDto> {
    if asked() {
        return None;
    }
    let found = detect()?;
    // 探测结果就是运行目录，不该发生：没什么可搬的，别弹窗
    if found.dir == get_base_dir() {
        return None;
    }
    mml_log::info(format!(
        "colormc detected: {} (from={})",
        found.dir.display(),
        found.from
    ));
    Some(info(found.dir, found.from))
}

/// 记下"不迁移"：以后不再问
///
/// # 返回值
///
/// 标记写成功返回 `Ok(())`；写标记失败返回 `Err`，内容为错误原文
#[gui_macros::ipc_group("colormc")]
#[gui_macros::command]
pub fn window_skip_colormc() -> Result<(), String> {
    mark("skip")?;
    mml_log::info(String::from("colormc migrate skipped by user"));
    Ok(())
}

/// 执行迁移并返回兼容性报告
///
/// - `app`: 窗口句柄，进度经它广播
/// - `source`: ColorMC 工作目录，前端把 [`window_check_colormc`] 给的那份传回来
/// - `mode`: 取 `"copy"` 表示复制、源目录保留，取 `"move"` 表示移动、源目录搬空后删除
///
/// 跑在阻塞线程池上，可能搬几十 GB，期间用 `colormc-progress` 事件报进度。
///
/// # 返回值
///
/// 迁移与兼容性检测的报告，含搬入条目数 / 失败项 / 实例结论表；
/// 模式非法、已有迁移在跑或迁移失败时返回 `Err`，内容为 i18n 键
#[gui_macros::ipc_group("colormc")]
#[gui_macros::command]
pub async fn window_migrate_colormc(
    app: AppHandle,
    source: String,
    mode: String,
) -> Result<ColorMcReportDto, String> {
    let moved = match mode.as_str() {
        "copy" => false,
        "move" => true,
        _ => return Err(String::from(err_keys::COLORMC_MODE)),
    };

    if MIGRATING.swap(true, Ordering::AcqRel) {
        return Err(String::from(err_keys::COLORMC_BUSY));
    }

    let handle = app.clone();
    let dst = get_base_dir();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let src = PathBuf::from(source.trim());
        migrate(Some(&handle), &src, &dst, moved)
    })
    .await;

    MIGRATING.store(false, Ordering::Release);

    match result {
        Ok(Ok(report)) => {
            // 只在**迁移成功后**记"问过了"；写不上只记日志：数据已经搬完，
            // 不该因为一个标记文件把整次迁移报成失败，顶多下次启动再问一遍
            if let Err(err) = mark(if moved { "move" } else { "copy" }) {
                mml_log::error(format!("colormc marker write failed: {err}"));
            }
            Ok(report)
        }
        Ok(Err(err)) => Err(err),
        Err(err) => {
            // 原始错误只进日志：把 i18n 键和原始串拼一起，前端既翻不出来也读不懂
            mml_log::error(
                LogText::ColormcMigrateFailed {
                    error: err.to_string(),
                }
                .text(),
            );
            Err(String::from(err_keys::COLORMC_TASK))
        }
    }
}
