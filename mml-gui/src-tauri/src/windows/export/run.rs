//! 执行导出
//!
//! 在线导出按项目编号拉文件，普通导出直接打包本地目录；
//! 全程报进度，跑着的时候不重复入队。

use std::{path::{PathBuf}, sync::{Arc, atomic::{Ordering}}};

use crate::err_keys;

use mml_base::{archives::{ArchiveType, IBaseArchiveGui}};
use mml_game::game_export::{ExportArg, ExportPackType, OnlineFileExport};
use mml_game::launcher::file_online_info_obj::OnlineInfoObj;
use tauri::{AppHandle};

use crate::{dtos::export_dto::{ExportConfigDto, ExportProgressDto}};

use crate::windows::{config::resolve_under, instance_snapshot};
use super::list::{EXPORT_RUNNING, emit_export_progress, collect_files, online_of, export_meta};


/// 在线模组 → 导出条目：带上项目号，供 CurseForge manifest / Modrinth index 使用
///
/// - `file`: 模组文件路径
/// - `info`: 该文件在 mod_info.json 里的来源信息
/// - `pack`: 目标整合包格式
/// - `url`: 下载地址，前端可以改，所以不直接取 `info.url`
///
/// # 返回值
///
/// 导出条目；文件元数据读不到时返回 `None`
pub(super) fn make_online_export(
    file: PathBuf,
    info: OnlineInfoObj,
    pack: ExportPackType,
    url: String,
) -> Option<OnlineFileExport> {
    let meta = export_meta(&file)?;

    Some(OnlineFileExport {
        file,
        size: meta.size,
        pack,
        url,
        hash: meta.hash,
        info: Some(info),
    })
}

/// 只有下载地址、没有在线来源的条目
///
/// Modrinth index 里 `project` 留空、`downloads` 给这份地址；CurseForge 的清单格式
/// 没有下载地址，所以只在 Modrinth 用得上
///
/// - `file`: 文件路径
/// - `pack`: 目标整合包格式
/// - `url`: 下载地址
///
/// # 返回值
///
/// 导出条目；文件元数据读不到时返回 `None`
pub(super) fn make_plain_export(file: PathBuf, pack: ExportPackType, url: String) -> Option<OnlineFileExport> {
    let meta = export_meta(&file)?;

    Some(OnlineFileExport {
        file,
        size: meta.size,
        pack,
        url,
        hash: meta.hash,
        info: None,
    })
}

/// 进度回调：内核 `IBaseArchiveGui` → `export-progress` 事件
pub(super) struct ExportProgressGui {
    /// 应用句柄，事件经它转发到前端
    pub(super) app: AppHandle,
    /// 导出目标文件名，随每条进度一起下发
    pub(super) file: String,
}

/// 发起导出：后台任务，进度经 export-progress 事件上报
///
/// 勾选条目的去向按格式分：
/// - CurseForge：对得上 mod_info.json 的按项目号 / 文件号写进 manifest，其余整份进 overrides。
///   它的清单格式里没有下载地址，所以前端填的 URL 在这个格式下用不上
/// - Modrinth：填了下载地址的写进 index 的 files，有在线来源的再补上项目号；
///   没填地址的和目录展开出来的文件一并进 overrides
///
/// - `app`: 应用句柄，事件与后台任务都用它
/// - `uuid`: 实例 uuid
/// - `config`: 导出设置，含目标文件名、格式与勾选的模组 / 文件
///
/// # 返回值
///
/// 任务已开始返回 `Ok(())`；已有导出在跑、uuid 非法、格式或文件名为空返回错误文案
#[gui_macros::ipc_group("export")]
#[gui_macros::command]
pub fn export_run(app: AppHandle, uuid: String, config: ExportConfigDto) -> Result<(), String> {
    if EXPORT_RUNNING.swap(true, Ordering::SeqCst) {
        return Err(err_keys::EXPORT_RUNNING.to_string());
    }

    let game = instance_snapshot(&uuid)?;
    let pack = match config.pack.as_str() {
        "curseforge" => ExportPackType::CurseForge,
        "modrinth" => ExportPackType::Modrinth,
        "mmc" => ExportPackType::MMC,
        "hmcl" => ExportPackType::HMCL,
        _ => {
            EXPORT_RUNNING.store(false, Ordering::SeqCst);
            return Err(err_keys::EXPORT_PACK_TYPE.to_string());
        }
    };
    // 整包格式（MMC / HMCL）没有下载清单，选中的一律进包，前端填的地址在这两种下不作数
    let whole_pack = matches!(pack, ExportPackType::MMC | ExportPackType::HMCL);
    if config.file.trim().is_empty() {
        EXPORT_RUNNING.store(false, Ordering::SeqCst);
        // 这里校验的是**导出文件名**，原来报的是 err.serverEmpty，即服务器为空，
        // 前端弹出来的文案牛头不对马嘴
        return Err(err_keys::FILE_NAME.to_string());
    }

    let base = game.get_game_path();
    let online = online_of(&game);

    let mut mods = Vec::new();
    let mut files = Vec::new();
    let mut select = Vec::new();

    // 模组与文件走同一套路由，只是前端分了两块勾选
    for pick in config.mods.iter().chain(config.files.iter()) {
        let path = resolve_under(&base, &pick.path)?;

        // 目录整份进 overrides：里面每个文件的下载地址不会一致，填了也不作数
        if !path.is_file() {
            select.extend(collect_files(&path));
            continue;
        }

        // 整包格式：没有下载清单，文件一律进包
        if whole_pack {
            select.push(path);
            continue;
        }

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let url = pick.url.trim().to_string();
        let mut indexed = false;

        if let Some(info) = online.get(&name) {
            // 前端改过地址就用它，没填则退回来源里记的那个
            let url = if url.is_empty() {
                info.url.clone()
            } else {
                url
            };
            // CurseForge 只认项目号 / 文件号；Modrinth 还得有一个能下载的地址
            if pack != ExportPackType::Modrinth || !url.is_empty() {
                if let Some(item) = make_online_export(path.clone(), info.clone(), pack, url) {
                    mods.push(item);
                    indexed = true;
                }
            }
        } else if pack == ExportPackType::Modrinth && !url.is_empty() {
            // 本地来源但填了地址：Modrinth 也能当下载项写，不占 overrides 体积
            if let Some(item) = make_plain_export(path.clone(), pack, url) {
                files.push(item);
                indexed = true;
            }
        }

        if !indexed {
            select.push(path);
        }
    }

    let arg = ExportArg {
        file: PathBuf::from(&config.file),
        pack,
        archive: ArchiveType::Zip,
        mods,
        files,
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
