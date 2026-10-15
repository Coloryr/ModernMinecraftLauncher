//! 添加资源窗口的下载执行与存档选择
//!
//! `add_resource_download` 是那个 220 行的函数：校验参数 → 建下载项与在线信息 →
//! 占位去重 → 交给下载器；失败路径都要把占位从 `DOWNLOAD_NOW` 里摘掉。

use std::path::PathBuf;

use mml_base::file_item::FileItemObj;
use mml_game::launcher::file_online_info_obj::OnlineInfoObj;
use mml_game::launcher::{FileType, ModPackType};
use mml_game::{curseforge, modrinth};
use mml_names::names;
use mml_net::{curseforge_api, modrinth_api};
use tauri::AppHandle;
use uuid::Uuid;

use crate::dtos::add_resource_dto::ResourceSaveDto;
use crate::err_keys;

use super::status::{build_status, emit_add_resource_status};
use super::{DOWNLOAD_NOW, SourceDownloadInfo, SourceInfo};

/// 下载资源到实例
///
/// 模组 / 资源包 / 光影包直接放入实例对应目录；数据包必须指定存档 `world`，
/// zip 原样放入其 datapacks 目录；存档先下到下载目录，
/// 下载完成后再走 `import_save` 解压导入。
///
/// 下载走全局下载器，进度在下载窗口可见，命令立即返回；
/// 进度按目标文件路径匹配后经 add-resource-status 事件推给前端，
/// 完成后条目保留一段时间再从任务表移除，窗口顶部进度条停在「完成」
///
/// - `app`: 窗口句柄，用于广播下载状态事件
/// - `game`: 实例 uuid
/// - `source`: 下载源，取 `curseforge` 或 `modrinth`
/// - `pid`: 项目 ID
/// - `fid`: 文件 ID
/// - `file_type`: 资源类型，取 `mod` / `resourcepack` / `shaderpack` / `save` / `datapacks`
/// - `world`: 目标存档的目录名，即 saves/ 下的文件夹名；只有数据包必填，其余类型忽略
///
/// # 返回值
///
/// 参数非法、类型不受支持或该文件已在下载中时返回 `Err`，内容为 i18n 键；
/// 占位并交给下载器后立即返回 `Ok(())`，真实进度靠事件推
#[gui_macros::ipc_group("add_resource")]
#[tauri::command]
pub async fn add_resource_download(
    app: AppHandle,
    game: String,
    source: String,
    pid: String,
    fid: String,
    file_type: String,
    world: Option<String>,
) -> Result<(), String> {
    let uuid = Uuid::parse_str(&game);
    if uuid.is_err() {
        return Err(String::from(err_keys::UUID));
    }
    let uuid = uuid.unwrap();
    let instance = mml_game::get_instance(&uuid);
    if instance.is_none() {
        return Err(String::from(err_keys::GAME_NOT_FOUND));
    }
    let instance = instance.unwrap();
    let file_type = FileType::from_string(&file_type);
    if file_type.is_none() {
        return Err(String::from(err_keys::FILE_TYPE_NOT_FOUND));
    }
    let file_type = file_type.unwrap();
    if !matches!(
        file_type,
        FileType::Mod
            | FileType::Resourcepack
            | FileType::Shaderpack
            | FileType::Save
            | FileType::DataPacks
    ) {
        return Err(String::from(err_keys::FILE_TYPE_NOT_FOUND));
    }
    let source = ModPackType::from_string(&source);
    if !matches!(source, ModPackType::CurseForge | ModPackType::Modrinth) {
        return Err(String::from(err_keys::SOURCE_TYPE));
    }
    // 存档只有 CurseForge 有，Modrinth 没有 world 类型
    if file_type == FileType::Save && !matches!(source, ModPackType::CurseForge) {
        return Err(String::from("addResource.saveSourceLimit"));
    }
    let world = world
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty());
    // 数据包必须指定目标存档
    if file_type == FileType::DataPacks && world.is_none() {
        return Err(String::from(err_keys::SAVE_NOT_FOUND));
    }

    // 目标目录
    let dir = {
        let game = instance.read().unwrap();
        match file_type {
            FileType::Mod => game.get_mods_path(),
            FileType::Resourcepack => game.get_resourcepacks_path(),
            FileType::Shaderpack => game.get_shaderpacks_path(),
            FileType::DataPacks => game
                .get_saves_path()
                .join(world.as_deref().unwrap())
                .join(names::GAME_DATAPACK_DIR),
            // 存档先下到临时目录，下载完成后再解压导入
            _ => mml_downloader::get_cache_path(),
        }
    };

    // 同一实例同一文件不重复下载：先占位，名字与目标路径等文件信息拿到后再补全
    let key = SourceInfo {
        pid: pid.clone(),
        fid: fid.clone(),
    };
    {
        let mut map = DOWNLOAD_NOW.write().unwrap();
        let entry = map.entry(uuid).or_default();
        if entry.contains_key(&key) {
            return Err(String::from(err_keys::ALREADY_DOWNLOADING));
        }
        entry.insert(
            key.clone(),
            SourceDownloadInfo {
                file: PathBuf::new(),
                name: String::new(),
                now: 0.0,
                done: false,
                failed: false,
                cancelled: false,
            },
        );
    }
    emit_add_resource_status(&app, build_status(&app));

    // 构建下载项目与已下载标记，失败路径都要移除 DOWNLOAD_NOW 条目
    let res: Result<(FileItemObj, OnlineInfoObj), String> = async {
        match source {
            ModPackType::CurseForge => {
                let fnum: u64 = fid
                    .parse()
                    .map_err(|_| String::from(err_keys::BAD_FILE_ID))?;
                let mut list = curseforge_api::get_files(vec![fnum])
                    .await
                    .map_err(|err| err.to_string())?;
                let mut data = list
                    .pop()
                    .ok_or_else(|| String::from(err_keys::FILE_NOT_FOUND))?;
                let obj = curseforge::make_file_item_obj(&mut data, &dir);
                let info = OnlineInfoObj {
                    path: dir.to_string_lossy().to_string(),
                    name: data.display_name.clone(),
                    file: data.file_name.clone(),
                    sha1: data.sha1_hash(),
                    url: obj.url.clone(),
                    modid: data.mod_id.to_string(),
                    fileid: data.id.to_string(),
                };
                Ok((obj, info))
            }
            ModPackType::Modrinth => {
                let data = modrinth_api::get_version(&pid, &fid)
                    .await
                    .map_err(|err| err.to_string())?;
                let obj = modrinth::make_download_obj(&data, &dir);
                // 取 primary 文件，没有 primary 退回第一个。
                // **不能 unwrap**：Modrinth 偶尔返回一个 files 都不带的版本，
                // 原来那句 `files.first().unwrap()` 会直接把整个进程 panic 掉。
                let file = data
                    .files
                    .iter()
                    .find(|item| item.primary)
                    .or_else(|| data.files.first())
                    .ok_or_else(|| String::from(err_keys::FILE_NOT_FOUND))?;
                let info = OnlineInfoObj {
                    path: dir.to_string_lossy().to_string(),
                    name: data.name.clone(),
                    file: file.filename.clone(),
                    sha1: file.hashes.sha1.clone(),
                    url: file.url.clone(),
                    modid: pid.clone(),
                    fileid: data.id.clone(),
                };
                Ok((obj, info))
            }
            _ => Err(String::from(err_keys::SOURCE_TYPE)),
        }
    }
    .await;

    let (obj, online_info) = match res {
        Ok(item) => item,
        Err(err) => {
            DOWNLOAD_NOW
                .write()
                .unwrap()
                .entry(uuid)
                .or_default()
                .remove(&key);
            emit_add_resource_status(&app, build_status(&app));
            return Err(err);
        }
    };

    // 补全条目的显示名与目标路径，进度回调按路径匹配
    {
        let mut map = DOWNLOAD_NOW.write().unwrap();
        if let Some(info) = map.get_mut(&uuid).and_then(|entry| entry.get_mut(&key)) {
            info.file = obj.file.clone();
            info.name = obj.name.clone();
        }
    }
    emit_add_resource_status(&app, build_status(&app));

    let is_save = file_type == FileType::Save;
    let save_zip = obj.file.clone();
    let pid2 = pid.clone();
    let app2 = app.clone();

    // 后台任务：下载 → 终态标记 → 存档导入 / 已下载标记 → 延时移除
    tauri::async_runtime::spawn(async move {
        let outcome = mml_downloader::start_download_task_outcome(vec![obj]).await;

        // 终态：完成 / 失败 / 取消三态互斥（与整合包那套一致）；取消时进度停在原地，
        // 不拉满 —— 用户看得见它是被中途停掉的
        {
            let mut map = DOWNLOAD_NOW.write().unwrap();
            if let Some(info) = map.get_mut(&uuid).and_then(|entry| entry.get_mut(&key)) {
                if outcome.is_done() {
                    info.now = 100.0;
                    info.done = true;
                } else if outcome.is_cancelled() {
                    info.cancelled = true;
                } else {
                    info.now = 100.0;
                    info.failed = true;
                }
            }
        }
        emit_add_resource_status(&app2, build_status(&app2));

        if outcome.is_done() {
            if is_save {
                let instance = instance.clone();
                // 只关心"解包失败"；spawn_blocking 自身的 JoinError 沿用原来的忽略口径
                if let Ok(Err(err)) = tauri::async_runtime::spawn_blocking(move || {
                    instance.read().unwrap().import_save(&save_zip)
                })
                .await
                {
                    mml_log::error_type(err);
                }
            }

            let game = instance.read().unwrap();
            let mut list = game.read_online_info();
            list.insert(pid2, online_info);
            game.save_online_info(&list);
        }

        // 终态条目**不自动移除**：用户要能回头看这次下了哪些、哪些失败了，
        // 由 add_resource_clear_done 手动清（与整合包安装那套一致）
    });

    Ok(())
}

