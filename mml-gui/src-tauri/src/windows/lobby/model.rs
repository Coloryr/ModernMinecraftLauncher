//! 大厅会话随窗口释放，下载、HTTP 请求和账户锁同寿命。

use super::runtime::Session;
use crate::{
    dtos::{LobbyStateDto, LobbyViewDto},
    err_keys,
};
use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use tokio_util::sync::CancellationToken;

/// 大厅窗口的模型：账户锁、陶瓦会话与最近一次视图快照
pub(crate) struct LobbyModel {
    /// 应用句柄，账户锁状态变更经它广播
    pub app: AppHandle,
    /// 窗口关闭时取消，下载与 HTTP 请求都靠它收手
    pub cancel: CancellationToken,
    /// 串行化窗口内的操作，准备账户、启动陶瓦、房间操作与启动游戏互斥
    pub gate: tokio::sync::Mutex<()>,
    /// 本次窗口持有的账户锁，窗口关闭或验证失败时释放
    pub lease: Mutex<Option<mml_auth::auths::CurrentUserGuard>>,
    /// 最近一次视图快照，命令与轮询都读写它
    pub view: Mutex<LobbyViewDto>,
    /// 当前陶瓦会话，未启动时为 `None`
    pub(super) session: Mutex<Option<Arc<Session>>>,
}

impl LobbyModel {
    /// 建一个还没验证账户、也没启动陶瓦的模型
    ///
    /// - `app`: 应用句柄
    ///
    /// # 返回值
    ///
    /// 空会话、空账户锁的模型
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            cancel: CancellationToken::new(),
            gate: tokio::sync::Mutex::new(()),
            lease: Mutex::new(None),
            view: Mutex::new(LobbyViewDto::default()),
            session: Mutex::new(None),
        }
    }

    /// 窗口是否还能用：已取消或还没验证账户都算关闭
    ///
    /// # 返回值
    ///
    /// 可用返回 `Ok(())`，否则返回 `err.lobbyClosed`
    pub fn check(&self) -> Result<(), String> {
        if self.cancel.is_cancelled() || self.view.lock().unwrap().account_uuid.is_empty() {
            Err(err_keys::LOBBY_CLOSED.into())
        } else {
            Ok(())
        }
    }

    /// 取消窗口任务、释放账户锁并丢掉会话，可重复调用
    pub fn close(&self) {
        if self.cancel.is_cancelled() {
            return;
        }
        self.cancel.cancel();
        self.lease.lock().unwrap().take();
        crate::windows::account::emit_account_change(&self.app);
        self.session.lock().unwrap().take();
    }
}

impl Drop for LobbyModel {
    /// 模型释放时按关窗处理，别把账户锁和陶瓦进程漏出去
    fn drop(&mut self) {
        self.close();
    }
}

/// 取大厅窗口的模型
///
/// # 返回值
///
/// 模型句柄；窗口还没建或不在了返回 `err.lobbyClosed`
pub(super) fn get() -> Result<Arc<LobbyModel>, String> {
    crate::windows::lifecycle::model_for_kind("lobby").ok_or_else(|| err_keys::LOBBY_CLOSED.into())
}

/// 只接受当前连接的状态，旧轮询不能更新刚重建的会话
///
/// - `current`: 模型当前持有的会话，可能是换过的那一个
/// - `source`: 本次状态来自哪个会话
/// - `view`: 待更新的视图
/// - `state`: 本次拉到的状态
pub(super) fn accept_state(
    current: Option<&Arc<Session>>,
    source: &Arc<Session>,
    view: &mut LobbyViewDto,
    state: LobbyStateDto,
) {
    if current.is_some_and(|current| Arc::ptr_eq(current, source))
        && state.index >= view.state.index
    {
        view.state = state;
    }
}

/// 新工作进程的状态序号从零开始，连同前端世代一起重置
///
/// - `view`: 待重置的视图
pub(super) fn reset_view(view: &mut LobbyViewDto) {
    view.session_generation += 1;
    view.ready = false;
    view.version.clear();
    view.state = LobbyStateDto {
        state: "stopped".into(),
        ..Default::default()
    };
}
