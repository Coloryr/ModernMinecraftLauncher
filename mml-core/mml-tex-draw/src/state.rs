//! 渲染状态
//!
//! 防重入标志、取消令牌与进度回调。
use std::sync::{
    LazyLock, OnceLock, RwLock,
    atomic::{AtomicBool, Ordering},
};

use mml_game::gui_hook;
use tokio_util::sync::CancellationToken;

/// 渲染防重入标志：load_blocks 可能被并发触发，界面按钮连点或界面与内核各起一轮，
/// 不挡住的话会各自下发一次客户端 jar 下载任务，同一文件下两遍
pub(crate) static LOADING: AtomicBool = AtomicBool::new(false);

/// 本轮渲染的取消令牌，每轮 load_blocks 开始时换新，取消不会串到下一轮
///
/// 同时用于渲染循环按条目查询与本轮下发的下载任务，取消只影响本轮，
/// 不动下载队列里其它任务如整合包安装
pub(crate) static CANCEL: LazyLock<RwLock<CancellationToken>> =
    LazyLock::new(|| RwLock::new(CancellationToken::new()));

/// 全局渲染进度回调，GUI 启动时注册；调用 [`load_blocks`] 时没传回调的
/// 调用方经它上报进度，界面按钮那条路径自带回调，这里只是兜底
pub(crate) static GUI: OnceLock<gui_hook::ProgressGui> = OnceLock::new();

/// 注册全局渲染进度回调，启动时调用一次
///
/// - `gui`: 全局进度回调，重复注册会被忽略
pub fn set_gui_handel(gui: gui_hook::ProgressGui) {
    let _ = GUI.set(gui);
}

/// 是否正在渲染，供界面层防重入查询
///
/// # 返回值
///
/// 正在渲染时返回 `true`
pub fn is_loading() -> bool {
    LOADING.load(Ordering::Acquire)
}

/// 取消本轮渲染
///
/// 渲染循环与在途下载都是协作式取消：置位后循环跳过剩余条目、下载在下一个数据块
/// 处收手，`load_blocks` 返回 [`ErrorType::TaskCancel`]；也挂在核心停止链上
pub fn cancel() {
    CANCEL.read().unwrap().cancel();
}

/// 本轮渲染是否已被取消
///
/// # 返回值
///
/// 本轮已请求取消时返回 `true`
pub fn is_cancelled() -> bool {
    CANCEL.read().unwrap().is_cancelled()
}

/// 取本轮取消令牌，渲染循环持有克隆，按条目无锁查询
pub(crate) fn cancel_token() -> CancellationToken {
    CANCEL.read().unwrap().clone()
}
