//! 整合包详情与文件
//!
//! 详情按项目编号拉，文件列表给出可选版本与下载地址；
//! 文件编号到本地路径的映射由 `installed` 提供。

use mml_game::launcher::{FileType, ModPackType};
use mml_net::curseforge_api::{self, CurseFogreArg};
use mml_net::modrinth_api::{self};

use crate::dtos::add_resource_dto::{FileListDto, FileListItemDto, ProjectDetailDto};

use crate::err_keys;

use super::installed::{installed_modpack_files, running_files};
use super::search::PackFileId;
use crate::windows::add_modpack::{CURSEFOGRE_FILE, CURSEFOGRE_INFO, MODRINTH_FILE, MODRINTH_INFO};

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
#[gui_macros::command]
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
#[gui_macros::command]
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
