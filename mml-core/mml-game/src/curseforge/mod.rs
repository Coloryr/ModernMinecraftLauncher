//! CurseForge 内容查询与整合包安装
//!
//! 子模块按职责划分：
//! - `convert`: 排序字段、加载器 ID 与版本信息转下载项
//! - `search`: 分类与游戏版本查询
//! - `deps`: 模组依赖递归解析
//! - `pack`: 整合包文件信息获取与条目路径解析
//! - `upgrade`: 整合包升级

use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    path::Path,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use tokio_util::sync::CancellationToken;

use crate::{
    GameInstance,
    curseforge::pack_obj::CurseForgePackObj,
    data_res::{DownloadItemRes, ItemPathRes},
    gui_hook::{AddModPackGui, AddModPackState},
    launcher::{
        FileType, file_online_info_obj::OnlineInfoObj, instance_setting_obj::InstanceSettingObj,
    },
    loader::LoaderType,
    modpack::{BaseModPackWorker, ModPackWorker, curseforge_worker::CurseForgeWorker},
};
use mml_base::{
    archives::{BaseArchive, BaseArchiveGui},
    file_item::{FileHash, FileItemObj, LaterRun},
};
use mml_names::{
    i18_items::error_type::{CoreResult, ErrorType},
    names,
};
use mml_net::curseforge_api::{
    self, CurseFogreArg,
    categories_obj::CurseForgeCategoriesObj,
    file_obj::{CurseForgeFileDataObj, CurseForgeFileObj, DependenciesObj},
    version_obj::{CurseForgeVersionDataObj, CurseForgeVersionTypeDataObj},
};

pub mod pack_obj;

/// 分类信息缓存
static CATEGORIES: OnceLock<CurseForgeCategoriesObj> = OnceLock::new();
/// 游戏版本列表缓存
static GAME_VERSION: OnceLock<Vec<String>> = OnceLock::new();

mod convert;
mod deps;
mod pack;
mod search;
mod upgrade;

pub use self::{
    convert::{CurseForgeSortField, make_file_item_obj, make_file_online_info_obj, to_loader_id},
    deps::{CurseForgeModDependenciesRes, get_mod_dependencies},
    pack::get_modpack_info,
    search::{get_categories, get_game_versions},
    upgrade::upgrade_modpack,
};
