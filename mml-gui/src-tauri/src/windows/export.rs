//! 实例导出窗口：导出为 CurseForge / Modrinth 标准整合包
//!
//! 打包逻辑在内核 `mml_game::game_export` 的 `InstanceSettingObj::export`，
//! 这里只做实例文件到在线清单 / overrides 的收集与任务托管：
//! `export_run` 后台跑导出，进度经 `export-progress` 事件上报前端。

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::err_keys;

use mml_base::{
    archives::{ArchiveType, IBaseArchiveGui},
    file_item::FileHash,
    hash_helper::{self, HashType},
};
use mml_game::game_export::{ExportArg, ExportPackType, OnlineFileExport};
use mml_game::launcher::file_online_info_obj::OnlineInfoObj;
use mml_game::launcher::instance_setting_obj::InstanceSettingObj;
use mml_game::loader::LoaderType;
use mml_sys::path_helper;
use tauri::{AppHandle, Emitter};

use crate::{
    dtos::export_dto::{ExportConfigDto, ExportEntryDto, ExportInfoDto, ExportProgressDto},
    dtos::log_dto::LogFocusDto,
    listens,
};

use super::{config::resolve_under, instance_snapshot};

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

/// 目录是否含至少一个文件，递归统计
///
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

/// 读 mod_info.json 里带在线来源的那些，键是文件名
///
/// 判据是 modid 与 fileid 都非空：只有一个的话拼不出清单项。
///
/// - `game`: 实例设置快照
///
/// # 返回值
///
/// 文件名 → 来源信息
fn online_of(game: &InstanceSettingObj) -> HashMap<String, OnlineInfoObj> {
    let mut map = HashMap::new();
    for info in game.read_online_info().into_values() {
        if !info.modid.is_empty() && !info.fileid.is_empty() {
            map.insert(info.file.clone(), info);
        }
    }
    map
}

/// 列一个目录的直接子项，路径转成相对游戏目录，顺带补上在线来源
///
/// - `dir`: 待列的目录
/// - `base`: 游戏目录，即 `.minecraft`
/// - `online`: 在线来源表，见 [`online_of`]
///
/// # 返回值
///
/// 见 [`ExportEntryDto`]；目录在前、同类按路径排序，读不到目录返回内核错误串
fn list_entries(
    dir: &Path,
    base: &Path,
    online: &HashMap<String, OnlineInfoObj>,
) -> Result<Vec<ExportEntryDto>, String> {
    let mut list = Vec::new();

    for item in std::fs::read_dir(dir).map_err(|err| err.to_string())? {
        let Ok(item) = item else {
            continue;
        };
        let path = item.path();
        let Ok(rel) = path.strip_prefix(base) else {
            continue;
        };
        let is_dir = path.is_dir();
        let info = online.get(&item.file_name().to_string_lossy().to_string());

        list.push(ExportEntryDto {
            path: rel.to_string_lossy().replace('\\', "/"),
            dir: is_dir,
            size: if is_dir {
                0
            } else {
                item.metadata().map(|meta| meta.len()).unwrap_or(0)
            },
            url: info.map(|x| x.url.clone()).unwrap_or_default(),
            mod_id: info.map(|x| x.modid.clone()),
            file_id: info.map(|x| x.fileid.clone()),
        });
    }

    list.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.path.cmp(&b.path)));
    Ok(list)
}

/// 获取实例导出信息：实例元数据加 `mods` 目录下的模组
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 见 [`ExportInfoDto`]；uuid 非法或实例不存在返回错误文案
#[tauri::command]
pub fn export_get_info(uuid: String) -> Result<ExportInfoDto, String> {
    let game = instance_snapshot(&uuid)?;
    let online = online_of(&game);
    let mods = list_entries(&game.get_mods_path(), &game.get_game_path(), &online)?;

    Ok(ExportInfoDto {
        name: game.name.clone(),
        custom_loader: game.loader == LoaderType::Custom,
        mods,
    })
}

/// 浏览游戏目录下的一个目录，给"文件选择"当懒加载的树用
///
/// - `uuid`: 实例 uuid
/// - `dir`: 相对游戏目录的路径；空串表示游戏目录本身
///
/// # 返回值
///
/// 该目录的直接子项；路径非法返回 `err.fileName`，目录不存在返回 `err.fileNotFound`
#[tauri::command]
pub fn export_list_dir(uuid: String, dir: String) -> Result<Vec<ExportEntryDto>, String> {
    let game = instance_snapshot(&uuid)?;
    let base = game.get_game_path();

    // 根目录不走路径守卫：那里允许空串
    let path = if dir.trim().is_empty() {
        base.clone()
    } else {
        resolve_under(&base, &dir)?
    };
    if !path.is_dir() {
        return Err(err_keys::FILE_NOT_FOUND.to_string());
    }

    let online = online_of(&game);
    list_entries(&path, &base, &online)
}

/// 算导出条目要用的大小与校验值
///
/// - `file`: 文件路径
///
/// # 返回值
///
/// 文件大小与 sha1 + sha512；读不到文件返回 `None`
fn export_meta(file: &Path) -> Option<(usize, FileHash)> {
    let meta = std::fs::metadata(file).ok()?;
    let sha1 = hash_helper::gen_hash_from_file(HashType::Sha1, file).unwrap_or_default();
    let sha512 = hash_helper::gen_hash_from_file(HashType::Sha512, file).unwrap_or_default();

    Some((meta.len() as usize, FileHash::Sha1Sha512(sha1, sha512)))
}

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
fn make_online_export(
    file: PathBuf,
    info: OnlineInfoObj,
    pack: ExportPackType,
    url: String,
) -> Option<OnlineFileExport> {
    let (size, hash) = export_meta(&file)?;

    Some(OnlineFileExport {
        file,
        size,
        pack,
        url,
        hash,
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
fn make_plain_export(file: PathBuf, pack: ExportPackType, url: String) -> Option<OnlineFileExport> {
    let (size, hash) = export_meta(&file)?;

    Some(OnlineFileExport {
        file,
        size,
        pack,
        url,
        hash,
        info: None,
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
#[tauri::command]
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
