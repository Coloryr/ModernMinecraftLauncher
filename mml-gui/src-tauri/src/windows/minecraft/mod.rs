//! 从官方启动器 `.minecraft` 导入实例
//!
//! 首次启动探测本机的 `.minecraft`，让用户挑一个版本导入成 M²L 实例。只搬游戏数据 ——
//! `mods` / `saves` / `config` / `options.txt` / `servers.dat` 这类；`versions` /
//! `libraries` / `assets` 由启动器自己补，不重复搬，见 [`detect::game_entries`]。
//!
//! 与 ColorMC 迁移同一个弹窗出现（见前端 `OldDataModal`），但两者互不影响：
//! 各自的"问过了"标记分别落在 `minecraft_migrate.json` 与 `colormc_migrate.json`。
//!
//! | 文件 | 职责 |
//! | --- | --- |
//! | `detect` | 候选目录、版本列表、标记文件 |
//! | `import` | 建实例 + 复制游戏数据 + 进度事件 |

mod detect;
mod import;

use tauri::{AppHandle, Emitter};

use crate::dtos::{MinecraftImportDto, MinecraftInfoDto, MinecraftProgressDto};
use crate::log_text::LogText;

/// 导入进度事件，即 `minecraft-progress`
///
/// - `app`: 窗口句柄
/// - `data`: 本次进度，含阶段 / 已复制数 / 总数 / 当前文件
#[gui_macros::emit]
pub fn emit_minecraft_progress(app: &AppHandle, data: MinecraftProgressDto) {
    let _ = app.emit(crate::listens::MINECRAFT_PROGRESS, data);
}

/// 推一次复制进度
///
/// - `app`: 应用句柄
/// - `done`: 已复制文件数
/// - `total`: 总文件数
/// - `text`: 当前文件，相对 `.minecraft` 的路径
pub(crate) fn progress(app: &AppHandle, done: u32, total: u32, text: &str) {
    emit_minecraft_progress(
        app,
        MinecraftProgressDto {
            stage: "copy".to_string(),
            done,
            total,
            text: text.to_string(),
        },
    );
}

/// 检查本机有没有可导入的 `.minecraft`；**问过了就返回 `None`**
///
/// 主窗口启动后调一次；返回 `Some` 时前端在"发现旧数据"弹窗里列出来。
///
/// # 返回值
///
/// 探测结果，含路径 / 版本列表 / 会搬的顶层条目；已经问过或没探测到返回 `None`
#[gui_macros::ipc_group("minecraft")]
#[gui_macros::command]
pub fn window_check_minecraft() -> Option<MinecraftInfoDto> {
    if detect::asked() {
        return None;
    }
    let root = detect::detect()?;
    let info = detect::info(root);
    if info.versions.is_empty() {
        return None;
    }
    mml_log::info(
        LogText::MinecraftDetected {
            dir: info.path.clone(),
        }
        .text(),
    );
    Some(info)
}

/// 记下"不导入"：以后不再问
///
/// # 返回值
///
/// 标记写成功返回 `Ok(())`，失败返回错误原文
#[gui_macros::ipc_group("minecraft")]
#[gui_macros::command]
pub fn window_skip_minecraft() -> Result<(), String> {
    detect::mark("skip", "")?;
    mml_log::info(LogText::MinecraftImportSkipped.text());
    Ok(())
}

/// 把 `.minecraft` 的某个版本导入成实例
///
/// 跑在阻塞线程池上，复制期间用 `minecraft-progress` 事件报进度。
///
/// - `app`: 应用句柄，进度经它广播
/// - `path`: `.minecraft` 目录，前端把 [`window_check_minecraft`] 给的那份传回来
/// - `version`: 版本 id，即 `versions` 下的目录名
/// - `name`: 实例名，缺省用版本 id
///
/// # 返回值
///
/// 见 [`MinecraftImportDto`]；目录 / 版本缺失、核心未加载或建实例失败返回错误文案
#[gui_macros::ipc_group("minecraft")]
#[gui_macros::command]
pub async fn window_import_minecraft(
    app: AppHandle,
    path: String,
    version: String,
    name: Option<String>,
) -> Result<MinecraftImportDto, String> {
    tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(import::import(&app, &path, &version, name))
    })
    .await
    .map_err(|err| err.to_string())?
}
