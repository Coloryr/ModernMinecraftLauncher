//! 局域网游戏相关，基于组播发现

use std::{
    mem::MaybeUninit,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    sync::{
        Arc, LazyLock,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use mml_base::events::EventArgHandler;
use regex::Regex;

use mml_names::{
    i18,
    i18_items::{
        error_type::{CoreResult, ErrorData, ErrorType},
        thread_type::ThreadType,
    },
};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};

/// 组播端口
const PORT: u16 = 4445;

/// IPv4 组播地址
const IPV4: &str = "224.0.2.60";
/// IPv6 组播地址
const IPV6: &str = "FF75:230::60";

/// 局域网广播信息解析正则
static LAN_INFO_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[MOTD\](.*?)\[/MOTD\]\[AD\](.*?)\[/AD\]").unwrap());

/// 从广播文本解析 MOTD
///
/// - `text`: 广播文本
///
/// # 返回值
///
/// 格式不符时返回 `None`
fn get_motd(text: &str) -> Option<GameMotd> {
    let caps = LAN_INFO_RE.captures(text)?;

    Some(GameMotd {
        motd: caps
            .get(1)
            .map_or(String::from("missing"), |m| m.as_str().to_string()),
        port: caps
            .get(2)
            .map_or(String::from("missing"), |m| m.as_str().to_string()),
        addr: None,
    })
}

/// 构建 MOTD 广播文本
///
/// - `motd`: MOTD 信息
///
/// # 返回值
///
/// 返回广播文本
fn build_motd(motd: GameMotd) -> String {
    format!("[MOTD]{}[/MOTD][AD]{}[/AD]", motd.motd, motd.port)
}

/// 局域网组播收发器，客户端与服务端共用一个类型，靠创建方式区分角色
pub struct GameLan {
    /// IPv4 组播 socket
    socket_v4: Arc<Socket>,
    /// IPv6 组播 socket，本机没有 IPv6 时为 `None`
    socket_v6: Arc<Option<Socket>>,
    /// 是否运行中，`stop` 后收发线程退出循环
    is_run: Arc<AtomicBool>,
    /// 接收回调，只有客户端有
    events: Option<Arc<EventArgHandler<GameMotd>>>,
    /// IPv4 发送地址，只有服务端有
    send_v4: Option<SockAddr>,
    /// IPv6 发送地址，只有服务端且本机有 IPv6 时才有
    send_v6: Option<SockAddr>,
}

/// 局域网广播信息
pub struct GameMotd {
    /// 服务器描述，即广播文本里 `[MOTD]` 段的内容
    pub motd: String,
    /// 服务器端口，即广播文本里 `[AD]` 段的内容，以字符串承载
    pub port: String,
    /// 发送方地址，发送侧不填，接收侧由 socket 补上
    pub addr: Option<SocketAddr>,
}

impl GameLan {
    /// 创建局域网客户端，用于接收组播
    ///
    /// # 返回值
    ///
    /// 返回组播收发器；创建 socket 失败返回对应错误
    pub fn new_client() -> CoreResult<Self> {
        let interfaces = if_addrs::get_if_addrs().map_err(|err| {
            ErrorType::SocketError(ErrorData {
                error: err.to_string(),
            })
        })?;

        let v6 = interfaces
            .iter()
            .any(|item| !item.is_loopback() && item.ip().is_ipv6());

        let socket_v4 =
            Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).map_err(|err| {
                ErrorType::SocketError(ErrorData {
                    error: err.to_string(),
                })
            })?;
        socket_v4.set_reuse_address(true).map_err(|err| {
            ErrorType::SocketError(ErrorData {
                error: err.to_string(),
            })
        })?;
        let bind_addr_v4 = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, PORT);
        socket_v4
            .bind(&SockAddr::from(bind_addr_v4))
            .map_err(|err| {
                ErrorType::SocketError(ErrorData {
                    error: err.to_string(),
                })
            })?;
        let addr_v4 = IPV4.parse::<Ipv4Addr>().map_err(|err| {
            ErrorType::SocketError(ErrorData {
                error: err.to_string(),
            })
        })?;

        let mut socket_v6 = None;
        if v6 {
            let sock_v6 =
                Socket::new(Domain::IPV6, Type::DGRAM, Some(Protocol::UDP)).map_err(|err| {
                    ErrorType::SocketError(ErrorData {
                        error: err.to_string(),
                    })
                })?;

            sock_v6.set_reuse_address(true).map_err(|err| {
                ErrorType::SocketError(ErrorData {
                    error: err.to_string(),
                })
            })?;

            sock_v6.set_only_v6(true).map_err(|err| {
                ErrorType::SocketError(ErrorData {
                    error: err.to_string(),
                })
            })?;

            let bind_addr_v6 = SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, PORT, 0, 0);
            sock_v6.bind(&SockAddr::from(bind_addr_v6)).map_err(|err| {
                ErrorType::SocketError(ErrorData {
                    error: err.to_string(),
                })
            })?;

            socket_v6 = Some(sock_v6);
        }

        let addr_v6 = IPV6.parse::<Ipv6Addr>().map_err(|err| {
            ErrorType::SocketError(ErrorData {
                error: err.to_string(),
            })
        })?;

        for interface in interfaces.iter() {
            if interface.is_loopback() {
                continue;
            }

            match interface.ip() {
                IpAddr::V4(ipv4_addr) => {
                    socket_v4
                        .join_multicast_v4(&addr_v4, &ipv4_addr)
                        .map_err(|err| {
                            ErrorType::SocketError(ErrorData {
                                error: err.to_string(),
                            })
                        })?;
                }
                IpAddr::V6(_ipv6_addr) => {
                    if v6 {
                        if let Some(sock_v6) = socket_v6.as_ref() {
                            sock_v6
                                .join_multicast_v6(&addr_v6, interface.index.unwrap_or(0))
                                .map_err(|err| {
                                    ErrorType::SocketError(ErrorData {
                                        error: err.to_string(),
                                    })
                                })?;
                        }
                    }
                }
            }
        }

        Ok(Self {
            socket_v4: Arc::new(socket_v4),
            socket_v6: Arc::new(socket_v6.take()),
            is_run: Arc::new(AtomicBool::new(false)),
            events: Some(Arc::new(EventArgHandler::new())),
            send_v4: None,
            send_v6: None,
        })
    }

    /// 创建局域网服务端，用于发送组播
    ///
    /// # 返回值
    ///
    /// 返回组播收发器；创建 socket 失败返回对应错误
    pub fn new_server() -> CoreResult<Self> {
        let interfaces = if_addrs::get_if_addrs().map_err(|err| {
            ErrorType::SocketError(ErrorData {
                error: err.to_string(),
            })
        })?;

        let v6 = interfaces
            .iter()
            .any(|item| !item.is_loopback() && item.ip().is_ipv6());

        let socket_v4 =
            Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).map_err(|err| {
                ErrorType::SocketError(ErrorData {
                    error: err.to_string(),
                })
            })?;

        socket_v4.set_reuse_address(true).map_err(|err| {
            ErrorType::SocketError(ErrorData {
                error: err.to_string(),
            })
        })?;

        socket_v4.set_broadcast(true).map_err(|err| {
            ErrorType::SocketError(ErrorData {
                error: err.to_string(),
            })
        })?;

        let bind_addr_v4 = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0);
        socket_v4
            .bind(&SockAddr::from(bind_addr_v4))
            .map_err(|err| {
                ErrorType::SocketError(ErrorData {
                    error: err.to_string(),
                })
            })?;

        let ipv4 = IPV4.parse::<Ipv4Addr>().map_err(|err| {
            ErrorType::SocketError(ErrorData {
                error: err.to_string(),
            })
        })?;
        let addr_v4 = SocketAddr::V4(SocketAddrV4::new(ipv4, PORT));

        let mut socket_v6 = None;
        let mut addr_v6 = None;

        if v6 {
            let sock_v6 =
                Socket::new(Domain::IPV6, Type::DGRAM, Some(Protocol::UDP)).map_err(|err| {
                    ErrorType::SocketError(ErrorData {
                        error: err.to_string(),
                    })
                })?;

            sock_v6.set_reuse_address(true).map_err(|err| {
                ErrorType::SocketError(ErrorData {
                    error: err.to_string(),
                })
            })?;

            sock_v6.set_only_v6(true).map_err(|err| {
                ErrorType::SocketError(ErrorData {
                    error: err.to_string(),
                })
            })?;

            let bind_addr_v6 = SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0);
            sock_v6.bind(&SockAddr::from(bind_addr_v6)).map_err(|err| {
                ErrorType::SocketError(ErrorData {
                    error: err.to_string(),
                })
            })?;

            let ipv6 = IPV6.parse::<Ipv6Addr>().map_err(|err| {
                ErrorType::SocketError(ErrorData {
                    error: err.to_string(),
                })
            })?;
            addr_v6 = Some(SocketAddr::V6(SocketAddrV6::new(ipv6, PORT, 0, 0)));

            socket_v6 = Some(sock_v6);
        }

        Ok(Self {
            socket_v4: Arc::new(socket_v4),
            socket_v6: Arc::new(socket_v6),
            is_run: Arc::new(AtomicBool::new(false)),
            events: None,
            send_v4: Some(SockAddr::from(addr_v4)),
            send_v6: addr_v6.map(SockAddr::from),
        })
    }

    /// 添加接受回调
    ///
    /// - `handler`: 收到广播时的回调
    ///
    /// # 返回值
    ///
    /// 返回回调 ID；非客户端返回 `u64::MAX`
    pub fn add_event_handler<F>(&self, handler: F) -> u64
    where
        F: Fn(&GameMotd) + Send + Sync + 'static,
    {
        self.events
            .as_ref()
            .map(|item| item.add_handler(handler))
            .unwrap_or(u64::MAX)
    }

    /// 删除接受回调
    ///
    /// - `id`: 回调 ID
    pub fn remove_event_handler(&self, id: u64) {
        if let Some(handle) = self.events.as_ref() {
            handle.remove_handel(id);
        }
    }

    /// 启动发送组播，服务端专用
    ///
    /// - `motd`: 广播的服务器信息
    ///
    /// # 返回值
    ///
    /// 未按服务端创建或线程创建失败时返回对应错误
    pub fn start_send(&self, motd: GameMotd) -> CoreResult<()> {
        self.is_run.store(true, Ordering::Release);

        let run = self.is_run.clone();
        let socket_v4 = self.socket_v4.clone();
        let socket_v6 = self.socket_v6.clone();

        let addr_v4 = self.send_v4.clone().ok_or_else(|| {
            ErrorType::SocketError(ErrorData {
                error: "IPv4 send address not set".to_string(),
            })
        })?;

        let addr_v6 = self.send_v6.clone();

        thread::Builder::new()
            .name(i18::get_thread(ThreadType::LanServer))
            .spawn(move || {
                let motd = build_motd(motd);
                let data = motd.as_bytes();

                while run.load(Ordering::Acquire) {
                    // 发送 IPv4 组播
                    if let Err(err) = socket_v4.send_to(data, &addr_v4) {
                        mml_log::error_type(ErrorType::SocketError(ErrorData {
                            error: format!("Failed to send IPv4 multicast: {}", err),
                        }));
                    }

                    // 发送 IPv6 组播
                    if let Some(sock_v6) = socket_v6.as_ref()
                        && let Some(addr) = addr_v6.as_ref()
                    {
                        if let Err(err) = sock_v6.send_to(data, addr) {
                            mml_log::error_type(ErrorType::SocketError(ErrorData {
                                error: format!("Failed to send IPv6 multicast: {}", err),
                            }));
                        }
                    }

                    thread::sleep(std::time::Duration::from_secs(3));
                }
            })
            .map_err(|err| {
                ErrorType::ThreadError(ErrorData {
                    error: err.to_string(),
                })
            })?;

        Ok(())
    }

    /// 启动接收组播，客户端专用
    ///
    /// # 返回值
    ///
    /// 线程创建失败时返回对应错误
    pub fn start_read(&self) -> CoreResult<()> {
        self.is_run.store(true, Ordering::Release);

        let run_v4 = self.is_run.clone();
        let socket_v4 = self.socket_v4.clone();
        let events = self.events.clone();

        // IPv4 接收线程
        thread::Builder::new()
            .name(i18::get_thread(ThreadType::LanClientV4))
            .spawn(move || {
                let mut buffer = [MaybeUninit::<u8>::new(0); 1024];
                while run_v4.load(Ordering::Acquire) {
                    match socket_v4.recv_from(&mut buffer) {
                        Ok((size, addr)) => {
                            let data = unsafe {
                                let slice = &buffer[..size];
                                std::slice::from_raw_parts(slice.as_ptr() as *const u8, size)
                            };

                            if let Ok(text) = std::str::from_utf8(data) {
                                if let Some(mut motd) = get_motd(text) {
                                    motd.addr = Some(addr.as_socket().unwrap());
                                    if let Some(events) = events.as_ref() {
                                        events.emit(motd);
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            mml_log::error_type(ErrorType::SocketError(ErrorData {
                                error: err.to_string(),
                            }));
                        }
                    }
                }

                unsafe {
                    buffer.assume_init_drop();
                }
            })
            .map_err(|err| {
                ErrorType::ThreadError(ErrorData {
                    error: err.to_string(),
                })
            })?;

        // IPv6 接收线程
        let run_v6 = self.is_run.clone();
        let socket_v6 = self.socket_v6.clone();
        let events6 = self.events.clone();

        if socket_v6.as_ref().is_some() {
            thread::Builder::new()
                .name(i18::get_thread(ThreadType::LanClientV6))
                .spawn(move || {
                    let mut buffer = [MaybeUninit::<u8>::new(0); 1024];
                    while run_v6.load(Ordering::Acquire) {
                        match socket_v6.as_ref().as_ref().unwrap().recv_from(&mut buffer) {
                            Ok((size, addr)) => {
                                let data = unsafe {
                                    let slice = &buffer[..size];
                                    std::slice::from_raw_parts(slice.as_ptr() as *const u8, size)
                                };

                                if let Ok(text) = std::str::from_utf8(data) {
                                    if let Some(mut motd) = get_motd(text) {
                                        motd.addr = Some(addr.as_socket().unwrap());
                                        if let Some(events) = events6.as_ref() {
                                            events.emit(motd);
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                mml_log::error_type(ErrorType::SocketError(ErrorData {
                                    error: err.to_string(),
                                }));
                            }
                        }
                    }

                    unsafe {
                        buffer.assume_init_drop();
                    }
                })
                .map_err(|err| {
                    ErrorType::ThreadError(ErrorData {
                        error: err.to_string(),
                    })
                })?;
        }

        Ok(())
    }

    /// 停止组播收发，收发线程在下一轮循环检查到标志后自行退出
    pub fn stop(&self) {
        self.is_run.store(false, Ordering::Release);
    }
}
