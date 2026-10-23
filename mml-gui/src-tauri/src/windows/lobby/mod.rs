//! 正版账户验证、映射大厅预留接口及陶瓦联机操作。

mod install;
pub(crate) mod model;
mod room;
mod runtime;

use crate::{
    dtos::{LobbyViewDto, MappingLobbyDto},
    err_keys,
};
use model::LobbyModel;
use std::{sync::Arc, time::Duration};
use tauri::AppHandle;

/// 验证并锁定当前正版账户，成功后才允许显示大厅。
/// - `app`: 广播账户锁状态的应用句柄。
/// # 返回值
/// 已验证的大厅视图，失败时释放本次账户锁。
#[gui_macros::command]
pub async fn lobby_prepare(app: AppHandle) -> Result<LobbyViewDto, String> {
    let id = crate::windows::registry::uuid_for_kind("lobby").unwrap();
    let model =
        crate::windows::lifecycle::window_model_or_insert(&id, || LobbyModel::new(app.clone()))
            .ok_or(err_keys::MODEL_MISSING)?;
    let _operation = model.gate.lock().await;
    if model.cancel.is_cancelled() {
        return Err(err_keys::LOBBY_CLOSED.into());
    }
    let view = model.view.lock().unwrap().clone();
    if !view.account_uuid.is_empty() {
        return Ok(view);
    }
    let lease = mml_auth::auths::lock_current().ok_or(err_keys::LOBBY_ACCOUNT)?;
    if lease.key.auth_type != mml_auth::AuthType::OAuth {
        return Err(err_keys::LOBBY_ACCOUNT.into());
    }
    let mut account = mml_auth::auths::get(&lease.key.uuid, lease.key.auth_type)
        .ok_or(err_keys::LOBBY_ACCOUNT)?;
    let key = lease.key.clone();
    *model.lease.lock().unwrap() = Some(lease);
    crate::windows::account::emit_account_change(&app);
    let verified = tokio::select! {
        _ = model.cancel.cancelled() => Err(err_keys::LOBBY_CLOSED.to_string()),
        result = tokio::time::timeout(Duration::from_secs(60), account.refresh(model.cancel.clone())) =>
            result.map_err(|_| err_keys::LOBBY_ACCOUNT.to_string()).and_then(|r| r.map_err(|e| e.to_string())),
    };
    if let Err(error) = verified {
        model.lease.lock().unwrap().take();
        crate::windows::account::emit_account_change(&app);
        return Err(error);
    }
    if model.cancel.is_cancelled() || account.get_key() != key {
        model.lease.lock().unwrap().take();
        crate::windows::account::emit_account_change(&app);
        return Err(err_keys::LOBBY_ACCOUNT.into());
    }
    account.save();
    let mut view = model.view.lock().unwrap();
    view.account = account.user_name;
    view.account_uuid = key.uuid;
    view.state.state = "stopped".into();
    Ok(view.clone())
}

/// 获取大厅快照及陶瓦房间状态。
/// # 返回值
/// 当前房间、成员、节点及下载进度。
#[gui_macros::command]
pub async fn lobby_status() -> Result<LobbyViewDto, String> {
    let model = model::get()?;
    model.check()?;
    let session = model.session.lock().unwrap().clone();
    if let Some(session) = session {
        let state = tokio::select! {
            _ = model.cancel.cancelled() => return Err(err_keys::LOBBY_CLOSED.into()),
            state = session.state() => state?,
        };
        let mut view = model.view.lock().unwrap();
        if state.index >= view.state.index {
            view.state = state;
        }
    }
    let view = model.view.lock().unwrap().clone();
    Ok(view)
}

