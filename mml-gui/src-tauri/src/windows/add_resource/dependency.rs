//! 前置依赖：把某个文件的前置（含前置的前置）解析出来，交给「下载资源」窗口勾选下载
//!
//! 解析本身在内核里，即 `mml_game::modrinth` / `mml_game::curseforge` 的 `get_mod_dependencies`
//! （递归 + 去重 + 标记可选），这里只做三件事：取根文件对象、把实例里已经装了的过滤掉、
//! 转成前端 DTO。
//!
//! 只处理模组：其它类型的"前置"不是模组，下到 mods/ 反而是错的。

use mml_game::launcher::{FileType, ModPackType};
use mml_game::{curseforge, modrinth};
use mml_net::curseforge_api::{self, file_obj::CurseForgeFileDataObj};
use mml_net::modrinth_api::{self, version_obj::ModrinthVersionObj};
use uuid::Uuid;

use crate::dtos::add_resource_dto::{ResourceDependencyDto, ResourceDependencyFileDto};
use crate::image_manager;

/// 解析前置，并过滤掉实例里已有的
///
/// - `game`: 实例 uuid，用来取游戏版本 / 加载器与"已经装了什么"
/// - `source`: 下载源，取 `curseforge` 或 `modrinth`
/// - `pid`: 项目 ID
/// - `fid`: 文件 ID
/// - `file_type`: 资源类型；只有模组有前置，其余类型直接返回空表
///
/// # 返回值
///
/// 待用户勾选的前置列表，已剔除本实例已装的项目与指向自身的环；参数非法、
/// 实例不存在或源不受支持返回 `Err`
#[gui_macros::ipc_group("add_resource")]
#[tauri::command]
pub async fn add_resource_dependencies(
    game: String,
    source: String,
    pid: String,
    fid: String,
    file_type: String,
) -> Result<Vec<ResourceDependencyDto>, String> {
    let uuid = Uuid::parse_str(&game).map_err(|_| String::from("err.uuid"))?;
    let instance = mml_game::get_instance(&uuid).ok_or_else(|| String::from("err.gameNotFound"))?;
    let file_type = FileType::from_string(&file_type)
        .ok_or_else(|| String::from("err.fileTypeNotFound"))?;
    if file_type != FileType::Mod {
        return Ok(Vec::new());
    }

    let source = ModPackType::from_string(&source);
    if !matches!(source, ModPackType::CurseForge | ModPackType::Modrinth) {
        return Err(String::from("err.sourceType"));
    }

    // 实例侧：游戏版本与加载器决定给依赖挑哪个文件，"已装表"决定哪些不用再列出来
    let (version, loader, online) = {
        let game = instance.read().unwrap();
        (game.version.clone(), game.loader, game.read_online_info())
    };

    let list = match source {
        ModPackType::Modrinth => {
            let obj = modrinth_file(&pid, &fid).await?;
            modrinth::get_mod_dependencies(&obj, &version, loader)
                .await
                .into_iter()
                .filter(|item| item.mod_id != pid && !online.contains_key(&item.mod_id))
                .map(|item| {
                    let files: Vec<ResourceDependencyFileDto> = item
                        .list
                        .iter()
                        .map(|file| ResourceDependencyFileDto {
                            fid: file.id.clone(),
                            name: file.name.clone(),
                            time: file.date_published.clone(),
                        })
                        .collect();

                    ResourceDependencyDto {
                        // 内核把最合适的那个排在第一个，它就是默认选中的
                        fid: files.first().map(|file| file.fid.clone()).unwrap_or_default(),
                        pid: item.mod_id,
                        name: item.name,
                        icon: item.icon.map(|url| image_manager::push_image_url(&url)),
                        optional: item.opt,
                        files,
                    }
                })
                .collect()
        }
        _ => {
            let obj = curseforge_file(&pid, &fid).await?;
            curseforge::get_mod_dependencies(&obj, &version, loader)
                .await
                .into_iter()
                .filter(|item| item.mod_id.to_string() != pid)
                .filter(|item| !online.contains_key(&item.mod_id.to_string()))
                .map(|item| {
                    let files: Vec<ResourceDependencyFileDto> = item
                        .list
                        .iter()
                        .map(|file| ResourceDependencyFileDto {
                            fid: file.id.to_string(),
                            name: file.display_name.clone(),
                            time: file.file_date.clone(),
                        })
                        .collect();

                    ResourceDependencyDto {
                        fid: files.first().map(|file| file.fid.clone()).unwrap_or_default(),
                        pid: item.mod_id.to_string(),
                        name: item.name,
                        icon: item.icon.map(|url| image_manager::push_image_url(&url)),
                        optional: item.opt,
                        files,
                    }
                })
                .collect()
        }
    };

    Ok(list)
}

/// 取 Modrinth 的文件对象，即版本接口
///
/// 不走文件列表页留下的缓存：缓存里的对象没实现 `Clone`，为它给几个 API 结构体加 Clone
/// 不划算，而这里最多多花一次请求。
///
/// - `pid`: 项目 ID
/// - `fid`: 文件 ID，即版本 ID
///
/// # 返回值
///
/// 版本对象；请求失败返回错误串
async fn modrinth_file(pid: &str, fid: &str) -> Result<ModrinthVersionObj, String> {
    modrinth_api::get_version(pid, fid)
        .await
        .map_err(|err| err.to_string())
}

/// 取 CurseForge 的文件对象，即文件接口
///
/// - `_pid`: 项目 ID，文件接口按文件 ID 取，这里只为调用点对齐
/// - `fid`: 文件 ID
///
/// # 返回值
///
/// 文件对象；请求失败或文件不存在返回错误串
async fn curseforge_file(_pid: &str, fid: &str) -> Result<CurseForgeFileDataObj, String> {
    let fnum: u64 = fid.parse().map_err(|_| String::from("err.badFileId"))?;
    let mut list = curseforge_api::get_files(vec![fnum])
        .await
        .map_err(|err| err.to_string())?;

    list.pop().ok_or_else(|| String::from("err.fileNotFound"))
}
