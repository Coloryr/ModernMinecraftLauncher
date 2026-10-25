//! 启动器与游戏、以及启动器实例之间的 IPC 通信
//!
//! 启动器在本机绑定 `127.0.0.1` 的临时端口作为 TCP 服务端：
//! - 游戏通过注入的 Agent 连接回启动器，端口与实例 ID 分别来自
//!   `-Dcolormc.mixin.port` 与 `-Dcolormc.mixin.uuid`；
//! - 重复启动的启动器实例通过该端口通知已运行实例，防止多开：
//!   无启动参数时请求显示窗口，有启动参数时传递参数。
//!
//! # 消息格式
//!
//! 每条消息格式为：`[i32 消息类型][消息体]`，没有总长度前缀。
//! 消息体按字段自定界：字符串为 `[i32 长度][UTF-8 字节]`，字符串列表为
//! `[i32 数量][字符串...]`，布尔为 1 字节，整数为 4 字节，均为大端。
//! 解析时先读消息类型，再按类型逐个读取字段即可确定消息边界，
//! 与 ColorMC 的 `LaunchSocketUtils` 一致。
//!
//! ## 游戏 <-> 启动器
//!
//! | 类型 | 值 | 方向 | 消息体 |
//! |------|----|------|--------|
//! | 鼠标状态 | 1 | 游戏 -> 启动器 | uuid(String) + 状态(bool) |
//! | 服务器 MOTD | 2 | 启动器 -> 游戏 | ip(String) + 端口(String) + MOTD(String) |
//! | 鼠标移动 | 5 | 启动器 -> 游戏 | dx(f64) + dy(f64) |
//! | 鼠标按键 | 6 | 启动器 -> 游戏 | button(i32) + down(bool) |
//! | 键盘按键 | 7 | 启动器 -> 游戏 | key(i32) + down(bool) |
//! | 鼠标滚轮 | 8 | 启动器 -> 游戏 | amount(i32) |
//! | 游戏通道 | 9 | 游戏 -> 启动器 | uuid(String) |
//! | 设置标题 | 10 | 启动器 -> 游戏 | 标题(String) |
//! | 窗口大小 | 11 | 游戏 -> 启动器 | uuid(String) + 宽(i32) + 高(i32) |
//!
//! 输入类消息由启动器按游戏通道定向发送，消息体不含 uuid。
//! 键盘与鼠标按键使用 GLFW 编码，鼠标移动是相对位移，单位像素，正数向右或向下；
//! 滚轮正数向上。游戏抓取鼠标时鼠标移动用于转动视角，未抓取时用于移动系统光标。
//!
//! ## 启动器 -> 启动器，防止多开
//!
//! | 类型 | 值 | 消息体 |
//! |------|----|--------|
//! | 启动显示 | 3 | 无 |
//! | 启动参数 | 4 | 参数列表(StringList) |
//!
//! 新启动实例只发送其中一条：无启动参数时发送「启动显示」(3) 请求已运行实例
//! 显示主窗口；有启动参数时发送「启动参数」(4) 将本次参数交给已运行实例。
//! 发送后立即关闭连接，不会在本地启动 IPC 服务。
//!
//! 通过 [`init()`] 启动服务并获取监听端口，启动游戏时应将该端口作为
//! `GameLaunchArg.mixin` 传入，游戏 Agent 通过该端口连接回启动器。
//! 启动器启动时若已记录到运行中的实例端口，将端口传给 [`init()`]：
//! init 会读取本进程命令行参数并调用 [`notify_existing()`] 通知已运行实例，
//! 通知成功时返回 `Ok(None)`，当前进程应退出；否则本进程作为第一个实例
//! 启动服务。

use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex, OnceLock, RwLock},
    thread,
    time::Duration,
};

use bytes::{Buf, BufMut, BytesMut};
use mml_base::events::{EventArgHandler, EventHandler};
use mml_names::i18_items::error_type::{
    CoreResult, ErrorData,
    ErrorType::{
        IpcAcceptError, IpcBindError, IpcBufferOverrun, IpcFrameTooLarge, IpcInvalidFrame,
        IpcRuntimeCreateError, IpcWriteError, SocketError, ThreadError,
    },
};
use mml_names::i18_items::info_type::InfoType;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    time,
};
use uuid::Uuid;

