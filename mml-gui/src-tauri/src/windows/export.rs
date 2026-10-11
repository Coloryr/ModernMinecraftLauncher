//! 实例导出窗口：导出为 CurseForge / Modrinth 标准整合包
//!
//! 打包逻辑在内核 `mml_game::game_export` 的 `InstanceSettingObj::export`，
//! 这里只做实例文件到在线模组 / 本地文件分组的收集与任务托管：
//! `export_run` 后台跑导出，进度经 `export-progress` 事件上报前端。

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use mml_base::{
    archives::{ArchiveType, IBaseArchiveGui},
    file_item::FileHash,
    hash_helper::{self, HashType},
};
use mml_game::{
    game_export::{ExportArg, ExportPackType, OnlineFileExport},
    launcher::instance_setting_obj::InstanceSettingObj,
};
use mml_sys::path_helper;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::{
    dtos::export_dto::{ExportConfigDto, ExportInfoDto, ExportModDto, ExportProgressDto},
    dtos::log_dto::LogFocusDto,
    listens,
};

/// 导出任务进行中，同时只允许一个导出任务
static EXPORT_RUNNING: AtomicBool = AtomicBool::new(false);

/// 切换目标实例事件：窗口已存在时再次打开会推送，前端据此切换实例
///
/// - `app`: 应用句柄，广播用它
/// - `event`: 目标实例 uuid
#[gui_macros::emit]
pub fn emit_export_focus(app: &AppHandle, event: LogFocusDto) {
    let _ = app.emit(listens::EXPORT_FOCUS, event);
}

/// 导出进度事件：打包各文件时推进，结束时发 done / failed
///
/// - `app`: 应用句柄，广播用它
/// - `event`: 当前进度快照
#[gui_macros::emit]
fn emit_export_progress(app: &AppHandle, event: ExportProgressDto) {
    let _ = app.emit(listens::EXPORT_PROGRESS, event);
}

/// 解析实例 uuid
///
/// - `uuid`: 实例 uuid 字符串
///
/// # 返回值
///
/// 实例设置的克隆；uuid 非法返回 `err.uuid`，实例不存在返回 `err.gameNotFound`
fn parse_instance(uuid: &str) -> Result<InstanceSettingObj, String> {
    let uuid = Uuid::parse_str(uuid).map_err(|_| "err.uuid".to_string())?;
    mml_game::get_instance(&uuid)
        .map(|g| g.read().unwrap().clone())
        .ok_or_else(|| "err.gameNotFound".to_string())
}

/// 目录是否含至少一个文件，递归统计
///
/// - `path`: 待检查的目录
///
/// # 返回值
///
/// 目录存在且有文件时为真
fn dir_has_files(path: &PathBuf) -> bool {
    path.is_dir() && !path_helper::get_all_files(path).is_empty()
}

/// 收集目录下全部文件
///
/// - `path`: 待收集的目录
///
/// # 返回值
///
/// 目录下的全部文件；目录不存在时返回空表
fn collect_files(path: &PathBuf) -> Vec<PathBuf> {
    if path.is_dir() {
        path_helper::get_all_files(path)
    } else {
        Vec::new()
    }
}

/// 获取实例导出信息：在线模组 / 本地模组分组加各内容目录有无
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 实例元数据与模组分组；uuid 非法或实例不存在返回错误文案
#[tauri::command]
pub fn export_get_info(uuid: String) -> Result<ExportInfoDto, String> {
    let game = parse_instance(&uuid)?;

    // mod_info.json 里有来源的文件走在线清单，判据是 modid 与 fileid 都非空；其余进 overrides
    let online_list = game.read_online_info();
    let mut online: std::collections::HashMap<
        String,
        &mml_game::launcher::file_online_info_obj::OnlineInfoObj,
    > = std::collections::HashMap::new();
    for info in online_list.values() {
        if !info.modid.is_empty() && !info.fileid.is_empty() {
            online.insert(info.file.clone(), info);
        }
    }

    let mut online_mods = Vec::new();
    let mut local_mods = Vec::new();
    for file in collect_files(&game.get_mods_path()) {
        let Some(name) = file.file_name().map(|n| n.to_string_lossy().to_string()) else {
            continue;
        };
        if let Some(info) = online.get(&name) {
            online_mods.push(ExportModDto {
                name,
                online: true,
                mod_id: Some(info.modid.clone()),
                file_id: Some(info.fileid.clone()),
            });
        } else {
            local_mods.push(ExportModDto {
                name,
                online: false,
                mod_id: None,
                file_id: None,
            });
        }
    }

    Ok(ExportInfoDto {
        name: game.name.clone(),
        version: game.version.clone(),
        loader: game.loader.to_string().to_owned(),
        loader_version: game.loader_version.clone(),
        online_mods,
        local_mods,
        has_config: dir_has_files(&game.get_config_path()),
        has_resource_packs: dir_has_files(&game.get_resourcepacks_path()),
        has_shader_packs: dir_has_files(&game.get_shaderpacks_path()),
    })
}

/// 在线模组 → 导出条目：算文件大小与 sha1/sha512，供 Modrinth index 使用
///
/// - `file`: 模组文件路径
/// - `info`: 该文件在 mod_info.json 里的来源信息
/// - `pack`: 目标整合包格式
///
/// # 返回值
///
/// 导出条目；文件元数据读不到时返回 `None`
fn make_online_export(
    file: PathBuf,
    info: mml_game::launcher::file_online_info_obj::OnlineInfoObj,
    pack: ExportPackType,
) -> Option<OnlineFileExport> {
    let meta = std::fs::metadata(&file).ok()?;
    let sha1 = hash_helper::gen_hash_from_file(HashType::Sha1, &file).unwrap_or_default();
    let sha512 = hash_helper::gen_hash_from_file(HashType::Sha512, &file).unwrap_or_default();

    Some(OnlineFileExport {
        file,
        size: meta.len() as usize,
        pack,
        url: info.url.clone(),
        hash: FileHash::Sha1Sha512(sha1, sha512),
        info: Some(info),
    })
}

