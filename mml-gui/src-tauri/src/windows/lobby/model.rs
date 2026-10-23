//! 大厅会话随窗口释放，下载、HTTP 请求和账户锁同寿命。

use super::runtime::Session;
use crate::{
    dtos::{LobbyStateDto, LobbyViewDto},
    err_keys,
};
use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use tokio_util::sync::CancellationToken;

pub(crate) struct LobbyModel {
    pub app: AppHandle,
    pub cancel: CancellationToken,
    pub gate: tokio::sync::Mutex<()>,
    pub lease: Mutex<Option<mml_auth::auths::CurrentUserGuard>>,
    pub view: Mutex<LobbyViewDto>,
    pub(super) session: Mutex<Option<Arc<Session>>>,
}

impl LobbyModel {
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

    pub fn check(&self) -> Result<(), String> {
        if self.cancel.is_cancelled() || self.view.lock().unwrap().account_uuid.is_empty() {
            Err(err_keys::LOBBY_CLOSED.into())
        } else {
            Ok(())
        }
    }

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
    fn drop(&mut self) {
        self.close();
    }
}

pub(super) fn get() -> Result<Arc<LobbyModel>, String> {
    crate::windows::lifecycle::model_for_kind("lobby").ok_or_else(|| err_keys::LOBBY_CLOSED.into())
}

/// 只接受当前连接的状态，旧轮询不能更新刚重建的会话。
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

/// 新工作进程的状态序号从零开始，连同前端世代一起重置。
pub(super) fn reset_view(view: &mut LobbyViewDto) {
    view.session_generation += 1;
    view.ready = false;
    view.version.clear();
    view.state = LobbyStateDto {
        state: "stopped".into(),
        ..Default::default()
    };
}
