//! Modrinth 内容查询与整合包安装
//!
//! 子模块按职责划分：
//! - `convert`: 版本信息转下载项与在线文件信息
//! - `search`: 分类、加载器、游戏版本与作者头像查询
//! - `pack`: 整合包内文件解析
//! - `upgrade`: 整合包升级
//! - `deps`: 模组依赖递归解析
//! - `instance`: 实例在线信息的自动标记

pub(crate) use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Mutex, OnceLock},
};

pub(crate) use mml_base::{
    archives::{BaseArchive, BaseArchiveGui},
    file_item::{FileHash, FileItemObj, LaterRun},
    tools,
};
pub(crate) use mml_names::{
    i18_items::error_type::{CoreResult, ErrorType},
    names,
};
pub(crate) use mml_net::{
    modrinth_api::{
        self, ModrinthCategoriesObj,
        search_obj::HitObj,
        version_obj::{DependencieObj, ModrinthVersionObj},
    },
    urls,
};
pub(crate) use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
pub(crate) use tokio_util::sync::CancellationToken;

pub(crate) use crate::error::modrinth_avatar_err;
pub(crate) use crate::{
    GameInstance,
    data_res::DownloadItemRes,
    gui_hook::{AddModPackGui, AddModPackState},
    launcher::{
        FileType, file_online_info_obj::OnlineInfoObj, instance_setting_obj::InstanceSettingObj,
    },
    loader::LoaderType,
    modpack::{BaseModPackWorker, ModPackWorker, modrinth_worker::ModrinthPackWorker},
    modrinth::pack_obj::{ModrinthPackFileObj, ModrinthPackObj},
};

pub mod pack_obj;

/// 分类信息缓存
static CATEGORISE: OnceLock<Vec<ModrinthCategoriesObj>> = OnceLock::new();
/// 加载器名缓存，即 `tag/loader` 的名字表
static LOADERS: OnceLock<Vec<String>> = OnceLock::new();
/// 游戏版本列表缓存
static GAME_VERSIONS: OnceLock<Vec<String>> = OnceLock::new();

mod convert;
mod deps;
mod instance;
mod pack;
mod search;
mod upgrade;

pub use self::{
    convert::{make_download_obj, make_file_online_obj, make_pack_download_obj, to_loader_id},
    deps::{ModrinthModDependenciesRes, get_mod_dependencies},
    pack::get_mod_info,
    search::{author_avatars, get_categories, get_categories_icon, get_game_versions, order_tags},
    upgrade::upgrade_modpack,
};