use crate::byte_buf::ByteBufExt;

pub mod byte_buf;

const TYPE_GAME_MOUSE_STATE: i32 = 1;
const TYPE_GAME_MOTD: i32 = 2;
const TYPE_LAUNCH_SHOW: i32 = 3;
const TYPE_LAUNCH_ARG: i32 = 4;
const TYPE_MOUSE_MOVE: i32 = 5;
const TYPE_MOUSE_BUTTON: i32 = 6;
const TYPE_KEYBOARD_KEY: i32 = 7;
const TYPE_MOUSE_SCROLL: i32 = 8;
const TYPE_GAME_CHANNEL: i32 = 9;
const TYPE_SET_TITLE: i32 = 10;
const TYPE_GAME_WINDOW_SIZE: i32 = 11;

/// 单个消息体的最大长度，单位字节；本地 IPC 正常数据远小于该值，用于防御异常长度
const MAX_FRAME_SIZE: usize = 1 << 20;

/// 客户端出站通道
type TcpChannel = mpsc::UnboundedSender<BytesMut>;

/// 已连接的客户端，广播时使用
static CLIENTS: RwLock<Vec<TcpChannel>> = RwLock::new(Vec::new());
/// 已注册的游戏实例通道，键为实例 ID
static GAME_CHANNELS: LazyLock<RwLock<HashMap<Uuid, TcpChannel>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// 服务器列表，用于向游戏广播 MOTD
static SERVER_INFOS: RwLock<Vec<ServerInfo>> = RwLock::new(Vec::new());
/// 新启动实例传入的启动参数，用于防止多开时的参数转发
static RUN_ARG: RwLock<Vec<String>> = RwLock::new(Vec::new());
/// 游戏上报的鼠标状态，键为实例 ID
static MOUSE_STATES: LazyLock<RwLock<HashMap<Uuid, bool>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// 游戏上报的窗口大小，键为实例 ID
static WINDOW_SIZES: LazyLock<RwLock<HashMap<Uuid, WindowSize>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// IPC 服务端口，只初始化一次
static IPC_PORT: OnceLock<u16> = OnceLock::new();
/// 保护服务启动过程：并发调用 [`init()`] 时只启动一个服务，其余复用同一端口
static INIT_LOCK: Mutex<()> = Mutex::new(());

/// 启动显示事件：新启动的启动器实例请求已运行实例显示主窗口
static IPC_EVENT: LazyLock<EventHandler> = LazyLock::new(|| EventHandler::new());
/// 启动参数事件：新启动的启动器实例传入的启动参数
static IPC_ARG_EVENT: LazyLock<EventArgHandler<Vec<String>>> =
    LazyLock::new(|| EventArgHandler::new());

/// 服务器信息
#[derive(Clone, Debug)]
pub struct ServerInfo {
    /// 服务器地址，IPv4 或域名
    pub ip: String,
    /// 服务器端口，字符串形式的十进制端口号
    pub port: String,
    /// 服务器 MOTD 信息，原样透传给游戏
    pub motd: String,
}

/// 游戏上报的窗口大小
///
/// 由游戏端的「窗口大小」(11) 消息上报，按实例 ID 记入 [`WINDOW_SIZES`]，
/// 经 [`get_window_size`] 查询
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowSize {
    /// 窗口宽度，单位像素；取值来自游戏端上报的 i32，无有效范围约束
    pub width: i32,
    /// 窗口高度，单位像素；取值来自游戏端上报的 i32，无有效范围约束
    pub height: i32,
}

/// 注册启动显示事件回调，新启动实例请求显示主窗口时触发
///
/// - `handler`: 回调函数
///
/// # 返回值
///
/// 返回处理器编号，用于 [`remove_ipc_event`] 移除
pub fn register_ipc_event<F>(handler: F) -> u64
where
    F: Fn() + Send + Sync + 'static,
{
    IPC_EVENT.add_handler(handler)
}

/// 移除启动显示事件回调
///
/// - `id`: 处理器编号
pub fn remove_ipc_event(id: u64) {
    IPC_EVENT.remove_handle(id);
}

