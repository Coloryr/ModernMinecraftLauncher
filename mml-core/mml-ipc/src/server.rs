use crate::*;

use crate::broadcast::broadcast_servers;
use crate::protocol::{message_len, process_message};

pub(crate) fn run_server(port_tx: std::sync::mpsc::Sender<u16>) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(err) => {
            mml_log::error_type(IpcRuntimeCreateError(ErrorData::from_error(err)));
            return;
        }
    };

    runtime.block_on(async {
        let listener = match tokio::net::TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(err) => {
                mml_log::error_type(IpcBindError(ErrorData::from_error(err)));
                return;
            }
        };

        // 端口对外之前先生成密钥，避免刚连上的客户端拿不到验证帧
        let _ = AUTH_KEY.set(Uuid::new_v4().to_string());

        // 回传绑定端口
        let port = listener.local_addr().map(|addr| addr.port()).unwrap_or(0);
        let _ = port_tx.send(port);

        let _ = tokio::spawn(accept_loop(listener));
        let _ = tokio::spawn(broadcast_servers());
        std::future::pending::<()>().await;
    });
}

/// 接受客户端连接，为每个连接启动处理任务
pub(crate) async fn accept_loop(listener: TcpListener) -> CoreResult<()> {
    loop {
        let (stream, addr) = match listener.accept().await {
            Ok(conn) => conn,
            Err(err) => {
                mml_log::error_type(IpcAcceptError(ErrorData::from_error(err)));
                continue;
            }
        };

        mml_log::info_type(InfoType::IpcClientConnected {
            addr: addr.to_string(),
        });
        tokio::spawn(handle_client(stream));
    }
}

/// 处理单个客户端连接，含读取循环与写入任务
///
/// 每个连接分配一条出站通道，写入任务负责将通道中的消息发送到 TCP；
/// 该通道同时注册进 [`CLIENTS`] 用于广播，并注册为游戏通道用于定向发送。
pub(crate) async fn handle_client(stream: TcpStream) -> CoreResult<()> {
    let (mut reader, mut writer) = tokio::io::split(stream);
    let (write_tx, mut write_rx) = mpsc::unbounded_channel::<BytesMut>();

    // 先发连接验证，客户端收到的第一条消息必须是密钥
    if let Some(key) = AUTH_KEY.get() {
        let mut auth = BytesMut::new();
        auth.put_i32(TYPE_GAME_AUTH);
        auth.write_string(key);
        let _ = write_tx.send(auth);
    }

    // 注册客户端到广播列表
    CLIENTS.write().unwrap().push(write_tx.clone());

    // 写入任务：从通道接收数据并发送到 TCP
    let write_handle = tokio::spawn(async move {
        while let Some(msg) = write_rx.recv().await {
            if let Err(err) = writer.write_all(&msg).await {
                mml_log::error_type(IpcWriteError(ErrorData::from_error(err)));
                break;
            }
        }
    });

    // 读取循环：消息无总长度前缀，按类型自定界解析，与 ColorMC 一致
    let mut buf = BytesMut::with_capacity(1024);
    loop {
        let msg_len = match message_len(&buf) {
            Ok(length) => length,
            Err(err) => {
                mml_log::error_type(err);
                break;
            }
        };
        if let Some(msg_len) = msg_len {
            let mut msg = buf.split_to(msg_len);
            if let Err(err) = process_message(&mut msg, &write_tx).await {
                mml_log::error_type(err);
                break;
            }
        } else {
            // 缓冲区内数据不足一条完整消息：继续读取；
            // 缓冲区异常膨胀仍无法解析说明流失步/坏数据，断开连接
            if buf.len() > MAX_FRAME_SIZE {
                mml_log::error_type(IpcBufferOverrun);
                break;
            }
            let n = reader
                .read_buf(&mut buf)
                .await
                .map_err(|err| SocketError(ErrorData::from_error(err)))?;
            if n == 0 {
                break;
            }
        }
    }

    // 连接关闭，停止写入任务；通道清理由 cleanup() 惰性处理
    write_handle.abort();
    Ok(())
}
