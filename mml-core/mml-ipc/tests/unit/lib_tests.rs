use super::*;
use crate::byte_buf::ByteBufExt;
use std::sync::OnceLock;

/// 日志系统只能启动一次；协议解析路径会调用 mml_log::*，
/// 未初始化时其内部 `SEM.get().unwrap()` 会 panic，故先启动。
static LOG_STARTED: OnceLock<()> = OnceLock::new();

/// 幂等地启动日志系统，首次调用时在临时目录建日志
fn ensure_log() {
    LOG_STARTED.get_or_init(|| {
        let dir = mml_testutil::temp_dir().join(format!("mml_ipc_unit_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        mml_log::start(dir).unwrap();
    });
}

/// 构造「类型 + 字符串」消息
fn msg_with_string(msg_type: i32, s: &str) -> BytesMut {
    let mut buf = BytesMut::new();
    buf.put_i32(msg_type);
    buf.write_string(s);
    buf
}

/// ---- message_len：消息边界推算 ----

/// 无消息体的「启动显示」消息恰为 4 字节
#[test]
fn test_message_len_launch_show() {
    let mut buf = BytesMut::new();
    buf.put_i32(TYPE_LAUNCH_SHOW);
    assert_eq!(message_len(&buf).unwrap(), Some(4));
}

/// 数据不足 4 字节，类型头不完整时应返回 None
#[test]
fn test_message_len_too_short() {
    let buf = BytesMut::from(&[0u8, 0, 0][..]);
    assert_eq!(message_len(&buf).unwrap(), None);
}

/// 鼠标状态消息：类型 + uuid 字符串 + 1 字节布尔
#[test]
fn test_message_len_mouse_state() {
    let uuid = Uuid::new_v4().to_string();
    let mut buf = BytesMut::new();
    buf.put_i32(TYPE_GAME_MOUSE_STATE);
    buf.write_string(&uuid);
    buf.put_u8(1);
    let full = buf.clone();

    assert_eq!(message_len(&buf).unwrap(), Some(full.len()));

    // 去掉末尾布尔字节后应判定为消息不完整
    buf.truncate(full.len() - 1);
    assert_eq!(message_len(&buf).unwrap(), None);

    // 去掉字符串内容的一部分同样不完整
    buf.truncate(full.len() - 3);
    assert_eq!(message_len(&buf).unwrap(), None);
}

/// 启动参数消息：类型 + 数量 + N 个字符串
#[test]
fn test_message_len_launch_arg() {
    let mut buf = BytesMut::new();
    buf.put_i32(TYPE_LAUNCH_ARG);
    buf.write_string_list(&["--open".to_string(), "abc".to_string()]);
    assert_eq!(message_len(&buf).unwrap(), Some(buf.len()));

    // 数量声明为 2 但只有 1 个字符串：不完整
    let mut bad = buf.clone();
    bad.truncate(bad.len() - 3);
    assert_eq!(message_len(&bad).unwrap(), None);
}

/// 游戏通道消息：类型 + uuid 字符串
#[test]
fn test_message_len_game_channel() {
    let buf = msg_with_string(TYPE_GAME_CHANNEL, &Uuid::new_v4().to_string());
    assert_eq!(message_len(&buf).unwrap(), Some(buf.len()));
}

/// 窗口大小消息：类型 + uuid 字符串 + 宽高各 4 字节
#[test]
fn test_message_len_window_size() {
    let mut buf = BytesMut::new();
    buf.put_i32(TYPE_GAME_WINDOW_SIZE);
    buf.write_string(&Uuid::new_v4().to_string());
    buf.put_i32(1920);
    buf.put_i32(1080);
    assert_eq!(message_len(&buf).unwrap(), Some(buf.len()));

    // 缺少高字段时不完整
    let mut bad = buf.clone();
    bad.truncate(bad.len() - 2);
    assert_eq!(message_len(&bad).unwrap(), None);
}

/// 未知类型无法确定边界，按仅含类型头的 4 字节处理，文档化现有行为
#[test]
fn test_message_len_unknown_type() {
    let mut buf = BytesMut::new();
    buf.put_i32(99);
    buf.write_string("extra");
    ensure_log();
    assert_eq!(message_len(&buf).unwrap(), Some(4));
}

/// 同一缓冲区内连续两条消息的边界推算
#[test]
fn test_message_len_two_frames() {
    let mut buf = BytesMut::new();
    buf.put_i32(TYPE_LAUNCH_SHOW);
    // 追加第二条消息
    let second = msg_with_string(TYPE_GAME_CHANNEL, &Uuid::new_v4().to_string());
    buf.extend_from_slice(&second);

    // 应先推算出第一条消息，4 字节
    assert_eq!(message_len(&buf).unwrap(), Some(4));
    // 消费掉第一条后应能推算出第二条
    let rest = buf.split_off(4);
    assert_eq!(message_len(&rest).unwrap(), Some(rest.len()));
}

/// ---- process_message：消息处理与全局状态 ----

/// 字符串和列表的负长度必须报错，超限声明不等待响应体。
#[test]
fn test_message_len_rejects_invalid_lengths() {
    for msg_type in [
        TYPE_GAME_CHANNEL,
        TYPE_GAME_MOUSE_STATE,
        TYPE_GAME_WINDOW_SIZE,
        TYPE_LAUNCH_ARG,
    ] {
        for length in [i32::MIN, -1] {
            let mut frame = BytesMut::new();
            frame.put_i32(msg_type);
            frame.put_i32(length);
            assert!(matches!(message_len(&frame), Err(IpcInvalidFrame)));
        }
        for length in [MAX_FRAME_SIZE as i32, i32::MAX] {
            let mut frame = BytesMut::new();
            frame.put_i32(msg_type);
            frame.put_i32(length);
            assert!(matches!(message_len(&frame), Err(IpcFrameTooLarge { .. })));
        }
    }

    let mut list = BytesMut::new();
    list.put_i32(TYPE_LAUNCH_ARG);
    list.put_i32(1);
    list.put_i32(-1);
    assert!(matches!(message_len(&list), Err(IpcInvalidFrame)));
}

/// 限额包含类型头、字符串头和固定字段，恰好等于限额时可接收。
#[test]
fn test_message_len_enforces_whole_frame_limit() {
    let mut frame = msg_with_string(TYPE_GAME_CHANNEL, &"x".repeat(MAX_FRAME_SIZE - 8));
    assert_eq!(message_len(&frame).unwrap(), Some(MAX_FRAME_SIZE));
    frame[..4].copy_from_slice(&TYPE_GAME_MOUSE_STATE.to_be_bytes());
    assert!(matches!(message_len(&frame), Err(IpcFrameTooLarge { .. })));
    frame[..4].copy_from_slice(&TYPE_GAME_WINDOW_SIZE.to_be_bytes());
    assert!(matches!(message_len(&frame), Err(IpcFrameTooLarge { .. })));
}

/// 鼠标状态消息应记录到全局表
#[tokio::test]
async fn test_process_mouse_state() {
    ensure_log();
    let uuid = Uuid::new_v4();
    let mut buf = msg_with_string(TYPE_GAME_MOUSE_STATE, &uuid.to_string());
    buf.put_u8(1);

    process_message(&mut buf, &mpsc::unbounded_channel().0)
        .await
        .unwrap();

    assert_eq!(get_mouse_state(uuid), Some(true));

    // 同 uuid 二次上报 false 应覆盖
    let mut buf = msg_with_string(TYPE_GAME_MOUSE_STATE, &uuid.to_string());
    buf.put_u8(0);
    process_message(&mut buf, &mpsc::unbounded_channel().0)
        .await
        .unwrap();
    assert_eq!(get_mouse_state(uuid), Some(false));
}

/// 鼠标状态消息携带非法 uuid 时应返回错误
#[tokio::test]
async fn test_process_mouse_state_bad_uuid() {
    ensure_log();
    let mut buf = msg_with_string(TYPE_GAME_MOUSE_STATE, "not-a-uuid");
    buf.put_u8(1);

    assert!(
        process_message(&mut buf, &mpsc::unbounded_channel().0)
            .await
            .is_err()
    );
}

/// 窗口大小消息应记录到全局表
#[tokio::test]
async fn test_process_window_size() {
    ensure_log();
    let uuid = Uuid::new_v4();
    let mut buf = msg_with_string(TYPE_GAME_WINDOW_SIZE, &uuid.to_string());
    buf.put_i32(1920);
    buf.put_i32(1080);

    process_message(&mut buf, &mpsc::unbounded_channel().0)
        .await
        .unwrap();

    assert_eq!(
        get_window_size(uuid),
        Some(WindowSize {
            width: 1920,
            height: 1080
        })
    );
}

/// 启动参数消息应写入全局状态并触发参数事件
#[tokio::test]
async fn test_process_launch_arg() {
    ensure_log();
    let expect = vec!["--arg1".to_string(), "--arg2".to_string()];

    let got = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Vec<String>>::new()));
    let got2 = got.clone();
    let id = register_ipc_arg_event(move |args| {
        got2.lock().unwrap().push(args.clone());
    });

    let mut buf = BytesMut::new();
    buf.put_i32(TYPE_LAUNCH_ARG);
    buf.write_string_list(&expect);

    process_message(&mut buf, &mpsc::unbounded_channel().0)
        .await
        .unwrap();

    remove_ipc_arg_event(id);

    assert_eq!(get_run_arg(), expect);
    assert_eq!(&*got.lock().unwrap(), &vec![expect.clone()]);
}

/// 注册游戏通道后应能从列表里查到
#[tokio::test]
async fn test_get_game_uuids() {
    ensure_log();
    let uuid = Uuid::new_v4();
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut buf = msg_with_string(TYPE_GAME_CHANNEL, &uuid.to_string());

    process_message(&mut buf, &tx).await.unwrap();

    assert!(get_game_uuids().contains(&uuid));
}

/// 向未注册通道的游戏发送标题应返回 false
#[test]
fn test_set_title_unknown_uuid() {
    assert!(!set_title(Uuid::new_v4(), "hello"));
}

