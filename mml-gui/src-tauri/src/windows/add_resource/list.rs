//! 添加资源窗口的列表与详情查询：项目列表的搜索与分类翻页，某项目的文件列表与项目详情
//!
//! 从 `add_resource/mod.rs` 拆出来的几个大函数。列表页顺手把项目与文件数据写进
//! `CURSEFOGRE_*` / `MODRINTH_*` 缓存，详情与下载都从缓存里取。

use mml_game::launcher::{FileType, ModPackType};
use mml_game::loader::LoaderType;
use mml_game::{curseforge, modrinth};
use mml_names::i18_items::error_type::ErrorType;
use mml_net::curseforge_api::{self, CurseFogreArg, CurseForgeSortType};
use mml_net::modrinth_api::{self, ModrinthSearchArg, ModrinthSortType};
use uuid::Uuid;

use crate::collect_utils;
use crate::dtos::add_resource_dto::{
    FileListDto, FileListItemDto, ProjectDetailDto, ProjectDto, ProjectItemDto,
};
use crate::err_keys;

use super::{CURSEFOGRE_FILE, CURSEFOGRE_INFO, DOWNLOAD_NOW, MODRINTH_FILE, MODRINTH_INFO};

/// 获取文件列表，即某个项目的版本 / 文件
///
/// 一页 50 个项目：CurseForge 换页需要重新查询，Modrinth 没有换页、一次获取所有。
///
/// - `game`: 实例 uuid，用来标记"已下载 / 正在下载"
/// - `source`: 下载源，取 `curseforge` 或 `modrinth`
/// - `pid`: 项目 ID
/// - `file_type`: 资源类型，决定 `loader` 是否生效
/// - `page`: 页码，从 0 开始
/// - `version`: 游戏版本过滤，`None` 表示不限
/// - `loader`: 模组加载器过滤，仅模组类型有意义，缺省 / 非法值按 normal
///
/// # 返回值
///
/// 该页文件列表、总数与最大页数、项目名；源不受支持、类型非法或实例不存在时返回 `Err`
#[gui_macros::ipc_group("add_resource")]
#[tauri::command]
pub async fn add_resource_file(
    game: String,
    source: String,
    pid: String,
    file_type: String,
    page: u32,
    version: Option<String>,
    loader: Option<String>,
) -> Result<FileListDto, String> {
    let Ok(uuid) = Uuid::parse_str(&game) else {
        return Err(String::from(err_keys::UUID));
    };
    let Some(game) = mml_game::get_instance(&uuid) else {
        return Err(String::from(err_keys::GAME_NOT_FOUND));
    };
    let info = game.read().unwrap().read_online_info();
    let source = ModPackType::from_string(&source);
    let Some(file_type) = FileType::from_string(&file_type) else {
        return Err(String::from(err_keys::FILE_TYPE_NOT_FOUND));
    };
    // loader 只对模组有意义，其余类型一律 Normal，缺省 / 非法值也视为 normal
    let mod_loader = if matches!(file_type, FileType::Mod) {
        LoaderType::from_string(&loader.clone().unwrap_or_default()).unwrap_or(LoaderType::Normal)
    } else {
        LoaderType::Normal
    };

    match source {
        ModPackType::CurseForge => {
            let data = curseforge_api::get_mod_info(&pid)
                .await
                .map_err(|err| err.to_string())?;
            let list = curseforge_api::get_files_page(CurseFogreArg {
                id: Some(pid.clone()),
                version,
                page: Some(page),
                loader: curseforge::to_loader_id(&mod_loader),
                ..Default::default()
            })
            .await
            .map_err(|err| err.to_string())?;

            let mut list1 = Vec::new();

            {
                let mut lock = CURSEFOGRE_FILE.write().await;
                let list2 = lock.entry(pid.clone()).or_default();

                for item in list.data {
                    let info = info.get(&item.mod_id.to_string());
                    let download = match info {
                        Some(data) => data.fileid == item.id.to_string(),
                        None => false,
                    };

                    list1.push(FileListItemDto::new_curseforge(
                        &item,
                        file_type,
                        download,
                        check_version_download_now(&uuid, &pid, &item.id.to_string()),
                    ));
                    list2.insert(item.id.to_string(), item);
                }
            }

            Ok(FileListDto {
                list: list1,
                count: list.pagination.total_count,
                name: data.data.name,
                max_page: list.pagination.total_count / 50,
            })
        }
        ModPackType::Modrinth => {
            let data = modrinth_api::get_project(&pid)
                .await
                .map_err(|err| err.to_string())?;

            let list = modrinth_api::get_file_versions(
                &pid,
                match version.as_ref() {
                    Some(data) => Some(data),
                    None => None,
                },
                match loader.as_ref() {
                    Some(data) => Some(data),
                    None => None,
                },
            )
            .await
            .map_err(|err| err.to_string())?;

            let mut list1 = Vec::new();

            let len = list.len() as u64;

            {
                let mut lock = MODRINTH_FILE.write().await;
                let list2 = lock.entry(pid.clone()).or_default();

                for item in list {
                    let info = info.get(&item.project_id);
                    let download = match info {
                        Some(data) => data.fileid == item.id,
                        None => false,
                    };

                    // 没有文件可下的版本直接跳过，理由见 new_modrinth 的注释
                    if let Some(dto) = FileListItemDto::new_modrinth(
                        &item,
                        file_type,
                        download,
                        check_version_download_now(&uuid, &pid, &item.id),
                    ) {
                        list1.push(dto);
                    }
                    list2.insert(item.id.to_string(), item);
                }
            }

            Ok(FileListDto {
                list: list1,
                count: len,
                name: data.title,
                max_page: len / 50,
            })
        }
        _ => Err(String::from(err_keys::SOURCE_TYPE)),
    }
}

