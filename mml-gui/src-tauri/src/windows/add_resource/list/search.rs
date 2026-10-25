//! 资源搜索
//!
//! 搜索走内核的搜索入口，带当前实例的游戏版本与加载器；
//! 只回一页结果，翻页由前端再调一次。

use mml_game::launcher::{FileType, ModPackType};
use mml_game::{curseforge, modrinth};
use mml_names::i18_items::error_type::ErrorType;
use mml_net::curseforge_api::{self, CurseFogreArg, CurseForgeSortType};
use mml_net::modrinth_api::{self, ModrinthSearchArg, ModrinthSortType};

use crate::collect_utils;
use crate::dtos::add_resource_dto::{ProjectDto, ProjectItemDto};
use crate::err_keys;

use super::query::{ListQuery, check_download_now, parse_query};
use crate::windows::add_resource::{CURSEFOGRE_INFO, MODRINTH_INFO};

/// 获取项目列表，支持搜索与分类翻页
///
/// 参数就是 IPC 契约，见 AGENTS.md §4：`bindings.ts` 由 Rust 源码生成，
/// 不能为了少几个参数把它们并成结构体 —— 那等于改前端调用方式。
///
/// - `game`: 实例 uuid，用来标记"已下载 / 正在下载 / 已收藏"
/// - `source`: 下载源，取 `curseforge` 或 `modrinth`
/// - `file_type`: 资源类型，取 `mod` / `save` / `resourcepack` / `shaderpack` / `datapacks`
/// - `page`: 页码，从 0 开始
/// - `sort`: 排序方式，取值来自 `add_resource_sort_type`
/// - `category`: 分类过滤，`None` 表示不限
/// - `filter`: 搜索关键字，`None` 表示不限
/// - `version`: 游戏版本过滤，`None` 表示不限
/// - `loader`: 模组加载器过滤，仅模组类型有意义，缺省 / 非法值按 normal
///
/// # 返回值
///
/// 该页项目列表与命中总数；源 / 排序不受支持或实例不存在时返回 `Err`
#[allow(clippy::too_many_arguments)]
#[gui_macros::ipc_group("add_resource")]
#[gui_macros::command]
pub async fn add_resource_list(
    game: String,
    source: String,
    file_type: String,
    page: u32,
    sort: String,
    category: Option<String>,
    filter: Option<String>,
    version: Option<String>,
    loader: Option<String>,
) -> Result<ProjectDto, String> {
    let ListQuery {
        uuid,
        info,
        source,
        file_type,
        mod_loader,
    } = parse_query(&game, &source, &file_type, loader.as_deref())?;

    match source {
        ModPackType::CurseForge => {
            let Some(sort) = CurseForgeSortType::from_string(&sort) else {
                return Err(String::from(err_keys::SORT_TYPE_NOT_FOUND));
            };

            let arg = CurseFogreArg {
                version,
                page: Some(page),
                sort: Some(sort),
                filter,
                category,
                loader: curseforge::to_loader_id(&mod_loader),
                ..Default::default()
            };

            let list = match file_type {
                FileType::Mod => curseforge_api::get_mod_list(arg).await,
                FileType::Save => curseforge_api::get_save_list(arg).await,
                FileType::Resourcepack => curseforge_api::get_resourcepack_list(arg).await,
                FileType::Shaderpack => curseforge_api::get_shaders_list(arg).await,
                FileType::DataPacks => curseforge_api::get_datapacks_list(arg).await,
                _ => Err(ErrorType::InvalidOperation),
            }
            .map_err(|err| err.to_string())?;

            let mut list1 = Vec::new();

            let mut map = CURSEFOGRE_INFO.write().await;

            for item in list.data {
                let pid = item.id.to_string();
                let info = info.get(&pid);

                let temp = ProjectItemDto::new_curseforge(
                    &item,
                    file_type,
                    info.is_some(),
                    true,
                    collect_utils::is_star(&pid),
                    check_download_now(uuid.as_ref(), &item.id.to_string()),
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
            let Some(sort) = ModrinthSortType::from_string(&sort) else {
                return Err(String::from(err_keys::SORT_TYPE_NOT_FOUND));
            };

            // 有一项分类需要输入文本才能搜索
            // if matches!(sort, ModrinthSortType::Downloads)

            let arg = ModrinthSearchArg {
                page: Some(page),
                category,
                query: filter,
                sort,
                version,
                loader: modrinth::to_loader_id(&mod_loader),
                ..Default::default()
            };

            // Modrinth 没有存档 world 类型
            let list = match file_type {
                FileType::Mod => modrinth_api::get_mod_list(arg).await,
                FileType::Resourcepack => modrinth_api::get_resourcepack_list(arg).await,
                FileType::Shaderpack => modrinth_api::get_shaderpack_list(arg).await,
                FileType::DataPacks => modrinth_api::get_datapack_list(arg).await,
                _ => Err(ErrorType::InvalidOperation),
            }
            .map_err(|err| err.to_string())?;

            // 搜索结果不给作者头像：按 author_id 批量查一次，一页一次请求；
            // 查不到就让前端退回名字首字母的占位
            let avatars = modrinth::author_avatars(&list.hits).await;

            let mut list1 = Vec::new();
            // 缓存先攒着，**构建 DTO 期间不持锁**：new_modrinth 里会 await 查
            // 标签图标，首次可能真发网络请求，持全局写锁跨 await 会把其它
            // 窗口读同一份缓存的请求一起堵住。攒完最后一次性写入。
            let mut cached = Vec::new();

            for item in list.hits {
                let pid = item.project_id.clone();
                let info = info.get(&pid);

                let temp = ProjectItemDto::new_modrinth(
                    &item,
                    file_type,
                    info.is_some(),
                    true,
                    collect_utils::is_star(&pid),
                    check_download_now(uuid.as_ref(), &pid),
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
