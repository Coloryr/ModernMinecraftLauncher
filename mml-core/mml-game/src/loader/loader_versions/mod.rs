//! 加载器版本列表
//!
//! 添加实例窗口的"加载器版本"下拉数据源：按加载器类型，可选按游戏版本，
//! 拉取可用版本号列表，全部来自 mml-net 的对应 API / 镜像源。
//!
//! 子模块:
//!
//! | 模块 | 职责 |
//! | --- | --- |
//! | `meta` | 加载器 meta 与"该游戏版本能不能装"的判据 |
//! | `fetch` | 各加载器的版本拉取、缓存与支持列表查询 |
//! | `version` | 版本号比较与从版本号反推游戏版本 |

pub(crate) use std::collections::{HashMap, HashSet};
pub(crate) use std::sync::{Arc, LazyLock, Mutex, RwLock};

pub(crate) use regex::Regex;

pub(crate) use mml_base::serialize_tools;
pub(crate) use mml_config::config_obj::SourceLocal;
pub(crate) use mml_names::i18_items::error_type::{CoreResult, DataNotFoundData, ErrorType};

pub(crate) use crate::gui_hook::ProgressGui;
pub(crate) use crate::launcher_path::version_path;
pub(crate) use crate::loader::LoaderType;
pub(crate) use crate::loader::liteloader_meta_obj::LiteloaderMetaObj;

/// Forge 官方源全量版本缓存；maven-metadata.xml 较大，避免每次选择重复下载解析
static FORGE_META_ALL: LazyLock<RwLock<Option<Vec<String>>>> = LazyLock::new(|| RwLock::new(None));
/// Forge BMCLAPI 源按游戏版本的列表缓存
static FORGE_BMCLAPI: LazyLock<Mutex<HashMap<String, Vec<String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
/// NeoForge 官方源全量版本缓存，版本号 → 所属游戏版本分组，避免重复下载解析
static NEOFORGE_META_ALL: LazyLock<RwLock<Option<HashMap<String, Vec<String>>>>> =
    LazyLock::new(|| RwLock::new(None));
/// NeoForge BMCLAPI 源按游戏版本的列表缓存
static NEOFORGE_BMCLAPI: LazyLock<Mutex<HashMap<String, Vec<String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
/// Cleanroom 官方 maven 全量版本缓存；它只服务 1.12.2，列表不随游戏版本变化
static CLEANROOM_LIST: LazyLock<RwLock<Option<Vec<String>>>> = LazyLock::new(|| RwLock::new(None));
/// Fabric meta 缓存：加载器版本与支持的游戏版本
static FABRIC_META: LazyLock<RwLock<Option<Arc<LoaderMeta>>>> = LazyLock::new(|| RwLock::new(None));
/// Quilt meta 缓存：加载器版本与支持的游戏版本
static QUILT_META: LazyLock<RwLock<Option<Arc<LoaderMeta>>>> = LazyLock::new(|| RwLock::new(None));

mod fetch;
mod meta;
mod version;

pub use self::fetch::{SUPPORT_LOAD_STEPS, get_loader_versions, get_support_loaders};
pub(crate) use self::meta::{LoaderMeta, meta_supports};
pub(crate) use self::version::{forge_of_mc, neoforge_mc_version, value_versions, version_ge};

#[cfg(test)]
#[path = "../../../tests/unit/loader_loader_versions_tests.rs"]
mod tests;
