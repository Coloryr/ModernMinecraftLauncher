use crate::*;

pub(crate) async fn broadcast_servers() {
    let mut interval = time::interval(Duration::from_secs(2));
    loop {
        interval.tick().await;

        // 复制服务器列表，避免长期持有锁
        let servers = SERVER_INFOS.read().unwrap().clone();

        if servers.is_empty() {
            continue;
        }

        for server in servers {
            let mut content = BytesMut::new();
            content.put_i32(TYPE_GAME_MOTD);

            content.write_string(&server.ip);
            content.write_string(&server.port);
            content.write_string(&server.motd);

            broadcast(content);
        }
    }
}

/// 把通过验证的连接加入广播列表
///
/// - `tx`: 该连接的出站通道
pub(crate) fn add_client(tx: &TcpChannel) {
    cleanup();
    CLIENTS.write().unwrap().push(tx.clone());
}

/// 清理已关闭的客户端通道
pub(crate) fn cleanup() {
    CLIENTS.write().unwrap().retain(|tx| !tx.is_closed());
    let mut channels = GAME_CHANNELS.write().unwrap();
    channels.retain(|_uuid, tx| !tx.is_closed());
    // 通道没了，它上报的鼠标状态也就不作数：留着会让下次启动的同一个实例
    // 一上来就顶着上一局的光标 / 抓取模式，而手柄模式是按这个状态切的
    MOUSE_STATES
        .write()
        .unwrap()
        .retain(|uuid, _| channels.contains_key(uuid));
}

/// 广播消息给所有客户端
///
/// - `msg`: 完整消息，含类型头与消息体
pub(crate) fn broadcast(msg: BytesMut) {
    cleanup();
    for tx in CLIENTS.read().unwrap().iter() {
        let _ = tx.send(msg.clone());
    }
}

/// 发送消息给指定游戏实例
///
/// - `uuid`: 游戏实例 ID
/// - `msg`: 完整消息，含类型头与消息体
///
/// # 返回值
///
/// 发送成功返回 `true`，实例未注册通道或通道已关闭时返回 `false`
pub(crate) fn send_to_game(uuid: Uuid, msg: BytesMut) -> bool {
    cleanup();
    if let Some(tx) = GAME_CHANNELS.read().unwrap().get(&uuid) {
        return tx.send(msg).is_ok();
    }
    false
}
