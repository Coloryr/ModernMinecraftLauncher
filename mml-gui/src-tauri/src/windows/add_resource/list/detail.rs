//! 资源详情与文件列表
//!
//! 详情按项目编号拉一次，文件列表带版本号做缓存键；
//! 返回的 DTO 是前端直接渲染的那一份。

use mml_game::curseforge;
use mml_game::launcher::ModPackType;
use mml_net::curseforge_api::{self, CurseFogreArg};
use mml_net::modrinth_api;

use crate::dtos::add_resource_dto::{FileListDto, FileListItemDto, ProjectDetailDto};
use crate::err_keys;

use super::query::{ListQuery, check_version_download_now, parse_query};
use crate::windows::add_resource::{CURSEFOGRE_FILE, CURSEFOGRE_INFO, MODRINTH_FILE};

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
#[gui_macros::command]
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
#[gui_macros::command]
pub async fn add_resource_file(
    game: String,
    source: String,
    pid: String,
    file_type: String,
    page: u32,
    version: Option<String>,
    loader: Option<String>,
) -> Result<FileListDto, String> {
    let ListQuery {
        uuid,
        info,
        source,
        file_type,
        mod_loader,
    } = parse_query(&game, &source, &file_type, loader.as_deref())?;

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
                        check_version_download_now(uuid.as_ref(), &pid, &item.id.to_string()),
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

            let list = modrinth_api::get_file_versions(&pid, version.as_deref(), loader.as_deref())
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
                        check_version_download_now(uuid.as_ref(), &pid, &item.id),
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
