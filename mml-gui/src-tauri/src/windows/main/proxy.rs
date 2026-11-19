//! 游戏内代理的连通性检查
//!
//! 只测「这个地址 + 端口能不能连上」：账号密码是启动时才带上的，
//! 连得上至少说明地址写对了、本机能出网。

use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use crate::err_keys;

/// 连接超时，单位毫秒
const TIMEOUT: Duration = Duration::from_millis(3000);

/// 测一次代理地址能不能连上
///
/// - \`ip\`: 代理主机名或 IP
/// - \`port\`: 代理端口
///
/// # 返回值
///
/// 连上时返回耗时，单位毫秒；地址不合法或连不上时返回错误文案
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_test_proxy(ip: String, port: u16) -> Result<u32, String> {
    let ip = ip.trim();
    if ip.is_empty() || port == 0 {
        return Err(err_keys::PROXY_INVALID.to_string());
    }
    let addr = (ip, port)
        .to_socket_addrs()
        .ok()
        .and_then(|mut addrs| addrs.next())
        .ok_or_else(|| err_keys::PROXY_INVALID.to_string())?;

    let start = Instant::now();
    TcpStream::connect_timeout(&addr, TIMEOUT).map_err(|_| err_keys::PROXY_UNREACHABLE.to_string())?;
    Ok(start.elapsed().as_millis().min(u32::MAX as u128) as u32)
}