/// 下载并启动陶瓦，读取版本和公共节点。
/// # 返回值
/// 陶瓦就绪后的大厅视图。
#[gui_macros::command]
pub async fn lobby_start() -> Result<LobbyViewDto, String> {
    let model = model::get()?;
    let _operation = model.gate.lock().await;
    model.check()?;
    if model.session.lock().unwrap().is_none() {
        let (session, version) = runtime::Session::start(&model).await?;
        {
            let mut current = model.session.lock().unwrap();
            model.check()?;
            *current = Some(session);
        }
        let mut view = model.view.lock().unwrap();
        view.version = version;
        view.ready = true;
        view.state.index = 0;
    }
    let nodes = tokio::select! {
        _ = model.cancel.cancelled() => return Err(err_keys::LOBBY_CLOSED.into()),
        nodes = runtime::nodes() => nodes,
    };
    model.view.lock().unwrap().nodes = nodes;
    lobby_status().await
}

fn session(model: &Arc<LobbyModel>) -> Result<Arc<runtime::Session>, String> {
    model.check()?;
    model
        .session
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| err_keys::LOBBY_START.into())
}

/// 创建或加入陶瓦房间。
/// - `action`: host 创建，join 加入，leave 退出。
/// - `room`: 加入房间的邀请码。
/// - `nodes`: 从当前节点列表选择的附加节点。
/// # 返回值
/// 控制请求成功或状态错误。
#[gui_macros::command]
pub async fn lobby_room_action(
    action: String,
    room: String,
    nodes: Vec<String>,
) -> Result<(), String> {
    let model = model::get()?;
    let _operation = model.gate.lock().await;
    let session = session(&model)?;
    let view = model.view.lock().unwrap().clone();
    tokio::select! {
        _ = model.cancel.cancelled() => Err(err_keys::LOBBY_CLOSED.into()),
        result = room::perform(&session, &view, &action, &room, &nodes) => result,
    }
}

/// 使用大厅锁定的账户启动所选实例，加入方自动连接本机转发端口。
/// - `app`: 游戏启动事件的应用句柄。
/// - `uuid`: 用户选择的实例。
/// # 返回值
/// 游戏启动已提交或参数、房间状态错误。
#[gui_macros::command]
pub async fn lobby_launch_game(app: AppHandle, uuid: String) -> Result<(), String> {
    let model = model::get()?;
    let _operation = model.gate.lock().await;
    let session = session(&model)?;
    let state = tokio::select! {
        _ = model.cancel.cancelled() => return Err(err_keys::LOBBY_CLOSED.into()),
        result = session.state() => result?,
    };
    let auto = match state.state.as_str() {
        "guest-ok" => mml_game::game_launch::AutoJoinType::Server(Arc::new(runtime::join_server(
            state.address.as_deref().ok_or(err_keys::LOBBY_STATE)?,
        )?)),
        "waiting" | "host-scanning" | "host-starting" | "host-ok" => {
            mml_game::game_launch::AutoJoinType::None
        }
        _ => return Err(err_keys::LOBBY_STATE.into()),
    };
    model.check()?;
    let key = model
        .lease
        .lock()
        .unwrap()
        .as_ref()
        .ok_or(err_keys::LOBBY_CLOSED)?
        .key
        .clone();
    crate::windows::main::launch::launch_game(app, uuid, None, None, Some(false), Some((key, auto)))
        .await
}

/// 释放大厅会话，单窗口页面离开时调用。
#[gui_macros::command]
pub fn lobby_close() {
    let _ = crate::windows::lifecycle::drop_model_for_kind("lobby");
}

/// 映射大厅预留查询接口，后端服务尚未接入。
/// # 返回值
/// 不可用状态与空房间列表。
#[gui_macros::command]
pub fn lobby_mapping_list() -> Result<MappingLobbyDto, String> {
    model::get()?.check()?;
    Ok(MappingLobbyDto {
        available: false,
        rooms: Vec::new(),
    })
}

/// 映射大厅预留操作接口。
/// - `action`: 创建或加入操作。
/// - `room`: 房间标识。
/// # 返回值
/// 后端尚未接入的提示。
#[gui_macros::command]
pub fn lobby_mapping_action(action: String, room: String) -> Result<(), String> {
    model::get()?.check()?;
    let _ = (action, room);
    Err(err_keys::LOBBY_MAPPING.into())
}

/// 关闭真实窗口前先取消任务并释放账户。
pub(crate) fn cancel() {
    if let Ok(model) = model::get() {
        model.close();
    }
}
