//! 启动器自带的工具链
//!
//! 统一放在启动器目录的 `tools` 下，整个启动器一份：换整合包、删实例都不会把这里的东西带走。
//! 目前只有 Python 运行环境，见 [`python`]。

pub mod python;

use std::path::PathBuf;

/// 工具链根目录，启动器目录下的 `tools`
///
/// # 返回值
///
/// 目录路径，不保证存在
pub fn tools_dir() -> PathBuf {
    mml_base::get_base_dir().join("tools")
}
