//! 下载整合包窗口的查询类命令：项目列表 / 详情 / 文件列表，文件即版本
//!
//! 从 `add_modpack/mod.rs` 拆出来的，光 `list_projects` 就有 118 行、`add_modpack_file` 132 行。
//! 列表页顺手把项目与文件写进 `CURSEFOGRE_*` / `MODRINTH_*` 缓存，详情与安装都从缓存取。
//! 命令带分组标记，值填原始组名 add_modpack，把组键钉回 `addModpack`，见 AGENTS.md §4。

use std::collections::HashSet;

use mml_game::launcher::{FileType, ModPackType};
use mml_net::curseforge_api::{self, CurseFogreArg, CurseForgeSortType};
use mml_net::modrinth_api::{self, ModrinthSearchArg, ModrinthSortType};
use tauri::WebviewWindow;

use crate::collect_utils;
use crate::dtos::add_resource_dto::{
    FileListDto, FileListItemDto, ProjectDetailDto, ProjectDto, ProjectItemDto,
};

use crate::err_keys;
use crate::windows::modpack_task::DOWNLOAD_NOW;

use super::{CURSEFOGRE_FILE, CURSEFOGRE_INFO, MODRINTH_FILE, MODRINTH_INFO, register_search};

/// 获取项目列表
///
/// - `window`: 调用方窗口，用于按窗口标签登记本发搜索
/// - `source`: 下载源标识，取值 `curseforge` / `modrinth`
/// - `page`: 页码，从 0 起
/// - `sort`: 排序方式 ID，取值由各下载源自己的排序枚举定
/// - `category`: 分类 ID，缺省为不限
/// - `filter`: 搜索词，缺省为不筛
/// - `version`: 游戏版本过滤，缺省为不限
///
/// # 返回值
///
/// 该页项目列表与总数；本发被新一发顶掉时返回 `err.cancelled`，下载源或排序非法返回 `Err`
#[gui_macros::ipc_group("add_modpack")]
#[tauri::command]
pub async fn add_modpack_list(
    window: WebviewWindow,
    source: String,
    page: u32,
    sort: String,
    category: Option<String>,
    filter: Option<String>,
    version: Option<String>,
) -> Result<ProjectDto, String> {
    // 同一窗口同时只跑一发：新的一发顶掉上一发，窗口关闭时由 `cancel_search` 全部取消
    let token = register_search(window.label());

    tokio::select! {
        res = list_projects(source, page, sort, category, filter, version) => res,
        _ = token.cancelled() => Err(String::from(err_keys::CANCELLED)),
    }
}

/// 按下载源拉取项目列表
///
/// 顺手把项目元数据写进 `CURSEFOGRE_INFO` / `MODRINTH_INFO`，详情与安装都从缓存取。
///
/// - `source`: 下载源标识，取值 `curseforge` / `modrinth`
/// - `page`: 页码，从 0 起
/// - `sort`: 排序方式 ID，取值由各下载源自己的排序枚举定
/// - `category`: 分类 ID，缺省为不限
/// - `filter`: 搜索词，缺省为不筛
/// - `version`: 游戏版本过滤，缺省为不限
///
/// # 返回值
///
/// 该页项目列表与总数；下载源或排序非法、请求失败返回 `Err`
pub(super) async fn list_projects(
    source: String,
    page: u32,
    sort: String,
    category: Option<String>,
    filter: Option<String>,
    version: Option<String>,
) -> Result<ProjectDto, String> {
    let source = ModPackType::from_string(&source);

    match source {
        ModPackType::CurseForge => {
            let sort = CurseForgeSortType::from_string(&sort);
            if sort.is_none() {
                return Err(String::from(err_keys::SORT_TYPE_NOT_FOUND));
            }
            let sort = sort.unwrap();

            let list = curseforge_api::get_modpack_list(CurseFogreArg {
                version,
                page: Some(page),
                sort: Some(sort),
                filter,
                category,
                ..Default::default()
            })
            .await
            .map_err(|err| err.to_string())?;

            let mut list1 = Vec::new();

            // 整页只构建一次集合，循环里只查表
            let installed = installed_modpack_pids();
            let running = running_pids();

            let mut map = CURSEFOGRE_INFO.write().await;

            for item in list.data {
                let pid = item.id.to_string();
                let temp = ProjectItemDto::new_curseforge(
                    &item,
                    FileType::Modpack,
                    installed.contains(&pid),
                    true,
                    collect_utils::is_star(&pid),
                    running.contains(&pid),
                    None,
                );

                list1.push(temp);
                map.insert(pid, item);
            }

            Ok(ProjectDto {
                items: list1,
                count: list.pagination.total_count,
            })
        }
        ModPackType::Modrinth => {
            let sort = ModrinthSortType::from_string(&sort);
            if sort.is_none() {
                return Err(String::from(err_keys::SORT_TYPE_NOT_FOUND));
            }
            let sort = sort.unwrap();

            let list = modrinth_api::get_modpack_list(ModrinthSearchArg {
                page: Some(page),
                query: filter,
                category,
                sort,
                version,
                ..Default::default()
            })
            .await
            .map_err(|err| err.to_string())?;

            let mut list1 = Vec::new();

            // 同 CurseForge：整页一次集合构建
            let installed = installed_modpack_pids();
            let running = running_pids();

            // 与上面 CurseForge 分支同理：构建 DTO 期间不持锁，new_modrinth 会
            // await 查标签图标；攒完一次性写入缓存。
            let mut cached = Vec::new();

            for item in list.hits {
                let pid = item.project_id.clone();
                let temp = ProjectItemDto::new_modrinth(
                    &item,
                    FileType::Modpack,
                    installed.contains(&pid),
                    true,
                    collect_utils::is_star(&pid),
                    running.contains(&pid),
                    None,
                )
                .await;

                list1.push(temp);
                cached.push((pid, item));
            }

            {
                let mut map = MODRINTH_INFO.write().await;
                for (pid, item) in cached {
                    map.insert(pid, item);
                }
            }

            Ok(ProjectDto {
                items: list1,
                count: list.total_hits,
            })
        }
        _ => Err(String::from(err_keys::SOURCE_TYPE)),
    }
}