/// 获取项目详情：简介 + 正文 + 作者 + 标签 + 截图
///
/// 与整合包窗口的 `add_modpack_detail` 同一口径，只是缓存改用本窗口的
/// `CURSEFOGRE_INFO` / `MODRINTH_INFO`（列表页请求时填充）。
///
/// - `source`: 下载源，取 `curseforge` 或 `modrinth`
/// - `pid`: 项目 ID
///
/// # 返回值
///
/// 项目详情；Modrinth 含 markdown 正文，CurseForge 只有简介；源不受支持或请求失败返回 `Err`
#[gui_macros::ipc_group("add_resource")]
#[tauri::command]
pub async fn add_resource_detail(source: String, pid: String) -> Result<ProjectDetailDto, String> {
    let source = ModPackType::from_string(&source);

    match source {
        ModPackType::CurseForge => {
            // 列表页已缓存项目数据，直接借用构造；未命中才发请求
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
#[tauri::command]
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
    let Ok(uuid) = Uuid::parse_str(&game) else {
        return Err(String::from(err_keys::UUID));
    };
    let Some(game) = mml_game::get_instance(&uuid) else {
        return Err(String::from(err_keys::GAME_NOT_FOUND));
    };
    let info = game.read().unwrap().read_online_info();
    let source = ModPackType::from_string(&source);
    let Some(file_type) = FileType::from_string(&file_type) else {
        return Err(String::from(err_keys::FILE_TYPE_NOT_FOUND));
    };
    // loader 只对模组有意义，其余类型一律 Normal，缺省 / 非法值也视为 normal
    let mod_loader = if matches!(file_type, FileType::Mod) {
        LoaderType::from_string(&loader.clone().unwrap_or_default()).unwrap_or(LoaderType::Normal)
    } else {
        LoaderType::Normal
    };

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
                    check_download_now(&uuid, &item.id.to_string()),
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
                    check_download_now(&uuid, &pid),
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

/// 项目是否正在下载，列表角标用；判据是该实例任务表里有同 pid 且**还没结束**的条目
///
/// - `game`: 实例 uuid
/// - `pid`: 项目 ID
///
/// # 返回值
///
/// 该项目下有任一文件在下载即为 `true`
fn check_download_now(game: &Uuid, pid: &str) -> bool {
    let read = DOWNLOAD_NOW.read().unwrap();
    let list = read.get(game);
    if list.is_none() {
        return false;
    }
    // 终态条目会一直留在表里（等用户点「清除已完成」），所以必须把 done / failed 排掉
    for (key, info) in list.unwrap().iter() {
        if key.pid == pid && !info.done && !info.failed {
            return true;
        }
    }

    false
}

/// 具体文件是否正在下载；判据是该实例任务表里有同 pid 与 fid 且**还没结束**的条目
///
/// - `game`: 实例 uuid
/// - `pid`: 项目 ID
/// - `fid`: 文件 ID
///
/// # 返回值
///
/// 该项目下这个文件正在下载即为 `true`
fn check_version_download_now(game: &Uuid, pid: &str, fid: &str) -> bool {
    let read = DOWNLOAD_NOW.read().unwrap();
    let list = read.get(game);
    if list.is_none() {
        return false;
    }
    for (key, info) in list.unwrap().iter() {
        if key.fid == fid && key.pid == pid && !info.done && !info.failed {
            return true;
        }
    }

    false
}
