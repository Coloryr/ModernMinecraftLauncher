//! 启动器与游戏、以及启动器实例之间的 IPC 通信
//!
//! 启动器在本机绑定 `127.0.0.1` 的临时端口作为 TCP 服务端：
//! - 游戏通过注入的 Agent 连接回启动器，端口、实例 ID 与验证密钥分别来自
//!   `-Dcoloryrasm.mixin.port`、`-Dcoloryrasm.mixin.uuid` 与 `-Dcoloryrasm.mixin.key`；
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
//! | 窗口聚焦 | 13 | 游戏 -> 启动器 | uuid(String) + 状态(bool) |
//! | 连接挑战 | 14 | 启动器 -> 游戏 | 挑战串(String) |
//! | 连接应答 | 15 | 游戏 -> 启动器 | 应答串(String) |
//! | 验证结果 | 16 | 启动器 -> 游戏 | 无，只在通过时下发 |
//!
//! 连接建立后启动器先下发「连接挑战」(14)，内容是这次连接现生成的随机串。游戏用启动参数里的
//! 密钥对它算 HMAC-SHA256，把 64 位小写十六进制结果作为「连接应答」(15) 发回；启动器用同一份
//! 密钥再算一遍比对，通过后把该连接加入广播列表，并回一条无消息体的「验证结果」(16)。
//! 密钥本身不过网，挑战串每次连接都换，抓到的应答无法重放。
//!
//! 应答不对时启动器不回「验证结果」，直接断开；游戏收不到它就知道握手没通过。
//!
//! 未通过验证的连接不能注册游戏通道，也不能上报鼠标 / 窗口状态，发了直接断开；
//! 建立连接后十秒内没通过验证同样断开。启动器实例之间的「启动显示 / 启动参数」不参与握手。
//!
//! 12 号消息是旧的「连接验证」（明文下发密钥），握手改成挑战应答后已停用，编号不再复用。
//!
//! 输入类消息由启动器按游戏通道定向发送，消息体不含 uuid。
//! 键盘与鼠标按键使用 GLFW 编码，鼠标移动是相对位移，单位像素，正数向右或向下；
//! 滚轮正数向上。游戏抓取鼠标时鼠标移动用于转动视角，未抓取时用于移动系统光标；
//! 可用 [`get_mouse_state`] 查询当前模式，据此切换手柄的按键映射。
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
//! 通过 [`init()`] 启动服务并获取监听端口与验证密钥，启动游戏时应把
//! [`get_port()`] 与 [`get_auth_key()`] 的结果作为启动参数传给游戏。
//! 游戏 Agent 连接回启动器后，先用密钥应答启动器下发的挑战串，通过后才注册游戏通道。
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
        IpcAcceptError, IpcBindError, IpcBufferOverrun, IpcFrameTooLarge, IpcGameAuthFailed,
        IpcGameAuthTimeout, IpcGameMessageBeforeAuth, IpcInvalidFrame, IpcRuntimeCreateError,
        IpcWriteError, SocketError, ThreadError,
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
const TYPE_GAME_WINDOW_FOCUS: i32 = 13;
const TYPE_GAME_CHALLENGE: i32 = 14;
const TYPE_GAME_PROOF: i32 = 15;
const TYPE_GAME_AUTH_RESULT: i32 = 16;

/// 单个消息体的最大长度，单位字节；本地 IPC 正常数据远小于该值，用于防御异常长度
const MAX_FRAME_SIZE: usize = 1 << 20;

/// 从连接建立到通过验证的时限，单位秒；超时断开，避免空闲连接一直占着任务
const AUTH_TIMEOUT: Duration = Duration::from_secs(10);

/// 客户端出站通道
type TcpChannel = mpsc::UnboundedSender<BytesMut>;

