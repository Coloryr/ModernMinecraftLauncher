//! 手柄输入：读取手柄、按映射生成键鼠消息、经 IPC 转发给游戏实例
//!
//! 配置放在运行目录的 gamepad 目录下，一份映射一个 json，Selected 为 true 的那份生效；
//! 目录里没有可用配置时用内置默认。启动入口是 init：建目录、读配置，再起读取与
//! 转发两条线程。
//!
//! 转发目标由 mml_ipc 里已注册的游戏通道决定：只有一个游戏连接时直接用它，
//! 多开时优先已抓取鼠标的那个。

mod map;
mod reader;
mod relay;

use std::{path::Path, sync::Once};

pub use map::{
    Axis, AxisRule, Button, ButtonRule, GamepadConfig, GamepadError, MouseRule, ProfileInfo, Stick,
    action_names, config, create, delete, profiles, save, set_active,
};
pub use reader::{AxisValue, ButtonValue, PadInfo, PadState, pad_state, pads};

/// 启动只做一次
static INIT: Once = Once::new();

/// 启动手柄读取与输入转发
///
/// - `base`: 运行目录，配置目录是其中的 gamepad
pub fn init(base: &Path) {
    INIT.call_once(|| {
        map::init(base);
        reader::start();
        relay::start();
    });
}
