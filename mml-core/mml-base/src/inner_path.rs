//! 内部数据存储路径模块
//!
//! 根据操作系统自动确定启动器的内部数据存储目录：
//!
//! | 平台 | 路径 |
//! |------|------|
//! | Windows | `%LOCALAPPDATA%\mml\` |
//! | Linux | `$HOME/.local/share/mml/` |
//! | macOS | `$HOME/.mml/` |
//!
//! 目录在首次访问时自动创建。

use std::{env, fs, path::PathBuf, sync::LazyLock};

use mml_sys::Os;

/// 内部数据存储路径，惰性初始化并自动创建目录
static INNER: LazyLock<PathBuf> = LazyLock::new(|| {
    let inner_path = if mml_sys::get_system_info().os == Os::MacOS {
        let home = env::var("HOME").expect("");
        PathBuf::from(home).join(".mml")
    } else {
        let local_app_data = env::var("LOCALAPPDATA")
            .or_else(|_| env::var("HOME").map(|h| format!("{}/.local/share", h)))
            .expect("");
        PathBuf::from(local_app_data).join("mml")
    };

    if !inner_path.exists() {
        fs::create_dir_all(&inner_path).expect("");
    }

    inner_path
});

/// 获取内部数据存储路径
///
/// # 返回值
///
/// 返回进程内缓存的同一路径，首次调用时创建目录
pub fn get_inner_path() -> PathBuf {
    INNER.clone()
}
