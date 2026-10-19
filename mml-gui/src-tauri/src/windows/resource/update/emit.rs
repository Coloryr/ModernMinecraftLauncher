//! 更新检查的进度与完成事件
//!
//! 阶段名与前端进度条口径一一对应，事件经 `listens` 转成前端常量。



use tauri::{Emitter, WebviewWindow};

use crate::dtos::{ModUpdateDoneDto, ModUpdateProgressDto};
use crate::listens;



/// 进度阶段：检查有没有更新
pub(super) const STAGE_CHECK: &str = "check";

/// 进度阶段：问平台取下载项
pub(super) const STAGE_FETCH: &str = "fetch";

/// 进度阶段：真正在下文件
pub(super) const STAGE_DOWNLOAD: &str = "download";

/// 进度阶段：收尾写回在线信息与记录
pub(super) const STAGE_FINISH: &str = "finish";

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
