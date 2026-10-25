//! mml-ipc 端到端测试
//!
//! 通过真实 TCP 连接验证 IPC 服务的启动、消息解析与定向发送。
//!
//! # 注意
//!
//! 所有测试共用进程内的同一个 IPC 服务，`init` 只启动一次，并默认并行执行，
//! 因此每个测试都必须容忍其他测试产生的消息：
//! - `test_motd_broadcast` 的 MOTD(2) 广播会发给所有已连接客户端；
//! - 读取消息时一律按类型跳过无关帧，直到等到自己需要的那条；
//! - 每条连接都设置读取超时，任何一条消息缺失都会超时失败而不是卡死。

use std::io::{Read, Write};
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, Ordering},
};

use bytes::Buf;
use mml_ipc::{
    ServerInfo, WindowSize, add_server, clear_servers, get_auth_key, get_mouse_state, get_port,
    get_run_arg, get_window_size, init, notify_existing, register_ipc_event, remove_ipc_event,
    send_key, send_mouse_button, send_mouse_move, send_mouse_scroll, set_title,
};
use uuid::Uuid;

/// 保证日志系统只启动一次，并行测试共用同一服务线程
static LOG_STARTED: OnceLock<()> = OnceLock::new();

fn ensure_log() {
    LOG_STARTED.get_or_init(|| {
        let dir = mml_testutil::temp_dir().join(format!("mml_ipc_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let _ = mml_log::start(dir);
    });
}

/// 以第一个实例身份启动 IPC 服务并返回监听端口
fn ipc_port() -> u16 {
    init(None).unwrap().expect("init should start server")
}

/// 连接测试用的 TCP 流，统一设置读取超时，防止卡死
fn connect(port: u16) -> std::net::TcpStream {
    let stream = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect to ipc server");
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .expect("set read timeout");
    stream
}

/// 读取一条消息，消息无总长度前缀，按类型自定界解析，返回「类型 + 消息体」完整字节
fn read_frame(stream: &mut std::net::TcpStream) -> Vec<u8> {
    let mut type_buf = [0u8; 4];
    stream.read_exact(&mut type_buf).unwrap();
    let msg_type = i32::from_be_bytes(type_buf);

    let mut frame = type_buf.to_vec();
    match msg_type {
        // SET_TITLE(10): 标题(String)
        10 => read_string_into(stream, &mut frame),
        // MOTD(2): ip(String) + 端口(String) + MOTD(String)
        2 => {
            read_string_into(stream, &mut frame);
            read_string_into(stream, &mut frame);
            read_string_into(stream, &mut frame);
        }
        // 鼠标移动(5): dx(f64) + dy(f64)
        5 => read_bytes_into(stream, &mut frame, 16),
        // 鼠标按键(6) / 键盘按键(7): i32 + 布尔
        6 | 7 => read_bytes_into(stream, &mut frame, 5),
        // 鼠标滚轮(8): i32
        8 => read_bytes_into(stream, &mut frame, 4),
        // 其它类型没有消息体
        _ => {}
    }
    frame
}

/// 给游戏连接补上「连接验证」(12)，密钥取本进程 IPC 服务生成的密钥
fn send_auth(stream: &mut std::net::TcpStream) {
    let key = get_auth_key().expect("auth key should be generated");
    let mut content = Vec::new();
    content.extend_from_slice(&12i32.to_be_bytes());
    content.extend_from_slice(&(key.len() as i32).to_be_bytes());
    content.extend_from_slice(key.as_bytes());
    write_frame(stream, &content);
}

/// 读取固定长度的消息体并追加到输出
///
/// - `len`: 消息体的字节数
fn read_bytes_into(stream: &mut std::net::TcpStream, out: &mut Vec<u8>, len: usize) {
    let mut bytes = vec![0u8; len];
    stream.read_exact(&mut bytes).unwrap();
    out.extend_from_slice(&bytes);
}

/// 读取一个字符串字段，含 i32 长度前缀与 UTF-8 内容，并追加到输出
fn read_string_into(stream: &mut std::net::TcpStream, out: &mut Vec<u8>) {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).unwrap();
    let len = i32::from_be_bytes(len_buf) as usize;
    out.extend_from_slice(&len_buf);
    let mut bytes = vec![0u8; len];
    stream.read_exact(&mut bytes).unwrap();
    out.extend_from_slice(&bytes);
}

/// 发送一条消息，不带总长度前缀
fn write_frame(stream: &mut std::net::TcpStream, content: &[u8]) {
    stream.write_all(content).unwrap();
}

/// 读取一条指定类型的消息，跳过无关帧，如其他测试产生的 MOTD 广播
fn read_frame_of(stream: &mut std::net::TcpStream, expect_type: i32) -> Vec<u8> {
    loop {
        let payload = read_frame(stream);
        let mut body = payload.as_slice();
        if body.get_i32() == expect_type {
            return payload;
        }
    }
}

/// 从帧字节中读出一个字符串字段
fn read_string(buf: &mut &[u8]) -> String {
    let len = buf.get_i32() as usize;
    let s = String::from_utf8(buf[..len].to_vec()).unwrap();
    buf.advance(len);
    s
}

/// 游戏注册通道后，启动器通过 `set_title` 定向发送消息给该游戏
#[test]
fn test_channel_set_title() {
    ensure_log();
    let port = ipc_port();
    let uuid = Uuid::new_v4();

    let mut stream = connect(port);
    send_auth(&mut stream);

    // 发送 GAME_CHANNEL(9): uuid
    let mut content = Vec::new();
    content.extend_from_slice(&9i32.to_be_bytes());
    let uuid_str = uuid.to_string();
    content.extend_from_slice(&(uuid_str.len() as i32).to_be_bytes());
    content.extend_from_slice(uuid_str.as_bytes());
    write_frame(&mut stream, &content);

    // 等待服务器注册通道
    let mut sent = false;
    for _ in 0..100 {
        if set_title(uuid, "hello") {
            sent = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(sent, "game channel not registered");

    // 读回服务器定向发送的 SET_TITLE(10)，跳过其它测试产生的 MOTD(2) 广播
    let payload = read_frame_of(&mut stream, 10);
    let mut body = payload.as_slice();
    assert_eq!(body.get_i32(), 10);
    assert_eq!(read_string(&mut body), "hello");
}

/// 启动器周期广播服务器 MOTD(2) 给所有已连接的游戏
#[test]
fn test_motd_broadcast() {
    ensure_log();
    let port = ipc_port();
    add_server(ServerInfo {
        ip: "127.0.0.1".to_string(),
        port: "25565".to_string(),
        motd: "123".to_string(),
    });

    let mut stream = connect(port);

    // 读取广播的 MOTD(2)，跳过其它类型帧
    let payload = read_frame_of(&mut stream, 2);
    let mut body = payload.as_slice();
    assert_eq!(body.get_i32(), 2);
    assert_eq!(read_string(&mut body), "127.0.0.1");
    assert_eq!(read_string(&mut body), "25565");
    // ServerInfo 的 motd 字段原样透传，与 ColorMC 的 ColorMCCloudServerObj 一致
    assert_eq!(read_string(&mut body), "123");

    clear_servers();
}

/// 新启动实例通过 `notify_existing` 通知已运行实例，一次性连接：
/// - 无参数时只发送 LAUNCH_SHOW(3)：显示事件触发，参数不被设置；
/// - 有参数时只发送 LAUNCH_ARG(4)：参数被存储，显示事件不触发。
#[test]
fn test_notify_existing() {
    ensure_log();
    let port = ipc_port();

    // 注册显示事件回调，用原子标志验证触发
    let shown = Arc::new(AtomicBool::new(false));
    let shown_flag = shown.clone();
    let id = register_ipc_event(move || {
        shown_flag.store(true, Ordering::SeqCst);
    });

    // 1) 无参数 -> 只发送 SHOW
    assert!(notify_existing(port, Vec::new()));
    let mut ok = false;
    for _ in 0..100 {
        if shown.load(Ordering::SeqCst) && get_run_arg().is_empty() {
            ok = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(ok, "empty args: LAUNCH_SHOW not handled");

    // 2) 有参数 -> 只发送 ARG：参数被存储，显示事件不触发
    let expect = vec!["--open".to_string(), "uuid123".to_string()];
    shown.store(false, Ordering::SeqCst);
    assert!(notify_existing(port, expect.clone()));

    let mut ok = false;
    for _ in 0..100 {
        if get_run_arg() == expect && !shown.load(Ordering::SeqCst) {
            ok = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    remove_ipc_event(id);
    assert!(ok, "with args: LAUNCH_ARG not handled");
}

/// `init` 传入端口：本进程已是服务端时直接返回同一端口，重复调用幂等
#[test]
fn test_init_with_port() {
    ensure_log();
    let port = ipc_port();

    // 本进程已启动服务，无论传入什么端口都返回第一次绑定的端口
    assert_eq!(init(Some(port)).unwrap(), Some(port));
    assert_eq!(init(None).unwrap(), Some(port));
    assert_eq!(get_port(), Some(port));
    assert!(get_auth_key().is_some(), "auth key should be generated");
}

/// 游戏上报的鼠标状态(1)与窗口大小(11)应存入全局状态并可通过 getter 读取
#[test]
fn test_game_state_report() {
    ensure_log();
    let port = ipc_port();
    let uuid = Uuid::new_v4();

    let mut stream = connect(port);
    send_auth(&mut stream);

    // MOUSE_STATE(1): uuid + 状态(bool)
    let mut content = Vec::new();
    content.extend_from_slice(&1i32.to_be_bytes());
    let uuid_str = uuid.to_string();
    content.extend_from_slice(&(uuid_str.len() as i32).to_be_bytes());
    content.extend_from_slice(uuid_str.as_bytes());
    content.push(1);
    write_frame(&mut stream, &content);

    // WINDOW_SIZE(11): uuid + 宽(i32) + 高(i32)
    let mut content = Vec::new();
    content.extend_from_slice(&11i32.to_be_bytes());
    content.extend_from_slice(&(uuid_str.len() as i32).to_be_bytes());
    content.extend_from_slice(uuid_str.as_bytes());
    content.extend_from_slice(&1920i32.to_be_bytes());
    content.extend_from_slice(&1080i32.to_be_bytes());
    write_frame(&mut stream, &content);

    // 等待服务端处理消息
    let mut mouse_ok = false;
    let mut size_ok = false;
    for _ in 0..100 {
        if get_mouse_state(uuid) == Some(true) {
            mouse_ok = true;
        }
        if get_window_size(uuid)
            == Some(WindowSize {
                width: 1920,
                height: 1080,
            })
        {
            size_ok = true;
        }
        if mouse_ok && size_ok {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(mouse_ok, "mouse state not recorded");
    assert!(size_ok, "window size not recorded");
}

/// 向未注册通道的游戏发送标题应返回 false，不依赖网络返回
#[test]
fn test_set_title_unknown_uuid() {
    ensure_log();
    ipc_port();
    assert!(!set_title(Uuid::new_v4(), "nobody"));
}

/// 游戏注册通道后，输入类消息应能按类型定向送达并保持字段布局
#[test]
fn test_send_input() {
    ensure_log();
    let port = ipc_port();
    let uuid = Uuid::new_v4();

    let mut stream = connect(port);
    send_auth(&mut stream);

    // 发送 GAME_CHANNEL(9): uuid
    let mut content = Vec::new();
    content.extend_from_slice(&9i32.to_be_bytes());
    let uuid_str = uuid.to_string();
    content.extend_from_slice(&(uuid_str.len() as i32).to_be_bytes());
    content.extend_from_slice(uuid_str.as_bytes());
    write_frame(&mut stream, &content);

    // 等到通道注册成功；成功的那次 send_key 已经把 KEYBOARD_KEY(7) 排入队列
    let mut sent = false;
    for _ in 0..100 {
        if send_key(uuid, 87, true) {
            sent = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(sent, "game channel not registered");

    let payload = read_frame_of(&mut stream, 7);
    let mut body = payload.as_slice();
    assert_eq!(body.get_i32(), 7);
    assert_eq!(body.get_i32(), 87);
    assert_eq!(body.get_u8(), 1);

    // 鼠标移动是相对位移，负数表示向左或向上
    assert!(send_mouse_move(uuid, 12.5, -3.25));
    let payload = read_frame_of(&mut stream, 5);
    let mut body = payload.as_slice();
    assert_eq!(body.get_i32(), 5);
    assert_eq!(body.get_f64(), 12.5);
    assert_eq!(body.get_f64(), -3.25);

    // 鼠标按键抬起
    assert!(send_mouse_button(uuid, 2, false));
    let payload = read_frame_of(&mut stream, 6);
    let mut body = payload.as_slice();
    assert_eq!(body.get_i32(), 6);
    assert_eq!(body.get_i32(), 2);
    assert_eq!(body.get_u8(), 0);

    // 滚轮向下
    assert!(send_mouse_scroll(uuid, -2));
    let payload = read_frame_of(&mut stream, 8);
    let mut body = payload.as_slice();
    assert_eq!(body.get_i32(), 8);
    assert_eq!(body.get_i32(), -2);
}

/// 断言连接已被服务端关闭
///
/// - `stream`: 待检查的连接
fn assert_closed(stream: &mut std::net::TcpStream) {
    let mut one = [0u8; 1];
    match stream.read(&mut one) {
        Ok(0) => {}
        Ok(n) => panic!("unexpected data before close: {n} bytes"),
        Err(err) if err.kind() == std::io::ErrorKind::ConnectionReset => {}
        Err(err) => panic!("connection was not closed: {err}"),
    }
}

/// 没有发送「连接验证」就发游戏消息，连接应被直接断开
#[test]
fn test_auth_required() {
    ensure_log();
    let port = ipc_port();
    let mut stream = connect(port);

    let uuid = Uuid::new_v4();
    let uuid_str = uuid.to_string();
    let mut content = Vec::new();
    content.extend_from_slice(&9i32.to_be_bytes());
    content.extend_from_slice(&(uuid_str.len() as i32).to_be_bytes());
    content.extend_from_slice(uuid_str.as_bytes());
    write_frame(&mut stream, &content);

    assert_closed(&mut stream);
}

/// 密钥错误时连接应被直接断开
#[test]
fn test_auth_wrong_key() {
    ensure_log();
    let port = ipc_port();
    let mut stream = connect(port);

    let mut content = Vec::new();
    content.extend_from_slice(&12i32.to_be_bytes());
    content.extend_from_slice(&5i32.to_be_bytes());
    content.extend_from_slice(b"wrong");
    write_frame(&mut stream, &content);

    assert_closed(&mut stream);
}

/// 向未注册通道的游戏发送输入应返回 false，不依赖网络返回
#[test]
fn test_send_input_unknown_uuid() {
    ensure_log();
    ipc_port();
    let uuid = Uuid::new_v4();
    assert!(!send_mouse_move(uuid, 1.0, -1.0));
    assert!(!send_mouse_button(uuid, 0, true));
    assert!(!send_key(uuid, 87, true));
    assert!(!send_mouse_scroll(uuid, 1));
}
