//! 大厅会话随窗口释放，下载、HTTP 请求和账户锁同寿命。

use super::runtime::Session;
use crate::{dtos::LobbyViewDto, err_keys};
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
