//! Java 运行时管理模块
//!
//! 本模块负责管理启动器中配置的 Java 运行时环境，即 JRE / JDK。
//!
//! # 核心功能
//!
//! - **自动扫描** — Windows 查系统注册表，Linux / macOS 查标准路径
//! - **手动添加** — 用户可手动指定 Java 可执行文件路径
//! - **版本匹配** — 根据 Minecraft 版本自动选择兼容的 Java 版本
//! - **架构匹配** — 自动过滤与系统架构一致的 Java，即 x86_64 / aarch64
//! - **变更通知** — 通过事件回调通知 UI 层 Java 列表已变更
//!
//! # 文件结构
//!
//! - `model`: Java 运行时信息类型
//! - `state`: 全局状态与变更事件
//! - `list`: 列表增删查与配置同步
//! - `scan`: 系统 Java 扫描
//! - `install`: 压缩包解压与目录注册
//! - `java_helper`: 可执行文件探测

pub(crate) use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, OnceLock, RwLock},
};

pub(crate) use mml_base::{
    archives::{self, ArchiveType, BaseArchiveGui},
    events::EventHandler,
};
pub(crate) use mml_config::config_obj::JvmConfigObj;
pub(crate) use mml_names::{
    i18_items::error_type::{CoreResult, ErrorType},
    names,
};
pub(crate) use mml_sys::{ArchEnum, Os, java_scan_helper, path_helper};

mod install;
mod list;
mod model;
mod scan;
mod state;

pub mod java_helper;

pub(crate) use self::state::{JAVA_DIR, JVMS, invoke_jvm_change};
pub use self::{
    install::{extract_java, register_java_dir, unzip_java},
    list::{
        add_info_item, add_info_items, add_item, get_all_java, get_java, get_java_info, load,
        remove, remove_all,
    },
    model::JavaInfoObj,
    scan::{find_all_java_from_path, find_java_from_path, scan_java},
    state::{add_jvm_change, get_java_dir, init, remove_jvm_change},
};

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
#[path = "../tests/unit/lib_tests.rs"]
mod tests;
