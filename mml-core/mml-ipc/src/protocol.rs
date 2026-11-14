use crate::*;

pub(crate) fn message_len(buf: &[u8]) -> CoreResult<Option<usize>> {
    if buf.len() < 4 {
        return Ok(None);
    }
    let msg_type = i32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
    let mut offset = 4usize;

    /// 在消息限额内推进偏移，数据不足时返回 false。
    fn advance(buf: &[u8], offset: &mut usize, len: usize) -> CoreResult<bool> {
        let end = offset
            .checked_add(len)
            .ok_or(IpcFrameTooLarge { len: usize::MAX })?;
        if end > MAX_FRAME_SIZE {
            return Err(IpcFrameTooLarge { len: end });
        }
        if end > buf.len() {
            return Ok(false);
        }
        *offset = end;
        Ok(true)
    }

    /// 拒绝负数字符串长度，再检查内容是否完整。
    fn string_field(buf: &[u8], offset: &mut usize) -> CoreResult<bool> {
        if !advance(buf, offset, 4)? {
            return Ok(false);
        }
        let len = i32::from_be_bytes(buf[*offset - 4..*offset].try_into().unwrap());
        let len = usize::try_from(len).map_err(|_| IpcInvalidFrame)?;
        advance(buf, offset, len)
    }

    match msg_type {
        TYPE_LAUNCH_SHOW => {}
        TYPE_GAME_MOUSE_STATE => {
            if !string_field(buf, &mut offset)? || !advance(buf, &mut offset, 1)? {
                return Ok(None);
            }
        }
        TYPE_LAUNCH_ARG => {
            if !advance(buf, &mut offset, 4)? {
                return Ok(None);
            }
            let count = i32::from_be_bytes(buf[offset - 4..offset].try_into().unwrap());
            let count = usize::try_from(count).map_err(|_| IpcInvalidFrame)?;
            // 每个字符串至少占四字节长度头，提前拒绝不可能容纳的列表。
            if count > (MAX_FRAME_SIZE - offset) / 4 {
                return Err(IpcFrameTooLarge {
                    len: offset.saturating_add(count.saturating_mul(4)),
                });
            }
            for _ in 0..count {
                if !string_field(buf, &mut offset)? {
                    return Ok(None);
                }
            }
        }
        TYPE_GAME_CHANNEL | TYPE_GAME_PROOF | TYPE_GAME_CHALLENGE => {
            if !string_field(buf, &mut offset)? {
                return Ok(None);
            }
        }
        TYPE_GAME_WINDOW_SIZE => {
            if !string_field(buf, &mut offset)? || !advance(buf, &mut offset, 8)? {
                return Ok(None);
            }
        }
        TYPE_GAME_WINDOW_FOCUS => {
            if !string_field(buf, &mut offset)? || !advance(buf, &mut offset, 1)? {
                return Ok(None);
            }
        }
        _ => {
            mml_log::warn_type(InfoType::IpcUnknownMessageType { msg_type });
            return Ok(Some(4));
        }
    }

    Ok(Some(offset))
}

/// 处理一条消息
///
/// - `data`: 消息体，不含类型头，读完即消费
/// - `client_tx`: 该连接的出站通道，通过验证后用于注册广播与游戏通道
/// - `state`: 该连接的握手状态，应答核对通过后就地置为已验证
///
/// # 返回值
///
/// 应答不匹配、或未验证就发游戏消息时返回错误，调用方据此断开连接
pub(crate) async fn process_message(
    data: &mut BytesMut,
    client_tx: &mpsc::UnboundedSender<BytesMut>,
    state: &mut ClientState,
) -> CoreResult<()> {
    if data.len() < 4 {
        return Ok(());
    }
    let msg_type = data.get_i32();

    // 游戏侧消息只能来自已验证的连接；启动器之间的「启动显示 / 启动参数」不要求验证
    match msg_type {
        TYPE_GAME_MOUSE_STATE | TYPE_GAME_CHANNEL | TYPE_GAME_WINDOW_SIZE | TYPE_GAME_WINDOW_FOCUS
            if !state.authenticated =>
        {
            return Err(IpcGameMessageBeforeAuth { msg_type });
        }
        _ => {}
    }

    match msg_type {
        TYPE_GAME_PROOF => {
            // 重复应答直接忽略，否则已通过验证的连接可以被改回未验证
            if state.authenticated {
                return Ok(());
            }
            let proof = data.read_string();
            let key = AUTH_KEY.get().map(String::as_str).unwrap_or_default();
            if !auth::verify(&auth::proof(key, &state.nonce), &proof) {
                return Err(IpcGameAuthFailed);
            }
            state.authenticated = true;

            // 先回「验证结果」再进广播列表：同一通道按入队顺序发送，
            // 这样 MOTD 广播不会插在验证结果前面，游戏端不会把它当成握手响应
            let mut result = BytesMut::new();
            result.put_i32(TYPE_GAME_AUTH_RESULT);
            let _ = client_tx.send(result);

            // 通过验证才进广播列表，未验证的连接收不到 MOTD
            broadcast::add_client(client_tx);
        }
        TYPE_GAME_MOUSE_STATE => {
            let uuid_str = data.read_string();
            let guid = Uuid::parse_str(&uuid_str)
                .map_err(|err| SocketError(ErrorData::from_error(err)))?;
            let value = data.read_bool();
            MOUSE_STATES.write().unwrap().insert(guid, value);
        }
        TYPE_LAUNCH_SHOW => {
            mml_log::info_type(InfoType::IpcLauncherShowWindow);
            IPC_EVENT.emit();
        }
        TYPE_LAUNCH_ARG => {
            let args = data.read_string_list();
            let mut write = RUN_ARG.write().unwrap();
            write.clear();
            write.extend(args.clone());
            IPC_ARG_EVENT.emit(args);
        }
        TYPE_GAME_CHANNEL => {
            let uuid_str = data.read_string();
            let guid = Uuid::parse_str(&uuid_str)
                .map_err(|err| SocketError(ErrorData::from_error(err)))?;
            GAME_CHANNELS
                .write()
                .unwrap()
                .insert(guid, client_tx.clone());
            mml_log::info_type(InfoType::IpcGameChannelBound {
                guid: guid.to_string(),
            });
        }
        TYPE_GAME_WINDOW_SIZE => {
            let uuid_str = data.read_string();
            let guid = Uuid::parse_str(&uuid_str)
                .map_err(|err| SocketError(ErrorData::from_error(err)))?;
            let width = data.get_i32();
            let height = data.get_i32();
            WINDOW_SIZES
                .write()
                .unwrap()
                .insert(guid, WindowSize { width, height });
        }
        TYPE_GAME_WINDOW_FOCUS => {
            let uuid_str = data.read_string();
            let guid = Uuid::parse_str(&uuid_str)
                .map_err(|err| SocketError(ErrorData::from_error(err)))?;
            let value = data.read_bool();
            FOCUS_STATES.write().unwrap().insert(guid, value);
        }
        _ => {
            mml_log::warn_type(InfoType::IpcUnknownMessageType { msg_type });
        }
    }

    Ok(())
}