/// 已连接的客户端，广播时使用
pub(crate) static CLIENTS: RwLock<Vec<TcpChannel>> = RwLock::new(Vec::new());
/// 已注册的游戏实例通道，键为实例 ID
pub(crate) static GAME_CHANNELS: LazyLock<RwLock<HashMap<Uuid, TcpChannel>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// 服务器列表，用于向游戏广播 MOTD
pub(crate) static SERVER_INFOS: RwLock<Vec<ServerInfo>> = RwLock::new(Vec::new());
/// 新启动实例传入的启动参数，用于防止多开时的参数转发
pub(crate) static RUN_ARG: RwLock<Vec<String>> = RwLock::new(Vec::new());
/// 游戏上报的鼠标状态，键为实例 ID
pub(crate) static MOUSE_STATES: LazyLock<RwLock<HashMap<Uuid, bool>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// 游戏上报的窗口大小，键为实例 ID
pub(crate) static WINDOW_SIZES: LazyLock<RwLock<HashMap<Uuid, WindowSize>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// 游戏上报的窗口聚焦状态，键为实例 ID
pub(crate) static FOCUS_STATES: LazyLock<RwLock<HashMap<Uuid, bool>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// IPC 服务端口，只初始化一次
pub(crate) static IPC_PORT: OnceLock<u16> = OnceLock::new();
/// 游戏连接的验证密钥，服务启动时随机生成一次
pub(crate) static AUTH_KEY: OnceLock<String> = OnceLock::new();
/// 保护服务启动过程：并发调用 [`init()`] 时只启动一个服务，其余复用同一端口
pub(crate) static INIT_LOCK: Mutex<()> = Mutex::new(());

/// 启动显示事件：新启动的启动器实例请求已运行实例显示主窗口
pub(crate) static IPC_EVENT: LazyLock<EventHandler> = LazyLock::new(|| EventHandler::new());
/// 启动参数事件：新启动的启动器实例传入的启动参数
pub(crate) static IPC_ARG_EVENT: LazyLock<EventArgHandler<Vec<String>>> =
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

/// 单条连接的握手状态
///
/// 挑战串在连接建立时生成；应答核对通过后认证标记置真，未通过验证的连接
/// 不进广播列表、不能注册游戏通道
pub(crate) struct ClientState {
    /// 本次连接下发的挑战串，应答按它计算
    pub(crate) nonce: String,
    /// 是否已通过挑战应答
    pub(crate) authenticated: bool,
}

impl ClientState {
    /// 为一条新连接生成握手状态，挑战串每次都不同
    fn new() -> Self {
        Self {
            nonce: auth::new_nonce(),
            authenticated: false,
        }
    }
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

/// 检查游戏输入通道是否仍连接，历史鼠标状态不代表连接存活。
///
/// - `uuid`: 游戏实例 ID
///
/// # 返回值
///
/// 通道存在且接收端未关闭时返回 true。
pub fn is_game_connected(uuid: Uuid) -> bool {
    GAME_CHANNELS
        .read()
        .unwrap()
        .get(&uuid)
        .is_some_and(|channel| !channel.is_closed())
}

/// 列出已注册游戏通道的实例
///
/// # 返回值
///
/// 已连接游戏实例的 ID，按 ID 排序；没有游戏连接时为空列表
pub fn get_game_uuids() -> Vec<Uuid> {
    cleanup();
    let mut uuids: Vec<Uuid> = GAME_CHANNELS.read().unwrap().keys().copied().collect();
    uuids.sort();
    uuids
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

/// 获取游戏窗口是否在前台
///
/// - `uuid`: 游戏实例 ID
///
/// # 返回值
///
/// 游戏上报过则返回最近一次状态；未上报过返回 `None`
pub fn get_focus_state(uuid: Uuid) -> Option<bool> {
    FOCUS_STATES.read().unwrap().get(&uuid).copied()
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

pub(crate) mod broadcast;
mod auth;
mod protocol;

#[cfg(test)]
pub(crate) use self::protocol::{message_len, process_message};
mod send;
mod server;

pub(crate) use self::broadcast::cleanup;
pub use self::send::*;
pub(crate) use self::server::run_server;

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

/// 获取本进程 IPC 服务的端口
///
/// # 返回值
///
/// 服务已启动时返回监听端口，未启动返回 None
pub fn get_port() -> Option<u16> {
    IPC_PORT.get().copied()
}

/// 获取游戏连接的验证密钥
///
/// # 返回值
///
/// 服务已启动时返回密钥，未启动返回 None；调用方应把它作为启动参数传给游戏
pub fn get_auth_key() -> Option<&'static str> {
    AUTH_KEY.get().map(String::as_str)
}

/// 在独立线程上运行 IPC 服务，阻塞直到线程结束
///
/// - `port_tx`: 回传绑定端口的通道
#[cfg(test)]
#[path = "../tests/unit/lib_tests.rs"]
mod tests;