/// 进度回调：内核 `IBaseArchiveGui` → `export-progress` 事件
struct ExportProgressGui {
    /// 应用句柄，事件经它转发到前端
    app: AppHandle,
    /// 导出目标文件名，随每条进度一起下发
    file: String,
}

impl IBaseArchiveGui for ExportProgressGui {
    /// 开始打包：发一条 `running` 且进度归零的事件
    ///
    /// - `total`: 总计需要打包的文件数
    fn start(&self, total: usize) {
        emit_export_progress(
            &self.app,
            ExportProgressDto {
                state: "running".to_string(),
                now: 0,
                total: total as u64,
                text: String::new(),
                error: None,
                file: self.file.clone(),
            },
        );
    }

    /// 单个文件打包完成：推进进度并带上当前文件名
    ///
    /// - `filename`: 当前文件名
    /// - `current`: 已处理数量
    fn update(&self, filename: Option<String>, current: usize) {
        emit_export_progress(
            &self.app,
            ExportProgressDto {
                state: "running".to_string(),
                now: current as u64,
                total: 0,
                text: filename.unwrap_or_default(),
                error: None,
                file: self.file.clone(),
            },
        );
    }

    /// 打包完成：终态由 `export_run` 统一发，这里不做处理
    fn done(&self) {}

    /// 文件名含非法字符时一律同意替换，导出侧不拦
    ///
    /// - `_name`: 原始文件名，这里不看
    ///
    /// # 返回值
    ///
    /// 恒为 `true`，即同意替换
    fn file_rename(&self, _name: &str) -> bool {
        true
    }
}

/// 发起导出：后台任务，进度经 export-progress 事件上报
///
/// - `app`: 应用句柄，事件与后台任务都用它
/// - `uuid`: 实例 uuid
/// - `config`: 导出设置，含目标文件名、格式与要打包的内容目录
///
/// # 返回值
///
/// 任务已开始返回 `Ok(())`；已有导出在跑、uuid 非法、格式或文件名为空返回错误文案
#[tauri::command]
pub fn export_run(app: AppHandle, uuid: String, config: ExportConfigDto) -> Result<(), String> {
    if EXPORT_RUNNING.swap(true, Ordering::SeqCst) {
        return Err("err.exportRunning".to_string());
    }

    let game = parse_instance(&uuid)?;
    let pack = match config.pack.as_str() {
        "curseforge" => ExportPackType::CurseForge,
        "modrinth" => ExportPackType::Modrinth,
        _ => {
            EXPORT_RUNNING.store(false, Ordering::SeqCst);
            return Err("err.exportPackType".to_string());
        }
    };
    if config.file.trim().is_empty() {
        EXPORT_RUNNING.store(false, Ordering::SeqCst);
        // 这里校验的是**导出文件名**，原来报的是 err.serverEmpty，即服务器为空，
        // 前端弹出来的文案牛头不对马嘴
        return Err("err.fileName".to_string());
    }

    // 在线模组：mods 目录文件与 mod_info.json 对上号的进 manifest / index
    let online_list = game.read_online_info();
    let mut online_by_name: std::collections::HashMap<
        String,
        mml_game::launcher::file_online_info_obj::OnlineInfoObj,
    > = std::collections::HashMap::new();
    for info in online_list.into_values() {
        if !info.modid.is_empty() && !info.fileid.is_empty() {
            online_by_name.insert(info.file.clone(), info);
        }
    }

    let mut mods = Vec::new();
    let mut select = Vec::new();

    if config.include_mods {
        for file in collect_files(&game.get_mods_path()) {
            let name = file
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            match online_by_name.remove(&name) {
                Some(info) => {
                    if let Some(item) = make_online_export(file, info, pack) {
                        mods.push(item);
                    }
                }
                // 无来源的模组直接进 overrides
                None => select.push(file),
            }
        }
    }
    if config.include_config {
        select.extend(collect_files(&game.get_config_path()));
    }
    if config.include_resource_packs {
        select.extend(collect_files(&game.get_resourcepacks_path()));
    }
    if config.include_shader_packs {
        select.extend(collect_files(&game.get_shaderpacks_path()));
    }

    let arg = ExportArg {
        file: PathBuf::from(&config.file),
        pack,
        archive: ArchiveType::Zip,
        mods,
        files: Vec::new(),
        unselect: Vec::new(),
        select,
        name: config.name,
        author: config.author,
        version: config.version,
        summary: config.summary,
        gui: Some(Arc::new(ExportProgressGui {
            app: app.clone(),
            file: config.file.clone(),
        })),
    };

    let save_file = config.file.clone();
    tokio::spawn(async move {
        let res = game.export(arg).await;
        EXPORT_RUNNING.store(false, Ordering::SeqCst);
        let event = match res {
            Ok(()) => ExportProgressDto {
                state: "done".to_string(),
                now: 0,
                total: 0,
                text: String::new(),
                error: None,
                file: save_file.clone(),
            },
            Err(e) => ExportProgressDto {
                state: "failed".to_string(),
                now: 0,
                total: 0,
                text: String::new(),
                error: Some(e.to_string()),
                file: save_file.clone(),
            },
        };
        emit_export_progress(&app, event);
    });

    Ok(())
}