/// 获取项目详情，双击列表项时调用
///
/// Modrinth：project 拿 markdown 正文与截图，team 拿作者头像；
/// CurseForge：只有简介 + 截图，优先用列表缓存，缓存未命中再查 mod_info
///
/// - `source`: 下载源标识，取值 `curseforge` / `modrinth`
/// - `pid`: 项目 ID
///
/// # 返回值
///
/// 项目详情，含简介、正文与截图；下载源非法或请求失败返回 `Err`
#[gui_macros::ipc_group("add_modpack")]
#[tauri::command]
pub async fn add_modpack_detail(source: String, pid: String) -> Result<ProjectDetailDto, String> {
    let source = ModPackType::from_string(&source);

    match source {
        ModPackType::CurseForge => {
            // 列表时已缓存了 mod 数据，直接借用构造；未命中才发请求
            let detail = {
                let map = CURSEFOGRE_INFO.read().await;
                map.get(&pid).map(ProjectDetailDto::new_curseforge)
            };
            match detail {
                Some(detail) => Ok(detail),
                None => {
                    let data = curseforge_api::get_mod_info(&pid)
                        .await
                        .map_err(|err| err.to_string())?;
                    Ok(ProjectDetailDto::new_curseforge(&data.data))
                }
            }
        }
        ModPackType::Modrinth => {
            let data = modrinth_api::get_project(&pid)
                .await
                .map_err(|err| err.to_string())?;

            ProjectDetailDto::new_modrinth(&data)
                .await
                .map_err(|err| err.to_string())
        }
        _ => Err(String::from(err_keys::SOURCE_TYPE)),
    }
}