/// 注册启动参数事件回调，新启动实例传入启动参数时触发
///
/// - `handler`: 回调函数
///
/// # 返回值
///
/// 返回处理器编号，用于 [`remove_ipc_arg_event`] 移除
pub fn register_ipc_arg_event<F>(handler: F) -> u64
where
    F: Fn(&Vec<String>) + Send + Sync + 'static,
{
    IPC_ARG_EVENT.add_handler(handler)
}

/// 移除启动参数事件回调
///
/// - `id`: 处理器编号
pub fn remove_ipc_arg_event(id: u64) {
    IPC_ARG_EVENT.remove_handel(id);
}

/// 获取新启动实例传入的启动参数，用于防止多开时的参数转发
///
/// # 返回值
///
/// 新启动实例转发的参数列表；没有时为空列表
pub fn get_run_arg() -> Vec<String> {
    RUN_ARG.read().unwrap().clone()
}

/// 获取游戏上报的鼠标抓取状态
///
/// - `uuid`: 游戏实例 ID
///
/// # 返回值
///
/// 返回该实例最近一次上报的状态。`true` 表示游戏已抓取鼠标，鼠标移动用于转动视角；
/// `false` 表示处于光标模式，鼠标移动用于移动系统光标；未上报过返回 `None`
pub fn get_mouse_state(uuid: Uuid) -> Option<bool> {
    MOUSE_STATES.read().unwrap().get(&uuid).copied()
}

/// 获取游戏上报的窗口大小
///
/// - `uuid`: 游戏实例 ID
///
/// # 返回值
///
/// 见 [`WindowSize`]；未上报过返回 `None`
pub fn get_window_size(uuid: Uuid) -> Option<WindowSize> {
    WINDOW_SIZES.read().unwrap().get(&uuid).copied()
}

/// 添加服务器信息，用于向游戏广播 MOTD
///
/// - `info`: 服务器信息，含地址、端口与 MOTD；重复添加不去重，按调用顺序累积
pub fn add_server(info: ServerInfo) {
    SERVER_INFOS.write().unwrap().push(info);
}

/// 清理服务器信息
pub fn clear_servers() {
    SERVER_INFOS.write().unwrap().clear();
}

/// 向指定游戏实例发送设置标题消息
///
/// - `uuid`: 游戏实例 ID
/// - `title`: 要设置的标题
///
/// # 返回值
///
/// 消息体只含标题，与 ColorMC 一致，游戏实例由 `uuid` 对应的通道定位；
/// 发送成功返回 `true`，游戏实例未注册通道时返回 `false`
pub fn set_title(uuid: Uuid, title: &str) -> bool {
    let mut content = BytesMut::new();
    content.put_i32(TYPE_SET_TITLE);
    content.write_string(title);
    send_to_game(uuid, content)
}

/// 向指定游戏实例发送鼠标相对移动
///
/// - `uuid`: 游戏实例 ID
/// - `dx`: 水平相对位移，单位像素，正数向右
/// - `dy`: 垂直相对位移，单位像素，正数向下
///
/// # 返回值
///
/// 发送成功返回 `true`，游戏实例未注册通道时返回 `false`。
/// 游戏抓取鼠标时该消息用于转动视角，未抓取时用于移动游戏窗口内的系统光标
pub fn send_mouse_move(uuid: Uuid, dx: f64, dy: f64) -> bool {
    let mut content = BytesMut::new();
    content.put_i32(TYPE_MOUSE_MOVE);
    content.put_f64(dx);
    content.put_f64(dy);
    send_to_game(uuid, content)
}

/// 向指定游戏实例发送鼠标按键
///
/// - `uuid`: 游戏实例 ID
/// - `button`: 鼠标按键，与 GLFW 一致，0 左键、1 右键、2 中键
/// - `down`: `true` 按下，`false` 抬起
///
/// # 返回值
///
/// 发送成功返回 `true`，游戏实例未注册通道时返回 `false`
pub fn send_mouse_button(uuid: Uuid, button: i32, down: bool) -> bool {
    let mut content = BytesMut::new();
    content.put_i32(TYPE_MOUSE_BUTTON);
    content.put_i32(button);
    content.put_u8(if down { 1 } else { 0 });
    send_to_game(uuid, content)
}

