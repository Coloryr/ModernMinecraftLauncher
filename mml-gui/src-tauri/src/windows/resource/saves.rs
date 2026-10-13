//! 资源管理窗口的存档分类：列表 / 备份列表 / 还原 / 删除 / 备份 / 改模式与难度
//!
//! 存档本体由内核 `game_saves` 扫描 level.dat；备份目录在存档下的 backups。

use std::collections::HashMap;

use crate::dtos::{SaveBackupDto, SaveEditDto, SaveItemDto};
use crate::err_keys;
use mml_game::game_saves::SaveEditObj;
use mml_sys::path_helper;
use uuid::Uuid;

use super::{KIND_SAVES, data_url, find_save, load_saves, parse_instance, resource_file};

// ==================== 存档 ====================

/// 存档列表：解析 level.dat
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 存档列表，含备份数量与图标；实例不存在或扫描失败返回错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_list_saves(uuid: String) -> Result<Vec<SaveItemDto>, String> {
    let instance = parse_instance(&uuid)?;
    let list = load_saves(instance.clone()).await?;

    // 备份索引只读一次，不是每个存档读一遍：存档 → 备份数
    let counts: HashMap<String, u32> = {
        let game = instance.read().unwrap();
        game.get_backups()
            .map(|info| {
                info.values()
                    .map(|item| (item.dir.clone(), item.back.len() as u32))
                    .collect()
            })
            .unwrap_or_default()
    };

    Ok(list
        .iter()
        .filter_map(|item| {
            let dir = item.path.file_name()?.to_string_lossy().to_string();
            if dir.is_empty() {
                return None;
            }
            // 图标是存档目录里的 png，SaveObj.icon 给了路径，小文件直接读
            let icon = item
                .icon
                .as_ref()
                .and_then(|p| path_helper::read_byte(p).ok());
            Some(SaveItemDto {
                dir: dir.clone(),
                level_name: item.level_name.clone(),
                last_played: item.last_played,
                game_type: item.game_type,
                hard_core: item.hard_core != 0,
                difficulty: item.difficulty,
                broken: item.broken,
                seed: item.random_seed.to_string(),
                icon: data_url(icon.as_ref()),
                backups: counts.get(&dir).copied().unwrap_or(0),
            })
        })
        .collect())
}

/// 某个存档的备份列表，按时间倒序，最近的在最上面
///
/// 索引是按**存档名**存的，见 `game_saves::backup`，这里按**目录名**反查 ——
/// 目录名才是稳定的键，存档可以改名，改名后索引会另起一条。
///
/// - `uuid`: 实例 uuid
/// - `dir`: 存档目录名，即 saves 下的一级目录
///
/// # 返回值
///
/// 备份列表，按修改时间倒序，最近的在最上面；索引里没有这个存档时返回空表
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub fn resource_list_save_backups(uuid: String, dir: String) -> Result<Vec<SaveBackupDto>, String> {
    let instance = parse_instance(&uuid)?;
    let game = instance.read().unwrap();

    let Ok(info) = game.get_backups() else {
        return Ok(Vec::new());
    };
    let Some(entry) = info.values().find(|item| item.dir == dir) else {
        return Ok(Vec::new());
    };

    let base = game.get_backup_path();
    let mut list: Vec<SaveBackupDto> = entry
        .back
        .iter()
        .map(|file| {
            let meta = std::fs::metadata(base.join(file)).ok();
            SaveBackupDto {
                file: file.clone(),
                size: meta.as_ref().map(|m| m.len()).unwrap_or(0),
                time: meta
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0),
            }
        })
        .collect();
    list.sort_by_key(|a| std::cmp::Reverse(a.time));
    Ok(list)
}

