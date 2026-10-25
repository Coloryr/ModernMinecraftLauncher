//! 窗口事件
//!
//! 关窗请求在这里统一处理：拦下要阻止的关闭，其余走到销毁流程。

use std::sync::Mutex;

use tauri::{Manager, WebviewWindow};

use uuid::Uuid;

use super::create::{close_guarded, emit_close_blocked, notify_modpack_window};
use super::models::{OPEN_WINDOWS, model_for_kind, remove_window_model};
use crate::windows::geometry::save_window_state;
use crate::windows::registry::{
    ADD_MODPACK_WINDOW_UUID, MAIN_WINDOW_UUID, family_of_label, uuid_of_label,
};
use crate::windows::{add_modpack, resource};

/// 窗口事件处理，注册于 `Builder::on_window_event`
///
/// 原生标题栏 X、JS API 直接 `close()` 都不经过 `window_close_window` 命令，
/// 在 `CloseRequested` 阶段统一保存几何，此时窗口仍存活、位置有效；
/// `Destroyed` 阶段清理句柄表并销毁跟随窗口的模型。
///
/// - `window`: 触发事件的窗口
/// - `event`: 窗口事件，只处理关闭请求 / 销毁 / 缩放与移动
pub fn on_window_event(window: &tauri::Window<tauri::Wry>, event: &tauri::WindowEvent) {
    let label = window.label().to_string();
    // uuid 按标签反查：静态窗口走注册表，日志窗口族按 `mml-log-<实例uuid>` 派生
    let Some(uuid) = uuid_of_label(&label) else {
        return;
    };
    match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            // 窗口模型开启关闭保护时，如正在查询数据，拒绝本次关闭请求并通知前端提示
            if close_guarded(&uuid) {
                api.prevent_close();
                emit_close_blocked(window);
                return;
            }
            // 关闭已放行：终止该窗口还在跑的操作 —— 整合包搜索、模组扫描 / 更新检查 / 下载更新
            add_modpack::cancel_search(&label);
            resource::cancel_window(&label);
            if label == "mml-lobby" {
                crate::windows::lobby::cancel();
            }
            // 下载整合包窗口关掉：那两个进度指示器要回到主页面，广播一次让 `window_open` 变 false
            if uuid == ADD_MODPACK_WINDOW_UUID {
                notify_modpack_window(window.app_handle());
            }
            if let Some(win) = window.app_handle().get_webview_window(&label) {
                // 最大化 / 全屏：只记标志、不覆盖几何，否则下次会开在另一块屏、而且不最大化
                let maximized =
                    win.is_maximized().unwrap_or(false) || win.is_fullscreen().unwrap_or(false);
                let _ = save_window_state(&uuid, &win, maximized);
            }
            if uuid == MAIN_WINDOW_UUID {
                // 有实例在跑：退回后台，托盘可见，核心与统计继续 —— 运行中的游戏还要接着计时。
                // 没有实例在跑：后台没有留下的理由，关窗就是退出应用，先让核心与统计落盘。
                if !crate::tray::has_running_instance() {
                    mml_core::stop();
                    mml_game::game_count::stop();
                }
                let others: Vec<(Uuid, WebviewWindow<tauri::Wry>)> = OPEN_WINDOWS
                    .write()
                    .unwrap()
                    .iter()
                    .filter(|(u, _)| **u != MAIN_WINDOW_UUID)
                    .map(|(u, w)| (*u, w.clone()))
                    .collect();
                for (other_uuid, win) in others {
                    // 主窗口退出是最终退出：destroy 强制关闭，绕过各窗口的关闭保护。但 destroy
                    // **不发 CloseRequested**，那条链路的收尾不会执行 —— 这里手动补上取消搜索、
                    // 记下最大化标志与几何，否则下次开窗既不是最大化、几何也停在旧值。
                    add_modpack::cancel_search(win.label());
                    resource::cancel_window(win.label());
                    let maximized =
                        win.is_maximized().unwrap_or(false) || win.is_fullscreen().unwrap_or(false);
                    let _ = save_window_state(&other_uuid, &win, maximized);
                    let _ = win.destroy();
                }
            }
        }
        // 窗口已销毁：清理句柄与跟随窗口的模型，下次开窗重新创建
        tauri::WindowEvent::Destroyed => {
            // 关窗时把还在等应答的重名确认对话框按"拒绝"收尾：用户没答就关窗的话，
            // 安装 future 会永久 pending、任务卡在"进行中"，发送端也一直留在表里
            if let Some(kind) = label.strip_prefix("mml-")
                && let Some(model) =
                    model_for_kind::<Mutex<crate::windows::add::AddWindowModel>>(kind)
            {
                model.lock().unwrap().reject_all_dialogs();
            }
            // 配置窗口关了，它打开过的 NBT 文件也该放手：句柄不能留到下次开窗。
            // 配置窗口是一个实例一个，只清本实例那一份 —— 别的实例的编辑器还开着
            if let Some(hit) = family_of_label(&label)
                && hit.family.kind == "config"
            {
                crate::windows::nbt_session::close_all_of(&hit.instance);
            }
            // 模型这就被移出表了，先把它里面在跑的任务取消掉：
            // 令牌被丢弃**不会**触发取消，任务会一直跑到自己结束
            resource::cancel_window(&label);
            OPEN_WINDOWS.write().unwrap().remove(&uuid);
            remove_window_model(&uuid);
            // 主界面关掉了：若此刻已经没有实例在跑，后台没有留下的理由。
            // 与"最后一个实例退出"两处互相兜底，谁后发生谁收尾，见 tray::quit_idle
            if uuid == MAIN_WINDOW_UUID && mml_core::get_state() {
                crate::tray::quit_idle(window.app_handle());
            }
        }
        // 缩放 / 移动：实时记录几何，不必等关窗，配置保存按文件去重合并写入
        // 最大化 / 全屏时不记，否则还原后会以最大化尺寸打开
        tauri::WindowEvent::Resized(_) | tauri::WindowEvent::Moved(_) => {
            if let Some(win) = window.app_handle().get_webview_window(&label) {
                if win.is_maximized().unwrap_or(false) || win.is_fullscreen().unwrap_or(false) {
                    return;
                }
                // 这里已确定不是最大化 / 全屏；取消最大化时也会走到这，顺手把标志清掉
                let _ = save_window_state(&uuid, &win, false);
            }
        }
        _ => {}
    }
}
