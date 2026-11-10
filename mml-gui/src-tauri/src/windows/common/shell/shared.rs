//! 跨窗口共用的解析与入口
//!
//! 窗口标签、实例句柄 / 快照、分组 uuid 与"唤出主窗口"这类被多个窗口共用的逻辑。
//! 这里不放 IPC 命令；依赖方向是 shell → registry / lifecycle / 游戏内核。

use mml_game::GameInstance;
use mml_game::launcher::instance_setting_obj::InstanceSettingObj;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::{err_keys, log_text::LogText};

use crate::windows::lifecycle::show_main_window;
use crate::windows::registry::{MAIN_WINDOW_UUID, WINDOWS_INFO};
use crate::windows::{add_resource, modpack_task};

/// 主窗口的标签，托盘与新实例唤回时按它找窗口
///
/// # 返回值
///
/// 形如 `mml-main`，取自窗口注册表，别处不要另写字面量
pub(crate) fn main_window_label() -> &'static str {
    WINDOWS_INFO
        .get(&MAIN_WINDOW_UUID)
        .map(|entry| entry.label)
        .unwrap_or("mml-main")
}

/// 是否有会自己发系统通知的长任务在跑
///
/// 资源下载与整合包安装在结束时各发一条通知，下载队列清空时就不必再补一条泛化的
/// 下载完成。资源任务要等下载器移除任务之后才写终态，所以此刻仍能读到它未结束。
pub(crate) fn has_self_notifying_task() -> bool {
    add_resource::has_active() || modpack_task::has_active()
}

/// 唤出主窗口：还在就显示并聚焦，已经关掉就按上次几何重建一个
///
/// 托盘点击与新实例请求「显示主窗口」都走这里。建窗是阻塞操作，不能在主线程
/// 直接做（托盘事件就在主线程），所以丢给异步运行时。
///
/// - `app`: 应用句柄
pub(crate) fn summon_main_window(app: &AppHandle) {
    mml_log::info(crate::log_text::LogText::MainWindowSummoned.text());
    if let Some(win) = app.get_webview_window(main_window_label()) {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
        return;
    }

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(err) = show_main_window(&app) {
            mml_log::error(LogText::TraySummonFailed { error: err }.text());
        }
    });
}

/// 解析前端传来的分组 uuid，空白 / 非法一律按"默认分组"处理
///
/// 分组以 uuid 为身份，见 mml-game 的 `game_group`；但 IPC 参数照例是字符串，
/// 所以各处统一在这里转一次。宁可落到默认分组，也不要因为一个过期 id 让整条命令失败。
///
/// - `group`: 前端传来的分组 uuid 字符串
///
/// # 返回值
///
/// 空白 / 非法 uuid 返回 `None`，调用方按默认分组处理
pub fn parse_group_id(group: Option<String>) -> Option<Uuid> {
    group
        .filter(|g| !g.trim().is_empty())
        .and_then(|g| Uuid::parse_str(&g).ok())
}

/// 解析实例 uuid，拿实例句柄
///
/// 要用实例上的方法（尤其异步的扫存档 / 读模组）时走它。
///
/// - `uuid`: 实例 uuid 字符串
///
/// # 返回值
///
/// uuid 非法返回 `err.uuid`，实例不存在返回 `err.gameNotFound`
pub(crate) fn instance_handle(uuid: &str) -> Result<GameInstance, String> {
    let uuid = Uuid::parse_str(uuid).map_err(|_| String::from(err_keys::UUID))?;
    mml_game::get_instance(&uuid).ok_or_else(|| String::from(err_keys::GAME_NOT_FOUND))
}

/// 取实例设置的快照，路径类方法都在这份克隆上
///
/// 紧接着要读盘（遍历目录、扫 level.dat）时走它：持着读锁走一圈会把别的窗口一起卡住。
///
/// - `uuid`: 实例 uuid 字符串
///
/// # 返回值
///
/// uuid 非法返回 `err.uuid`，实例不存在返回 `err.gameNotFound`
pub(crate) fn instance_snapshot(uuid: &str) -> Result<InstanceSettingObj, String> {
    let instance = instance_handle(uuid)?;
    let snapshot = instance.read().unwrap().clone();
    Ok(snapshot)
}
