//! 退出房间不依赖节点列表或房间快照，创建与加入才校验附加参数。

use super::runtime::{QueryParameter, Session};
use crate::{dtos::LobbyViewDto, err_keys};

/// 执行房间操作。
/// - `session`: 当前陶瓦会话。
/// - `view`: 已验证的账户和节点列表。
/// - `action`: host、join 或 leave。
/// - `room`: 加入房间的邀请码。
/// - `nodes`: 创建或加入时选择的节点。
/// # 返回值
/// 控制请求成功或参数、HTTP 错误。
pub(super) async fn perform(
    session: &Session,
    view: &LobbyViewDto,
    action: &str,
    room: &str,
    nodes: &[String],
) -> Result<(), String> {
    if action == "leave" {
        return session.action("/state/ide", &[]).await;
    }
    let state = session.state().await?;
    let path = match action {
        "host" if state.state == "waiting" => "/state/scanning",
        "join" if state.state == "waiting" && !room.trim().is_empty() => "/state/guesting",
        _ => return Err(err_keys::LOBBY_STATE.into()),
    };
    if nodes.len() > 64 || nodes.iter().any(|node| !view.nodes.contains(node)) {
        return Err(err_keys::LOBBY_STATE.into());
    }
    let mut query = vec![QueryParameter {
        name: "player",
        value: view.account.clone(),
    }];
    if action == "join" {
        query.push(QueryParameter {
            name: "room",
            value: room.trim().into(),
        });
    }
    query.extend(nodes.iter().map(|node| QueryParameter {
        name: "public_nodes",
        value: node.clone(),
    }));
    session.action(path, &query).await
}