/// 获取文件列表
///
/// 一页 50 个项目；CurseForge 换页要再查，Modrinth 一次返回全部，`max_page` 按 50 折算。
///
/// - `source`: 下载源标识，取值 `curseforge` / `modrinth`
/// - `pid`: 项目 ID
/// - `page`: 页码，从 0 起；只有 CurseForge 用得上
/// - `version`: 游戏版本过滤，缺省为不限
///
/// # 返回值
///
/// 该页文件列表、总数与项目名；下载源非法或请求失败返回 `Err`
#[gui_macros::ipc_group("add_modpack")]
#[tauri::command]
pub async fn add_modpack_file(
    source: String,
    pid: String,
    page: u32,
    version: Option<String>,
) -> Result<FileListDto, String> {
    let source = ModPackType::from_string(&source);

    match source {
        ModPackType::CurseForge => {
            // 项目名优先取列表页 / 详情页的缓存：只为拿个名字再查一次 mod_info，
            // 每次翻页都白多一个 HTTP 往返
            let cached = CURSEFOGRE_INFO
                .read()
                .await
                .get(&pid)
                .map(|item| item.name.clone());
            let name = match cached {
                Some(name) => name,
                None => {
                    let data = curseforge_api::get_mod_info(&pid)
                        .await
                        .map_err(|err| err.to_string())?;
                    let name = data.data.name.clone();
                    // 回填缓存：详情页与安装时的显示名都从这里取
                    CURSEFOGRE_INFO.write().await.insert(pid.clone(), data.data);
                    name
                }
            };

            let list = curseforge_api::get_files_page(CurseFogreArg {
                id: Some(pid.clone()),
                version,
                page: Some(page),
                ..Default::default()
            })
            .await
            .map_err(|err| err.to_string())?;

            let mut list1 = Vec::new();

            {
                // 版本列表同样一次构建集合
                let installed = installed_modpack_files();
                let running = running_files();
                let mut lock = CURSEFOGRE_FILE.write().await;
                let list2 = lock.entry(pid.clone()).or_default();

                for item in list.data {
                    let fid = item.id.to_string();
                    let key = PackFileId {
                        pid: pid.clone(),
                        fid: fid.clone(),
                    };
                    list1.push(FileListItemDto::new_curseforge(
                        &item,
                        FileType::Modpack,
                        installed.contains(&key),
                        running.contains(&key),
                    ));
                    list2.insert(fid, item);
                }
            }

            Ok(FileListDto {
                list: list1,
                count: list.pagination.total_count,
                name,
                max_page: list.pagination.total_count / 50,
            })
        }
        ModPackType::Modrinth => {
            // 同上：列表页缓存里已有项目名 title，未命中才查 project。
            // project 是 ModrinthProjectObj、与缓存的 HitObj 不同型，故不回填
            let cached = MODRINTH_INFO
                .read()
                .await
                .get(&pid)
                .map(|item| item.title.clone());
            let name = match cached {
                Some(name) => name,
                None => {
                    modrinth_api::get_project(&pid)
                        .await
                        .map_err(|err| err.to_string())?
                        .title
                }
            };

            let list = modrinth_api::get_file_versions(
                &pid,
                match version.as_ref() {
                    Some(data) => Some(data),
                    None => None,
                },
                None,
            )
            .await
            .map_err(|err| err.to_string())?;

            let mut list1 = Vec::new();

            let len = list.len() as u64;

            {
                let installed = installed_modpack_files();
                let running = running_files();
                let mut lock = MODRINTH_FILE.write().await;
                let list2 = lock.entry(pid.clone()).or_default();

                for item in list {
                    let key = PackFileId {
                        pid: pid.clone(),
                        fid: item.id.clone(),
                    };
                    // 没有文件可下的版本直接跳过，见 new_modrinth 的注释
                    if let Some(dto) = FileListItemDto::new_modrinth(
                        &item,
                        FileType::Modpack,
                        installed.contains(&key),
                        running.contains(&key),
                    ) {
                        list1.push(dto);
                    }
                    list2.insert(item.id.to_string(), item);
                }
            }

            Ok(FileListDto {
                list: list1,
                count: len,
                name,
                max_page: len / 50,
            })
        }
        _ => Err(String::from(err_keys::SOURCE_TYPE)),
    }
}

/// 已安装过的整合包 pid 集合，整页列表共用一次构建
///
/// 逐项调用会让每个列表项各自遍历一遍实例表并加一次锁；一页 20 项就是 20 遍。
///
/// # 返回值
///
/// 已安装整合包的项目 ID 集合；非整合包实例不计入
pub(super) fn installed_modpack_pids() -> HashSet<String> {
    mml_game::get_instances()
        .iter()
        .filter_map(|item| {
            let temp = item.read().unwrap();
            if temp.is_modpack {
                temp.pid.clone()
            } else {
                None
            }
        })
        .collect()
}

/// 整合包的项目 ID 与文件 ID 组合
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct PackFileId {
    /// 项目 ID
    pub(super) pid: String,
    /// 文件 ID
    pub(super) fid: String,
}

/// 已安装过的整合包 pid + fid 组合集合，版本列表共用
///
/// # 返回值
///
/// 已安装整合包的项目 ID 与文件 ID 组合；缺 pid 或 fid 的实例跳过
pub(super) fn installed_modpack_files() -> HashSet<PackFileId> {
    mml_game::get_instances()
        .iter()
        .filter_map(|item| {
            let temp = item.read().unwrap();
            if !temp.is_modpack {
                return None;
            }
            match (temp.pid.clone(), temp.fid.clone()) {
                (Some(pid), Some(fid)) => Some(PackFileId { pid, fid }),
                _ => None,
            }
        })
        .collect()
}

/// 正在安装的 pid 集合，供列表角标显示该项目任一版本在装
///
/// # 返回值
///
/// 有安装任务在跑的项目 ID 集合
pub(super) fn running_pids() -> HashSet<String> {
    DOWNLOAD_NOW
        .read()
        .unwrap()
        .keys()
        .map(|key| key.pid.clone())
        .collect()
}

/// 正在安装的 pid + fid 组合集合，供版本列表角标
///
/// # 返回值
///
/// 有安装任务在跑的项目 ID 与文件 ID 组合
pub(super) fn running_files() -> HashSet<PackFileId> {
    DOWNLOAD_NOW
        .read()
        .unwrap()
        .keys()
        .map(|key| PackFileId {
            pid: key.pid.clone(),
            fid: key.fid.clone(),
        })
        .collect()
}