/// 还原某个备份，**破坏性**：现有存档整目录移入回收站，再解开备份
///
/// - `uuid`: 实例 uuid
/// - `dir`: 存档目录名，即 saves 下的一级目录
/// - `file`: 备份文件名，取自 [`resource_list_save_backups`]
///
/// # 返回值
///
/// 只表示成功 / 失败：索引里找不到该存档返回 `err.saveNotFound`，其余是内核错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_restore_save_backup(
    uuid: String,
    dir: String,
    file: String,
) -> Result<(), String> {
    let instance = parse_instance(&uuid)?;

    // 解压耗时较长，与备份同一条路：丢到阻塞线程
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let game = instance.read().unwrap();
        let info = game.get_backups().map_err(|err| err.to_string())?;
        // 索引按存档名存，这里按目录名找，目录名才稳定
        let entry = info
            .values()
            .find(|item| item.dir == dir)
            .ok_or_else(|| err_keys::SAVE_NOT_FOUND.to_string())?;

        game.restore_backup(entry, &file, None)
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

/// 删除存档，目录进回收站
///
/// - `uuid`: 实例 uuid
/// - `dir`: 存档目录名，即 saves 下的一级目录
///
/// # 返回值
///
/// 只表示成功 / 失败：目录名非法返回 `err.fileName`，该名字不是目录返回 `err.saveNotFound`
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_delete_save(uuid: String, dir: String) -> Result<(), String> {
    let instance = parse_instance(&uuid)?;
    let file = resource_file(&instance, KIND_SAVES, &dir)?;
    if !file.is_dir() {
        return Err(err_keys::SAVE_NOT_FOUND.to_string());
    }
    path_helper::move_to_trash(&file).map_err(|err| err.to_string())
}

/// 备份存档：zip 到实例备份目录并登记
///
/// - `uuid`: 实例 uuid
/// - `dir`: 存档目录名，即 saves 下的一级目录
///
/// # 返回值
///
/// 该存档的显示名 `level_name`，前端拿去提示；找不到存档返回 `err.saveNotFound`
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_backup_save(uuid: String, dir: String) -> Result<String, String> {
    let instance = parse_instance(&uuid)?;

    // 压缩耗时较长，这里**全程不持锁**：先克隆一份设置快照，取存档列表与压缩都在这份
    // 快照上做；原来的实现整个 zip 期间都持着读锁，`get_saves().await` 还跨了 await
    let name = tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
        let game = instance.read().unwrap().clone();
        let saves = tokio::runtime::Handle::current().block_on(async { game.get_saves().await });
        let save = saves
            .iter()
            .find(|item| {
                item.path
                    .file_name()
                    .map(|n| *n.to_string_lossy() == dir)
                    .unwrap_or(false)
            })
            .ok_or_else(|| err_keys::SAVE_NOT_FOUND.to_string())?;

        save.backup(&game, None).map_err(|err| err.to_string())?;
        Ok(save.level_name.clone())
    })
    .await
    .map_err(|err| err.to_string())??;

    Ok(name)
}

/// 改存档的基本信息：模式 / 难度 / 极限模式 / 名字 / 种子，写回 level.dat
///
/// 实例正在运行时**不改**：游戏退出时会用它内存里的值覆盖这个文件，改了也白改。
/// 入参是下拉与输入框给的值，越界的模式与难度一律夹到合法区间，别把没意义的值写进存档。
///
/// - `uuid`: 实例 uuid
/// - `dir`: 存档目录名，即 saves 下的一级目录
/// - `info`: 新的信息，见 [`SaveEditDto`]
///
/// # 返回值
///
/// 只表示成功 / 失败：实例在运行返回 `err.instanceRunning`，名字为空返回
/// `err.saveNameEmpty`，种子不是整数返回 `err.saveSeed`，存档找不到返回
/// `err.saveNotFound`，其余是内核错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_save_edit(
    uuid: String,
    dir: String,
    info: SaveEditDto,
) -> Result<(), String> {
    let parsed = Uuid::parse_str(&uuid).map_err(|_| String::from(err_keys::UUID))?;
    let instance = parse_instance(&uuid)?;
    if mml_game::is_running(&parsed) {
        return Err(String::from(err_keys::INSTANCE_RUNNING));
    }

    // 前端已经拦过一道，这里是 IPC 边界，自己再判一次
    let level_name = info.level_name.trim().to_string();
    if level_name.is_empty() {
        return Err(String::from(err_keys::SAVE_NAME_EMPTY));
    }
    let seed = info
        .seed
        .trim()
        .parse::<i64>()
        .map_err(|_| String::from(err_keys::SAVE_SEED))?;

    let edit = SaveEditObj {
        game_type: info.game_type.clamp(0, 3),
        difficulty: info.difficulty.min(3),
        hard_core: info.hard_core,
        level_name,
        seed,
    };

    let mut save = find_save(instance, &dir).await?;
    save.set_info(&edit).map_err(|err| err.to_string())
}
