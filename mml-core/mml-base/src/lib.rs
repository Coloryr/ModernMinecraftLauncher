//! 启动器基础库模块
//!
//! 本模块是启动器最底层的公共库，提供了所有其他模块共享的基础设施：
//!
//! # 核心功能
//!
//! - **序列化工具** — JSON/TOML 的解析和序列化，自定义反序列化器
//! - **哈希计算** — MD5/SHA1/SHA256/SHA512 及 Base64 编解码
//! - **压缩包处理** — Zip/7z/Tar/TarGz/TarXz 的压缩和解压
//! - **事件系统** — 全局事件发布订阅，分带参数与无参数两种
//! - **字符串工具** — 数字/英文数字格式校验、字符串截取、路径拆分、命令行参数解析
//! - **版本号解析** — Minecraft 版本号的解析与比较
//!
//! # 子模块
//!
//! | 模块 | 用途 |
//! |------|------|
//! | [`serialize_tools`] | JSON/TOML 序列化 |
//! | [`hash_helper`] | 哈希和 Base64 |
//! | [`archives`] | 压缩包处理 |
//! | [`events`] | 事件发布订阅 |
//! | [`inner_path`] | 内部数据存储路径 |
//! | [`file_item`] | 文件下载项定义 |
//! | [`tools`] | 字符串/路径/命令行参数工具 |
//! | [`version_parse`] | Minecraft 版本号解析 |

pub mod archives;
pub mod events;
pub mod file_item;
pub mod hash_helper;
pub mod inner_path;
pub mod serialize_tools;
pub mod tools;
pub mod version_parse;

use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

/// 程序运行根目录，进程级全局单例
static BASE_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 初始化程序运行根目录
///
/// 应在程序启动时调用一次，设置后可通过 [`get_base_dir()`] 获取。
///
/// - `dir`: 程序运行目录
pub fn init<P: AsRef<Path>>(dir: P) {
    BASE_DIR.get_or_init(|| dir.as_ref().to_path_buf());
}

/// 获取程序运行根目录
///
/// # 返回值
///
/// 返回 [`init`] 传入的目录；尚未调用 [`init`] 时 panic
pub fn get_base_dir() -> PathBuf {
    BASE_DIR.get().unwrap().clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// init / get_base_dir 全局根目录
    ///
    /// BASE_DIR 是进程级全局单例，首次调用的参数生效，重复 init 不 panic 也不覆盖
    #[test]
    fn test_init_and_get_base_dir() {
        let dir = mml_testutil::temp_dir().join(format!(
            "mml_base_init_test_{}_{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        init(&dir);
        assert_eq!(get_base_dir(), dir);

        // 重复调用不应 panic，且保留第一次的值
        init("/mml_base_should_not_override");
        assert_eq!(get_base_dir(), dir);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
