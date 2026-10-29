//! 内部数据存储路径模块
//!
//! 根据操作系统自动确定启动器的内部数据存储目录：
//!
//! | 平台 | 路径 |
//! |------|------|
//! | Windows | `%LOCALAPPDATA%\mml\` |
//! | Linux / macOS | `$HOME/.local/share/mml/` |
//!
//! macOS 与 Linux 同一口径：内部数据落在当前账户的 `~/.local/share`。
//! 运行目录在 macOS 上也改成了 `~/.mml`，两者分开才不会把账户文件写进运行目录。
//!
//! 目录在首次访问时自动创建。

use std::{env, fs, path::PathBuf, sync::LazyLock};

/// 内部数据存储路径，惰性初始化并自动创建目录
static INNER: LazyLock<PathBuf> = LazyLock::new(|| {
    let local_app_data = env::var("LOCALAPPDATA")
        .or_else(|_| env::var("HOME").map(|h| format!("{}/.local/share", h)))
        .expect("");
    let inner_path = PathBuf::from(local_app_data).join("mml");

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
