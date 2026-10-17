//! 添加资源窗口：从 CurseForge / Modrinth 浏览并获取资源
//!
//! **一个实例一个窗口**：窗口身份由实例 uuid 派生、打开时就锁定，窗口内不再选择实例，
//! 下载任务本就按实例索引，见 `windows/registry` 的窗口族。
//!
//! 与「下载整合包」窗口 `add_modpack` 分开：这里按资源类型走通用的
//! 项目与文件列表查询，模组 / 资源包 / 光影包等都是这条链路。

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{LazyLock, RwLock},
};

use mml_net::curseforge_api::file_obj::CurseForgeFileDataObj;
use mml_net::curseforge_api::list_obj::CurseForgeListDataObj;
use mml_net::modrinth_api::search_obj::HitObj;
use mml_net::modrinth_api::version_obj::ModrinthVersionObj;
use tokio::sync::RwLock as AsyncRwLock;
use uuid::Uuid;

pub(crate) mod dependency;
pub(crate) mod download;
pub(crate) mod list;
pub(crate) mod meta;
pub(crate) mod status;

// 下载器回调按原路径再导出：`windows/download.rs` 用的是
// `add_resource::on_download_item`，搬进子模块后调用点不必跟着改
pub(crate) use self::status::on_download_item;

/// 添加资源窗口上次选的下载源
///
/// # 返回值
///
/// 下载源线串；没选过时是空串，由界面落回列表第一个
#[tauri::command]
pub fn add_resource_last_source() -> String {
    crate::gui_config::get().resource_window.add_resource_source
}

/// 记住这次选的下载源，落 `gui_config.json`
///
/// - `source`: 下载源线串，来自列表接口
#[tauri::command]
pub fn add_resource_set_last_source(source: String) {
    let mut config = crate::gui_config::get();
    if config.resource_window.add_resource_source == source {
        return;
    }
    config.resource_window.add_resource_source = source;
    crate::gui_config::set(config);
}
/// CurseForge 项目列表缓存，pid 映射到项目数据，列表页请求时填充
pub(super) static CURSEFOGRE_INFO: LazyLock<AsyncRwLock<HashMap<String, CurseForgeListDataObj>>> =
    LazyLock::new(|| AsyncRwLock::new(HashMap::new()));
/// Modrinth 项目列表缓存，pid 映射到搜索命中项，列表页请求时填充
pub(super) static MODRINTH_INFO: LazyLock<AsyncRwLock<HashMap<String, HitObj>>> =
    LazyLock::new(|| AsyncRwLock::new(HashMap::new()));

/// CurseForge 文件缓存，先按 pid 再按 fid 取文件数据，文件列表页请求时填充
pub(super) static CURSEFOGRE_FILE: LazyLock<
    AsyncRwLock<HashMap<String, HashMap<String, CurseForgeFileDataObj>>>,
> = LazyLock::new(|| AsyncRwLock::new(HashMap::new()));
/// Modrinth 版本缓存，先按 pid 再按版本号取版本数据，文件列表页请求时填充
pub(super) static MODRINTH_FILE: LazyLock<
    AsyncRwLock<HashMap<String, HashMap<String, ModrinthVersionObj>>>,
> = LazyLock::new(|| AsyncRwLock::new(HashMap::new()));

/// 资源下载任务表，按实例 uuid 索引下载条目。
/// 下载器回调来自下载线程且是同步的，这里用 std 锁；命令侧临界区都很短。
pub(super) static DOWNLOAD_NOW: LazyLock<
    RwLock<HashMap<Uuid, HashMap<SourceInfo, SourceDownloadInfo>>>,
> = LazyLock::new(|| RwLock::new(HashMap::new()));

/// 这个实例有没有还没结束的资源下载
///
/// 添加资源窗口的下载按实例分表（见 [`DOWNLOAD_NOW`]）；"没结束"指还没落到
/// done / failed / cancelled 三个终态之一 —— 下载完到写回在线信息之间也算忙。
///
/// - `instance`: 实例 uuid
///
/// # 返回值
///
/// 有未结束的条目时返回 `true`
pub(crate) fn instance_busy(instance: &Uuid) -> bool {
    let map = DOWNLOAD_NOW.read().unwrap();
    map.get(instance).is_some_and(|entry| {
        entry
            .values()
            .any(|info| !(info.done || info.failed || info.cancelled))
    })
}

/// 下载源定位信息，pid + fid 唯一确定一个资源 / 文件
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct SourceInfo {
    /// 项目 ID
    pub pid: String,
    /// 文件 ID
    pub fid: String,
}

/// 资源下载条目，作为添加资源窗口顶部进度条的数据源
pub struct SourceDownloadInfo {
    /// 目标文件路径，下载器回调按它匹配进度
    pub file: PathBuf,
    /// 显示名
    pub name: String,
    /// 下载进度，取值 0.0–100.0
    pub now: f64,
    /// 下载完成，保留一段时间后从任务表移除
    pub done: bool,
    /// 下载失败
    pub failed: bool,
    /// 下载被取消，与失败互斥
    pub cancelled: bool,
}