/// 向指定游戏实例发送键盘按键
///
/// - `uuid`: 游戏实例 ID
/// - `key`: GLFW 键码，例如 W 为 87
/// - `down`: `true` 按下，`false` 抬起
///
/// # 返回值
///
/// 发送成功返回 `true`，游戏实例未注册通道时返回 `false`
pub fn send_key(uuid: Uuid, key: i32, down: bool) -> bool {
    let mut content = BytesMut::new();
    content.put_i32(TYPE_KEYBOARD_KEY);
    content.put_i32(key);
    content.put_u8(if down { 1 } else { 0 });
    send_to_game(uuid, content)
}

/// 向指定游戏实例发送鼠标滚轮
///
/// - `uuid`: 游戏实例 ID
/// - `amount`: 滚轮格数，正数向上
///
/// # 返回值
///
/// 发送成功返回 `true`，游戏实例未注册通道时返回 `false`
pub fn send_mouse_scroll(uuid: Uuid, amount: i32) -> bool {
    let mut content = BytesMut::new();
    content.put_i32(TYPE_MOUSE_SCROLL);
    content.put_i32(amount);
    send_to_game(uuid, content)
}

/// 通知已运行的启动器实例，防止多开 —— 启动器客户端
///
/// - `port`: 已运行实例的监听端口
/// - `args`: 本进程的启动参数，为空则发送「启动显示」
///
/// # 返回值
///
/// 作为客户端向 `port` 发送一次数据后立即关闭连接，本进程不会启动 IPC 服务。
/// 根据启动参数决定发送的消息：
/// - `args` 为空时发送「启动显示」(3)，请求已运行实例显示主窗口；
/// - `args` 非空时发送「启动参数」(4)，把本次启动参数交给已运行实例。
///
/// 返回 `true` 表示已通知成功，当前进程应退出，由已有实例接管；
/// 返回 `false` 表示没有可连接的实例，端口错误或服务未运行
pub fn notify_existing(port: u16, args: Vec<String>) -> bool {
    use std::io::Write;

    let Ok(mut stream) = std::net::TcpStream::connect(("127.0.0.1", port)) else {
        return false;
    };
    if stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .is_err()
    {
        return false;
    }

    // 协议无总长度前缀，与 ColorMC 一致，消息体自定界
    let mut content = BytesMut::new();
    if args.is_empty() {
        content.put_i32(TYPE_LAUNCH_SHOW);
    } else {
        content.put_i32(TYPE_LAUNCH_ARG);
        content.write_string_list(&args);
    }

    stream.write_all(&content).is_ok()
}

/// 初始化 IPC 服务，含单开检查
///
/// - `port`: 已运行启动器实例记录的端口，无已运行实例时为 `None`
///
/// # 返回值
///
/// `port` 由调用方从端口文件或启动环境读取：
/// - 为 `Some(port)` 时：读取本进程命令行参数，根据有无启动参数调用
///   [`notify_existing()`] 通知已运行实例，无参数发送「启动显示」，有参数发送
///   「启动参数」。通知成功返回 `Ok(None)`，调用方应退出当前进程；通知失败
///   说明原实例已退出，端口过期，本进程继续作为第一个实例启动服务。
/// - 为 `None` 时：直接作为第一个实例启动服务。
///
/// 服务在本机绑定一个临时端口，在独立线程上启动接收客户端连接和服务器列表
/// 广播任务。返回 `Ok(Some(port))` 表示本进程作为服务端，应将该端口作为
/// `GameLaunchArg.mixin` 传入并记录供后续启动实例使用；返回 `Ok(None)` 表示
/// 已通知已运行实例，本进程应立即退出。
///
/// 本进程已启动过服务时，重复调用直接返回第一次绑定的端口
pub fn init(port: Option<u16>) -> CoreResult<Option<u16>> {
    // 串行化服务启动：并发调用时只有一个线程真正启动服务并设置 IPC_PORT，
    // 其余线程等待锁后直接复用同一端口，否则各自绑定会返回不同端口
    let _guard = INIT_LOCK.lock().unwrap();

    // 本进程已经是服务端：直接返回已绑定端口
    if let Some(port) = IPC_PORT.get() {
        return Ok(Some(*port));
    }

    // 存在已运行实例的端口：读取本进程命令行参数并通知已运行实例
    if let Some(port) = port {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if notify_existing(port, args) {
            mml_log::info_type(InfoType::IpcNotifyExisting { port });
            return Ok(None);
        }
        mml_log::warn_type(InfoType::IpcNotifyExistingFailed { port });
    }

    // 端口由服务线程绑定后回传。
    // 注意：必须使用 tokio 原生 bind，`TcpListener::from_std` 接受的流
    // 无法被 spawned 任务正常轮询，本平台实测任务不会运行。
    let (port_tx, port_rx) = std::sync::mpsc::channel::<u16>();

    thread::Builder::new()
        .name("Mml Ipc Server".to_string())
        .spawn(move || run_server(port_tx))
        .map_err(|err| ThreadError(ErrorData::from_error(err)))?;

    // 等待服务绑定端口；若线程启动后立即失败、发送端被丢弃，则返回错误
    let port = port_rx
        .recv()
        .map_err(|err| SocketError(ErrorData::from_error(err)))?;

    let _ = IPC_PORT.set(port);
    mml_log::info_type(InfoType::IpcServerStart { port });

    Ok(Some(port))
}

