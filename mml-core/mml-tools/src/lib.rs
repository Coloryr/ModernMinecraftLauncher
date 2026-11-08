//! 启动器自带的工具链
//!
//! 统一放在启动器目录的 `tools` 下，整个启动器一份：换整合包、删实例都不会把这里的东西带走。
//! Python 运行环境见 [`python`]；Windows / Linux 上的 RenderDoc 见 [`renderdoc`]；
//! 本机安装的 Nsight Graphics 见 [`nsight`]。

pub mod nsight;
pub mod python;
#[cfg(any(windows, target_os = "linux"))]
pub mod renderdoc;

use std::path::PathBuf;

/// 工具链根目录，启动器目录下的 `tools`
///
/// # 返回值
///
/// 目录路径，不保证存在
pub fn tools_dir() -> PathBuf {
    mml_base::get_base_dir().join("tools")
}
