use crate::*;

use crate::broadcast::send_to_game;

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