/// 在独立线程上运行 IPC 服务，阻塞直到线程结束
///
/// - `port_tx`: 回传绑定端口的通道
fn run_server(port_tx: std::sync::mpsc::Sender<u16>) {
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

        // 回传绑定端口
        let port = listener.local_addr().map(|addr| addr.port()).unwrap_or(0);
        let _ = port_tx.send(port);

        let _ = tokio::spawn(accept_loop(listener));
        let _ = tokio::spawn(broadcast_servers());
        std::future::pending::<()>().await;
    });
}

/// 接受客户端连接，为每个连接启动处理任务
async fn accept_loop(listener: TcpListener) -> CoreResult<()> {
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
async fn handle_client(stream: TcpStream) -> CoreResult<()> {
    let (mut reader, mut writer) = tokio::io::split(stream);
    let (write_tx, mut write_rx) = mpsc::unbounded_channel::<BytesMut>();

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

/// 计算缓冲区首条消息的完整长度，单位字节，不消费数据
///
/// 消息无总长度前缀，按类型读取字段推算边界，字符串与列表自带长度；
/// 缓冲区中数据不足一条完整消息时返回 `Ok(None)`，无效长度立即报错。未知类型无法确定长度，
/// 按一条仅含类型头的 4 字节消息处理。
fn message_len(buf: &[u8]) -> CoreResult<Option<usize>> {
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
        TYPE_GAME_CHANNEL => {
            if !string_field(buf, &mut offset)? {
                return Ok(None);
            }
        }
        TYPE_GAME_WINDOW_SIZE => {
            if !string_field(buf, &mut offset)? || !advance(buf, &mut offset, 8)? {
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
/// - `client_tx`: 该连接的出站通道，注册游戏通道时使用
async fn process_message(
    data: &mut BytesMut,
    client_tx: &mpsc::UnboundedSender<BytesMut>,
) -> CoreResult<()> {
    if data.len() < 4 {
        return Ok(());
    }
    let msg_type = data.get_i32();

    match msg_type {
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
        _ => {
            mml_log::warn_type(InfoType::IpcUnknownMessageType { msg_type });
        }
    }

    Ok(())
}

/// 周期广播服务器 MOTD 给所有游戏客户端
async fn broadcast_servers() {
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

/// 清理已关闭的客户端通道
fn cleanup() {
    CLIENTS.write().unwrap().retain(|tx| !tx.is_closed());
    GAME_CHANNELS
        .write()
        .unwrap()
        .retain(|_uuid, tx| !tx.is_closed());
}

/// 广播消息给所有客户端
///
/// - `msg`: 完整消息，含类型头与消息体
fn broadcast(msg: BytesMut) {
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
fn send_to_game(uuid: Uuid, msg: BytesMut) -> bool {
    cleanup();
    if let Some(tx) = GAME_CHANNELS.read().unwrap().get(&uuid) {
        return tx.send(msg).is_ok();
    }
    false
}

#[cfg(test)]
mod tests {
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

    /// 向未注册通道的游戏发送标题应返回 false
    #[test]
    fn test_set_title_unknown_uuid() {
        assert!(!set_title(Uuid::new_v4(), "hello"));
    }
}
