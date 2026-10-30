//! 把 `.minecraft` 的一个版本导入成实例
//!
//! 只搬游戏数据，[`super::detect::game_entries`] 已经排掉 `versions` / `libraries` /
//! `assets` 这些启动器自己的东西。实例配置由版本 json 推出：原版 / 已收录的加载器走
//! `to_instance`，M²L 没收录的自定义版本（LabyMod 这类）按版本 id 装一份版本 json，
//! 启动时才读得到。

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tauri::AppHandle;
use uuid::Uuid;

use mml_game::launcher_path::version_path;
use mml_game::loader::LoaderType;
use mml_game::other_launcher::official_obj::OfficialObj;
use mml_names::names;
use mml_sys::path_helper;

use crate::dtos::MinecraftImportDto;
use crate::log_text::LogText;

use super::detect;
use super::progress;

/// 等核心加载完成的超时，单位秒；实例 API 要核心 load 之后才可靠
const CORE_WAIT_SECS: u64 = 120;

/// 每复制这么多个文件推一次进度
const PROGRESS_STEP: u32 = 16;

/// 导入一个版本
///
/// - `app`: 应用句柄，用于发进度事件
/// - `path`: `.minecraft` 目录
/// - `version`: 版本 id，即 `versions` 下的目录名
/// - `name`: 实例名，缺省用版本 id
///
/// # 返回值
///
/// 见 [`MinecraftImportDto`]；目录 / 版本缺失或建实例失败返回错误文案
pub(super) async fn import(
    app: &AppHandle,
    path: &str,
    version: &str,
    name: Option<String>,
) -> Result<MinecraftImportDto, String> {
    let root = PathBuf::from(path);
    if !root.is_dir() {
        return Err("err.minecraftMissing".to_string());
    }

    // 实例 API 依赖核心已 load：弹窗是加载期间弹的，用户点得快时这里要等一会儿
    wait_core_ready()?;

    let json = root
        .join(names::VERSION_DIR)
        .join(version)
        .join(format!("{version}{}", names::JSON_DOT_EXT));
    if !json.is_file() {
        return Err("err.minecraftVersionMissing".to_string());
    }

    let official = OfficialObj::read_from_file(&json).map_err(|err| err.to_string())?;
    let mut instance = official.to_instance();

    if instance.version.is_empty() {
        // 未收录的自定义版本：版本号就用目录名，版本 json 装进 M²L 的版本目录
        instance.version = version.to_string();
        instance.loader = LoaderType::Normal;
        instance.loader_version = None;
        instance.custom_loader = None;
        install_version_json(&json, version)?;
    }

    instance.uuid = Uuid::new_v4();
    if let Some(name) = name.filter(|item| !item.trim().is_empty()) {
        instance.name = name.trim().to_string();
    }
    if instance.name.is_empty() {
        instance.name = version.to_string();
    }
    mml_game::check_name(&instance.name).map_err(|err| err.to_string())?;

    let game = instance
        .create_instance_in_group(None, None)
        .await
        .map_err(|err| err.to_string())?;
    let dest = game.read().unwrap().get_game_path();
    // HMCL / PCL 的版本隔离：游戏数据在 versions/<id>/ 里，不在根目录
    let (source, entries) = detect::game_data_root(&root, version);
    let (files, failed) = copy_game_data(app, &source, &entries, &dest)?;
    let name = game.read().unwrap().name.clone();

    // 标记写不上顶多下次再问一次，不该让已经建好的实例显示成失败
    if let Err(err) = detect::mark("import", version) {
        mml_log::error(LogText::MinecraftMarkerWriteFailed { error: err }.text());
    }
    mml_log::info(
        LogText::MinecraftImported {
            name: name.clone(),
            files,
        }
        .text(),
    );

    Ok(MinecraftImportDto {
        name,
        version: version.to_string(),
        files,
        failed,
    })
}

/// 等核心 load 完成
///
/// # 返回值
///
/// 加载完成返回 `Ok(())`；超时返回错误文案
fn wait_core_ready() -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(CORE_WAIT_SECS);
    while !mml_core::get_state() {
        if Instant::now() >= deadline {
            return Err("err.minecraftCoreTimeout".to_string());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

/// 把版本 json 装进 M²L 的版本目录，已有同名文件时不覆盖
///
/// - `json`: 源版本 json
/// - `version`: 版本 id
///
/// # 返回值
///
/// 复制成功或目标已存在返回 `Ok(())`
fn install_version_json(json: &Path, version: &str) -> Result<(), String> {
    let target = version_path::get_version_dir().join(format!("{version}{}", names::JSON_DOT_EXT));
    if target.is_file() {
        return Ok(());
    }
    path_helper::copy_file(json, &target).map_err(|err| err.to_string())
}

/// 复制游戏数据到实例的游戏目录
///
/// 单个文件失败不中断，记进返回的失败列表。
///
/// - `app`: 应用句柄，用于发进度事件
/// - `root`: 游戏数据目录，即 [`detect::game_data_root`] 给的那个
/// - `entries`: 要搬的条目，同样来自 [`detect::game_data_root`]
/// - `dest`: 实例的游戏目录
///
/// # 返回值
///
/// `(成功文件数, 失败条目相对路径)`
fn copy_game_data(
    app: &AppHandle,
    root: &Path,
    entries: &[PathBuf],
    dest: &Path,
) -> Result<(u32, Vec<String>), String> {
    let mut files: Vec<(PathBuf, PathBuf)> = Vec::new();
    for entry in entries {
        if entry.is_dir() {
            for file in path_helper::get_all_files(&entry) {
                let rel = file.strip_prefix(root).map_err(|err| err.to_string())?;
                files.push((file.clone(), dest.join(rel)));
            }
        } else if entry.is_file() {
            let rel = entry.strip_prefix(root).map_err(|err| err.to_string())?;
            files.push((entry.clone(), dest.join(rel)));
        }
    }

    let total = files.len() as u32;
    progress(app, 0, total, "");
    let mut done = 0u32;
    let mut failed: Vec<String> = Vec::new();
    for (from, to) in &files {
        if let Some(parent) = to.parent() {
            let _ = path_helper::create_dir_all(parent);
        }
        match path_helper::copy_file(from, to) {
            Ok(()) => done += 1,
            Err(_) => {
                if let Ok(rel) = from.strip_prefix(root) {
                    failed.push(rel.to_string_lossy().to_string());
                }
            }
        }
        let index = done + failed.len() as u32;
        if index % PROGRESS_STEP == 0 || index == total {
            let text = from
                .strip_prefix(root)
                .map(|rel| rel.to_string_lossy().to_string())
                .unwrap_or_default();
            progress(app, index, total, &text);
        }
    }

    Ok((done, failed))
}
