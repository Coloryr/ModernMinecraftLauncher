//! 添加实例窗口：窗口级命令与本窗口的模块出口
//!
//! 实例创建走 `mml_game::add_game`，含新建 / 文件夹导入 / 压缩包 / 在线网址，创建成功后
//! 发 instance-change 事件通知主窗口刷新列表。命令按职责拆在 `create` / `loader` / `model`
//! 三个子模块里，本文件只留取消、关闭保护与重名确认答复；模型只保存跟随窗口生命周期的
//! 运行态：取消令牌、关闭保护与重名确认应答通道。

use tauri::WebviewWindow;
// 带命令的子模块必须 `pub(crate)`：生成的 `tauri_commands!` 从 crate 根引用它们
pub(crate) mod create;
pub(crate) mod loader;
pub(crate) mod model;

// 按原路径再导出：窗口生命周期用 `add::AddWindowModel`，下载整合包窗口用 `add::instance_gui`
pub(crate) use self::model::{AddWindowModel, instance_gui};

use self::model::model;

/// 用户对重名确认对话框的答复，用于唤醒等待中的创建流程
///
/// - `window`: 调用方窗口，用于取本窗口模型
/// - `id`: 对话框事件 id，对应后端登记的应答通道
/// - `answer`: 用户是否同意
#[gui_macros::command]
pub fn add_answer_name_conflict(window: WebviewWindow, id: u32, answer: bool) {
    if let Ok(store) = model(&window)
        && let Some(tx) = store.lock().unwrap().take_dialog(id)
    {
        let _ = tx.send(answer);
    }
}

/// 取消进行中的安装任务
///
/// - `window`: 调用方窗口，用于取本窗口模型
///
/// # 返回值
///
/// 确实取消了返回 `true`；没有进行中的任务或模型缺失返回 `false`
#[gui_macros::command]
pub fn add_cancel(window: WebviewWindow) -> bool {
    let Ok(store) = model(&window) else {
        return false;
    };
    match store.lock().unwrap().take_cancel() {
        Some(token) => {
            token.cancel();
            true
        }
        None => false,
    }
}

/// 设置本窗口的关闭保护：查询数据期间开启，`CloseRequested` 阶段拒绝关闭
///
/// - `window`: 调用方窗口，用于取本窗口模型
/// - `enabled`: 是否开启关闭保护
#[gui_macros::command]
pub fn add_set_close_guard(window: WebviewWindow, enabled: bool) {
    if let Ok(store) = model(&window) {
        store.lock().unwrap().set_close_guard(enabled);
    }
}
