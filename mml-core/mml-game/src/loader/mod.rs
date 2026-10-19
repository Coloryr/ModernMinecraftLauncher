//! 模组加载器安装
//!
//! 子模块:
//!
//! | 模块 | 职责 |
//! | --- | --- |
//! | `custom` | 自定义加载器 |
//! | `fabric` / `fabric_loader_obj` / `fabric_meta_obj` | Fabric 安装与元数据 |
//! | `forge` / `forge_install_obj` / `forge_launch_obj` | Forge / NeoForge 安装与启动数据 |
//! | `liteloader` / `liteloader_meta_obj` | LiteLoader 安装与元数据 |
//! | `optifine` / `optifine_obj` | OptiFine 安装与元数据 |
//! | `quilt` / `quilt_loader_obj` / `quilt_meta_obj` | Quilt 安装与元数据 |
//! | `loader_versions` | 各加载器可用版本查询 |

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

pub mod custom;
pub mod fabric;
pub mod fabric_loader_obj;
pub mod fabric_meta_obj;
pub mod forge;
pub mod forge_install_obj;
pub mod forge_launch_obj;
pub mod liteloader;
pub mod liteloader_meta_obj;
pub mod loader_versions;
pub mod optifine;
pub mod optifine_obj;
pub mod quilt;
pub mod quilt_loader_obj;
pub mod quilt_meta_obj;

/// 加载器版本键，用于按游戏版本与加载器版本缓存数据
#[derive(Hash, Eq, PartialEq, Debug, Clone)]
pub struct LoaderKey {
    /// 游戏版本
    pub mc: String,
    /// 加载器版本
    pub version: String,
}

impl LoaderKey {
    /// 创建版本键
    ///
    /// - `mc`: 游戏版本
    /// - `version`: 加载器版本
    ///
    /// # 返回值
    ///
    /// 返回新建的版本键
    pub fn new(mc: &str, version: &str) -> Self {
        LoaderKey {
            mc: String::from(mc),
            version: String::from(version),
        }
    }
}

/// 运行库下载信息
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct LibrariesObj {
    /// Maven 坐标
    pub name: String,
    /// 下载源地址
    pub url: String,
}

/// 模组加载器类型，按变体顺序序列化为 `u8`
#[derive(Serialize_repr, Deserialize_repr, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LoaderType {
    /// 无模组加载器
    Normal,
    /// Forge加载器
    Forge,
    /// Fabric加载器
    Fabric,
    /// Quilt加载器
    Quilt,
    /// NeoForge加载器
    NeoForge,
    /// 高清修复
    OptiFine,
    /// LiteLoader
    LiteLoader,
    /// 自定义
    Custom,
}

impl Default for LoaderType {
    fn default() -> Self {
        LoaderType::Normal
    }
}

impl LoaderType {
    /// 加载器独立 ID，与前端 i18n 键对应，不随语言变化
    ///
    /// # 返回值
    ///
    /// 返回该加载器的 ID 字符串
    pub fn id(&self) -> &'static str {
        match self {
            LoaderType::Normal => "normal",
            LoaderType::Forge => "forge",
            LoaderType::Fabric => "fabric",
            LoaderType::Quilt => "quilt",
            LoaderType::NeoForge => "neoforge",
            LoaderType::OptiFine => "optifine",
            LoaderType::LiteLoader => "liteloader",
            LoaderType::Custom => "custom",
        }
    }

    /// 按 ID 解析加载器类型
    ///
    /// - `id`: 加载器 ID，形如 `normal`
    ///
    /// # 返回值
    ///
    /// 未知 ID 返回 `None`
    pub fn from_string(id: &str) -> Option<Self> {
        match id {
            "normal" => Some(LoaderType::Normal),
            "forge" => Some(LoaderType::Forge),
            "fabric" => Some(LoaderType::Fabric),
            "quilt" => Some(LoaderType::Quilt),
            "neoforge" => Some(LoaderType::NeoForge),
            "optifine" => Some(LoaderType::OptiFine),
            "liteloader" => Some(LoaderType::LiteLoader),
            "custom" => Some(LoaderType::Custom),
            _ => None,
        }
    }

    /// 全部加载器 ID，作为下拉列表数据源
    ///
    /// # 返回值
    ///
    /// 返回按展示顺序排列的全部 ID
    pub fn ids() -> Vec<&'static str> {
        vec![
            "normal",
            "forge",
            "fabric",
            "quilt",
            "neoforge",
            "optifine",
            "liteloader",
            "custom",
        ]
    }
}
