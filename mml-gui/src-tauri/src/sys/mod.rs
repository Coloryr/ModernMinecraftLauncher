//! 系统集成
//!
//! 托盘、通知、单开与唤回、启动参数、macOS 菜单、致命错误弹窗，以及音乐自定义协议。
//! 这些模块直接碰操作系统或 webview 运行时，不持有业务数据。

pub mod build_info;
pub mod err_box;
// 菜单只在 macOS 上真正挂出去：其余平台的构建照样编译它，只是这些条目没有调用点
#[cfg_attr(not(target_os = "macos"), allow(dead_code, unused_imports))]
pub(crate) mod menu;
pub mod music;
pub mod notify;
pub mod single_instance;
pub mod start_arg;
pub mod tray;