/// 获取实例的存档列表，下载数据包时用来选目标存档
///
/// - `game`: 实例 uuid
///
/// # 返回值
///
/// 未损坏的存档，含 `name` 显示名与 `dir` 目录名；uuid 非法或实例不存在时返回 `Err`
#[gui_macros::ipc_group("add_resource")]
#[tauri::command]
pub async fn add_resource_saves(game: String) -> Result<Vec<ResourceSaveDto>, String> {
    let uuid = Uuid::parse_str(&game);
    if uuid.is_err() {
        return Err(String::from(err_keys::UUID));
    }
    let uuid = uuid.unwrap();
    let game = mml_game::get_instance(&uuid);
    if game.is_none() {
        return Err(String::from(err_keys::GAME_NOT_FOUND));
    }
    let game = game.unwrap();

    // get_saves 是异步的，而 std 锁卫不能跨 await，future 须 Send：先克隆实例设置快照、立刻放锁，
    // 再在阻塞线程里 block_on —— 持锁等异步很容易和写锁互相卡死，clippy 的 await_holding_lock 报的就是这里
    let saves = tauri::async_runtime::spawn_blocking(move || {
        let game = game.read().unwrap().clone();
        tokio::runtime::Handle::current().block_on(async { game.get_saves().await })
    })
    .await
    .map_err(|err| err.to_string())?;

    Ok(saves
        .iter()
        .filter(|item| !item.broken)
        .filter_map(|item| {
            let dir = item.path.file_name()?.to_string_lossy().to_string();
            if dir.is_empty() {
                return None;
            }
            Some(ResourceSaveDto {
                name: item.level_name.clone(),
                dir,
            })
        })
        .collect())
}
