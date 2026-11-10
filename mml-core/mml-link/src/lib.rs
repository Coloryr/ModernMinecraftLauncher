//! 资源链接 实验性功能
//! 模组和材质包支持文件硬链接
//! 可以将 download/mods 下的模组文件硬链接到实例 mods 下；
//! 每个实例的映射记录写在实例目录的 link_info.json

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        LazyLock, Mutex, OnceLock, RwLock,
        atomic::{AtomicBool, Ordering},
    },
};

use uuid::Uuid;

use crate::info::{LinkInfoObj, ResourceType};

mod index_store;
pub mod info;

pub use index_store::{IndexKind, Store};

mod index;
mod link;
mod plan;
mod util;

/// 公共模组目录
static MOD_DOWNLOAD_PATH: OnceLock<PathBuf> = OnceLock::new();
/// 公共资源包目录
static RESOURCE_PACK_DOWNLOAD_PATH: OnceLock<PathBuf> = OnceLock::new();
/// 公共索引库文件，模组与资源包共用
static INDEX_PATH: OnceLock<PathBuf> = OnceLock::new();
/// 公共索引库；打不开时为空，索引退化成只在内存里
static INDEX: OnceLock<Store> = OnceLock::new();

/// 资源映射总开关，由 GUI 配置同步
static ENABLED: AtomicBool = AtomicBool::new(false);

/// 游戏实例链接资源文件存储
static LINK_DATA: OnceLock<RwLock<HashMap<Uuid, Vec<LinkInfoObj>>>> = OnceLock::new();
/// 待写盘的映射记录：实例 uuid 与根目录
static LINK_DIRTY: LazyLock<Mutex<HashSet<(Uuid, PathBuf)>>> = LazyLock::new(Default::default);

/// 下载到公共目录后补链所需的全部信息
#[derive(Clone, Debug)]
pub struct LinkPlan {
    /// 资源类型
    pub resource: ResourceType,
    /// 实例 uuid
    pub instance: Uuid,
    /// 实例根目录
    pub base: PathBuf,
    /// 实例内目标文件
    pub target: PathBuf,
    /// 公共目录里的源文件
    pub source: PathBuf,
    /// 项目 ID
    pub pid: String,
    /// 文件 ID
    pub fid: String,
    /// 显示名
    pub name: String,
    /// 文件 SHA1
    pub hash: String,
    /// 下载地址
    pub url: String,
}

impl ResourceType {
    /// 公共存储目录
    fn download_dir(self) -> &'static PathBuf {
        match self {
            Self::Mod => MOD_DOWNLOAD_PATH.get().unwrap(),
            Self::ResourcePack => RESOURCE_PACK_DOWNLOAD_PATH.get().unwrap(),
        }
    }

    /// 索引库里的资源类型
    fn kind(self) -> IndexKind {
        match self {
            Self::Mod => IndexKind::Mod,
            Self::ResourcePack => IndexKind::ResourcePack,
        }
    }
}

/// 设置资源映射总开关
///
/// - `enabled`: 是否启用；关闭后只保留已映射文件，不再处理新下载
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

/// 资源映射是否启用
///
/// # 返回值
///
/// 启用时为 `true`
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub use self::index::*;
pub use self::link::*;
pub use self::plan::*;
pub(crate) use self::util::*;

pub(crate) use self::index::log_index_error;

#[cfg(test)]
#[path = "../tests/unit/lib_tests.rs"]
mod tests;
