//! 整合包搜索
//!
//! 搜索结果按当前实例的游戏版本过滤，在线列表只回一页；
//! 已安装的项目在结果里带上本地状态。


use mml_game::launcher::{FileType, ModPackType};
use mml_game::modrinth;
use mml_net::curseforge_api::{self, CurseFogreArg, CurseForgeSortType};
use mml_net::modrinth_api::{self, ModrinthSearchArg, ModrinthSortType};
use tauri::WebviewWindow;

use crate::collect_utils;
use crate::dtos::add_resource_dto::{ProjectDto, ProjectItemDto};

use crate::err_keys;

use crate::windows::add_modpack::{CURSEFOGRE_INFO, MODRINTH_INFO, register_search};
use super::installed::{installed_modpack_pids, running_pids};


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
#[gui_macros::command]
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
pub(crate) async fn list_projects(
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

            // 搜索结果不给作者头像：按 author_id 批量查一次，一页一次请求；
            // 查不到就让前端退回名字首字母的占位
            let avatars = modrinth::author_avatars(&list.hits).await;

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
                    avatars.get(&item.author_id).cloned(),
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

/// 整合包的项目 ID 与文件 ID 组合
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PackFileId {
    /// 项目 ID
    pub(crate) pid: String,
    /// 文件 ID
    pub(crate) fid: String,
}
