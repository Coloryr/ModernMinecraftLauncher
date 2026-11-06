//! 服务器实例生成
//!
//! 把本地实例导出成"在线实例"的服务端清单：MML 的 `server.json` 与 `server.json.sha1`，
//! 或 HMCL 的 `server-manifest.json`；要托管的文件一并写进用户选的文件夹。
//!
//! 独立窗口，标签 `mml-server_instance`，入口在主窗口实例右键菜单，
//! 见 `ServerInstanceWindow.vue`。窗口已开着时再次打开只推送目标实例，不重建。
//!
//! 生成是一次性的阻塞任务，关窗不会打断它：任务在后台跑完，期间再次生成会返回忙。

mod files;
mod mapping;
mod pack;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};

use mml_base::hash_helper::{HashType, gen_hash};
use mml_game::{
    launcher::{
        ModPackType, file_online_info_obj::OnlineInfoObj, instance_setting_obj::InstanceSettingObj,
    },
    other_launcher::hmcl_obj::HMCLServerFileObj,
    serverpack::{
        generate,
        serverpack_obj::{ServerArchiveItemObj, ServerBaseObj, ServerItemObj},
    },
};
use mml_names::names;
use tauri::{AppHandle, Emitter};

use super::instance_snapshot;
use crate::windows::export::list::custom_ids;
use crate::{
    dtos::log_dto::LogFocusDto,
    dtos::server_instance_dto::{
        ServerInstanceChangesDto, ServerInstanceIdDto, ServerInstanceMetaDto,
        ServerInstanceOptionsDto, ServerInstanceProgressDto, ServerInstanceResultDto,
        ServerInstanceSnapshotDto, ServerInstanceSnapshotEntryDto,
    },
    err_keys,
};

/// 生成任务进行中：同一时间只跑一个，避免两次生成往同一个文件夹里写
static RUNNING: AtomicBool = AtomicBool::new(false);

/// 占用生成名额，命令结束时自动释放
struct RunningGuard;

impl RunningGuard {
    /// 抢名额
    ///
    /// # 返回值
    ///
    /// 已有任务在跑返回 `err.serverInstanceBusy`
    fn acquire() -> Result<Self, String> {
        RUNNING
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .map(|_| Self)
            .map_err(|_| err_keys::SERVER_INSTANCE_BUSY.to_string())
    }
}

impl Drop for RunningGuard {
    /// 释放名额
    fn drop(&mut self) {
        RUNNING.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

/// 进度回调：已处理项数、总项数、当前处理的文件
type Progress<'a> = &'a dyn Fn(usize, usize, String);

/// 切换目标实例事件：窗口已存在时再次打开会推送，前端据此换实例
///
/// - `app`: 应用句柄，广播用它
/// - `event`: 目标实例 uuid
#[gui_macros::emit]
pub fn emit_server_instance_focus(app: &AppHandle, event: LogFocusDto) {
    let _ = app.emit(crate::listens::SERVER_INSTANCE_FOCUS, event);
}

/// 生成进度事件，事件名 `server-instance-progress`
///
/// - `app`: 应用句柄，广播用它
/// - `event`: 当前进度
#[gui_macros::emit]
pub fn emit_server_instance_progress(app: &AppHandle, event: ServerInstanceProgressDto) {
    let _ = app.emit(crate::listens::SERVER_INSTANCE_PROGRESS, event);
}

/// 生成服务器实例，把清单与托管文件写进指定文件夹
///
/// - `app`: 应用句柄，进度事件经它推给前端
/// - `uuid`: 源实例 uuid
/// - `options`: 生成选项，见 [`ServerInstanceOptionsDto`]
///
/// # 返回值
///
/// 见 [`ServerInstanceResultDto`]；参数不合法、扫描或写盘失败返回错误键或错误文案
#[gui_macros::ipc_group("server_instance")]
#[gui_macros::command]
pub async fn server_instance_generate(
    app: AppHandle,
    uuid: String,
    options: ServerInstanceOptionsDto,
) -> Result<ServerInstanceResultDto, String> {
    let guard = RunningGuard::acquire()?;
    // 生成过程中整个实例都在被读，删实例要等它结束
    let task = crate::windows::instance_task::begin(&uuid)?;
    let game = instance_snapshot(&uuid)?;
    let result = tokio::task::spawn_blocking(move || {
        let progress = |now: usize, total: usize, text: String| {
            emit_server_instance_progress(
                &app,
                ServerInstanceProgressDto {
                    now: now as u64,
                    total: total as u64,
                    text,
                },
            );
        };
        generate(&game, &options, &progress)
    })
    .await
    .map_err(|e| e.to_string())?;
    drop(task);
    drop(guard);
    result
}

/// 导出服务器包：MMC 结构的实例 zip，同时带 `game.json` 供 M²L 识别
///
/// - `uuid`: 源实例 uuid
/// - `options`: 生成选项，包名 / 发布地址 / 勾选都取自它
/// - `file`: 输出的 zip 路径，父目录不存在会创建
/// - `complete`: `true` 连同模组 / 资源包 / 配置目录一起打包，`false` 只带基础文件
///
/// # 返回值
///
/// 写盘成功返回 `Ok(())`；参数不合法、源实例本身是服务器实例或打包失败返回错误
#[gui_macros::ipc_group("server_instance")]
#[gui_macros::command]
pub async fn server_instance_export_pack(
    app: AppHandle,
    uuid: String,
    options: ServerInstanceOptionsDto,
    file: String,
    complete: bool,
) -> Result<(), String> {
    let file = file.trim().to_string();
    if file.is_empty() {
        return Err(err_keys::FILE_NAME.to_string());
    }
    // 导出期间整个实例都在被读，删实例要等它结束
    let task = crate::windows::instance_task::begin(&uuid)?;
    let game = instance_snapshot(&uuid)?;
    // 服务器实例本身就是同步结果，再导出会循环覆盖，直接拒绝
    if game.modpack_type == ModPackType::ServerPack {
        return Err(err_keys::SERVER_INSTANCE_FROM_SERVER.to_string());
    }
    let path = PathBuf::from(file);
    let progress: Arc<dyn Fn(usize, usize, String) + Send + Sync> =
        Arc::new(move |now: usize, total: usize, text: String| {
            emit_server_instance_progress(
                &app,
                ServerInstanceProgressDto {
                    now: now as u64,
                    total: total as u64,
                    text,
                },
            );
        });
    let result = tokio::task::spawn_blocking(move || {
        pack::export_pack(&game, &options, &path, complete, progress)
    })
    .await
    .map_err(|e| e.to_string())?;
    drop(task);
    result
}

/// 读取上次生成保存的名字 / 版本 / 说明 / 网址等
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 没有生成过时返回 `None`
#[gui_macros::ipc_group("server_instance")]
#[gui_macros::command]
pub async fn server_instance_get_meta(
    uuid: String,
) -> Result<Option<ServerInstanceMetaDto>, String> {
    let game = instance_snapshot(&uuid)?;
    Ok(mapping::IdMap::load(&game.get_base_path())
        .meta()
        .map(|meta| ServerInstanceMetaDto {
            name: meta.name.clone(),
            version: meta.version.clone(),
            text: meta.text.clone(),
            url: meta.url.clone(),
            format: meta.format.clone(),
            folder: meta.folder.clone(),
            base_files: meta.base_files.clone(),
            configs: meta.configs.as_ref().map(|configs| {
                configs
                    .iter()
                    .map(|item| crate::dtos::server_instance_dto::ServerInstanceConfigDto {
                        dir: item.dir.clone(),
                        mode: item.mode.clone(),
                        delete_old: item.delete_old,
                    })
                    .collect()
            }),
        }))
}

/// 读取已保存的自定义编号，给模组 / 资源包表格回填
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 每个已保存条目的路径与项目 / 文件编号
#[gui_macros::ipc_group("server_instance")]
#[gui_macros::command]
pub async fn server_instance_get_ids(uuid: String) -> Result<Vec<ServerInstanceIdDto>, String> {
    let game = instance_snapshot(&uuid)?;
    Ok(mapping::IdMap::load(&game.get_base_path())
        .snapshot()
        .into_iter()
        .map(|item| ServerInstanceIdDto {
            path: item.path,
            project_id: item.project_id,
            file_id: item.file_id,
        })
        .collect())
}

/// 读取上次生成保存的模组 / 资源包快照，给生成前的变化预览用
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 快照路径与 SHA1；没有生成过时 `generated` 为 false
#[gui_macros::ipc_group("server_instance")]
#[gui_macros::command]
pub async fn server_instance_get_snapshot(
    uuid: String,
) -> Result<ServerInstanceSnapshotDto, String> {
    let game = instance_snapshot(&uuid)?;
    let ids = mapping::IdMap::load(&game.get_base_path());
    let entries = |items: &BTreeMap<String, mapping::EntrySnapshot>| {
        items
            .values()
            .map(|item| ServerInstanceSnapshotEntryDto {
                path: item.path.clone(),
                sha1: item.sha1.clone(),
                optional: item.optional,
            })
            .collect::<Vec<_>>()
    };
    Ok(ServerInstanceSnapshotDto {
        generated: ids.loaded(),
        mods: entries(ids.mods()),
        packs: entries(ids.packs()),
    })
}

/// 配置文件的同步方式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigMode {
    /// 整个目录打成一个压缩包，客户端下载后解压
    Archive,
    /// 目录里的文件逐个列进清单，客户端按原路径下载
    Files,
}

impl ConfigMode {
    /// 按前端给的值解析同步方式
    ///
    /// - `value`: `archive` / `files`；空串按压缩包处理
    ///
    /// # 返回值
    ///
    /// 不认识时返回 `err.serverInstanceMode`
    fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "" | "archive" => Ok(ConfigMode::Archive),
            "files" => Ok(ConfigMode::Files),
            _ => Err(err_keys::SERVER_INSTANCE_MODE.to_string()),
        }
    }
}

/// 生成目标格式
enum Format {
    /// MML 的 `server.json`，配 `server.json.sha1`
    Mml,
    /// HMCL 的 `server-manifest.json`
    Hmcl,
}

impl Format {
    /// 按前端给的值解析格式
    ///
    /// - `value`: `mml` 或 `hmcl`
    ///
    /// # 返回值
    ///
    /// 不认识时返回 `err.serverInstanceFormat`
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "mml" => Ok(Format::Mml),
            "hmcl" => Ok(Format::Hmcl),
            _ => Err(err_keys::SERVER_INSTANCE_FORMAT.to_string()),
        }
    }
}

/// 一个要同步的配置目录
struct ConfigDir {
    /// 目录名，相对游戏目录，只允许顶层目录
    dir: String,
    /// 同步方式
    mode: ConfigMode,
    /// 压缩包同步时是否先删除目标目录
    delete_old: bool,
}

/// 本次要写进包里的内容
struct Contents {
    /// 勾选的模组，带手填的下载信息
    mods: Vec<files::ModPick>,
    /// 配置目录与各自的同步方式
    configs: Vec<ConfigDir>,
    /// 资源包文件，带手填的编号与地址
    packs: Vec<PackPick>,
}

/// 一个要同步的资源包
struct PackPick {
    /// 相对游戏目录的路径
    path: PathBuf,
    /// 手动填的下载地址
    url: String,
    /// 手动填的项目编号
    pid: String,
    /// 手动填的文件编号
    fid: String,
    /// 是否使用自定义编号
    custom: bool,
    /// 编号是否被用户改过
    edited: bool,
    /// 编号类型是否被用户改过
    type_changed: bool,
}

/// 校验并规范化配置目录
///
/// 只允许顶层目录与一级子目录，如 `config` / `config/setting`；更深的目录不接受。
///
/// - `value`: 前端给的目录名
///
/// # 返回值
///
/// 统一用 `/` 分隔的目录名；为空、超过两级或含空段 / `.` / `..` 时返回
/// `err.serverInstanceDir`
fn config_dir(value: &str) -> Result<String, String> {
    let dir = value.trim().replace('\\', "/");
    let parts: Vec<&str> = dir.split('/').collect();
    if parts.is_empty()
        || parts.len() > 2
        || parts
            .iter()
            .any(|part| part.is_empty() || *part == "." || *part == "..")
    {
        return Err(err_keys::SERVER_INSTANCE_DIR.to_string());
    }
    Ok(dir)
}

/// 生成入口：校验参数 → 展开勾选 → 按格式写盘
///
/// - `game`: 源实例
/// - `options`: 生成选项
/// - `progress`: 进度回调
///
/// # 返回值
///
/// 见 [`ServerInstanceResultDto`]
fn generate(
    game: &InstanceSettingObj,
    options: &ServerInstanceOptionsDto,
    progress: Progress<'_>,
) -> Result<ServerInstanceResultDto, String> {
    // 服务器实例本身就是同步结果，再导出会循环覆盖，直接拒绝
    if game.modpack_type == ModPackType::ServerPack {
        return Err(err_keys::SERVER_INSTANCE_FROM_SERVER.to_string());
    }
    let format = Format::parse(&options.format)?;
    let url = normalize_url(&options.url)?;
    let base = game.get_game_path();
    let out = prepare_folder(&game.get_base_path(), &options.folder)?;

    let mods = files::mods(&base, &options.mods)?;
    let mut configs = Vec::new();
    for item in &options.configs {
        let dir = config_dir(&item.dir)?;
        configs.push(ConfigDir {
            dir,
            mode: ConfigMode::parse(&item.mode)?,
            delete_old: item.delete_old,
        });
    }
    let packs = options
        .resource_packs
        .iter()
        .map(|pick| {
            Ok(PackPick {
                path: files::file(&base, &pick.path)?,
                url: pick.url.trim().to_string(),
                pid: pick.pid.trim().to_string(),
                fid: pick.fid.trim().to_string(),
                custom: pick.custom,
                edited: pick.edited,
                type_changed: pick.type_changed,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let has_base = matches!(format, Format::Mml) && !options.base_files.is_empty();
    if mods.is_empty() && configs.is_empty() && packs.is_empty() && !has_base {
        return Err(err_keys::SERVER_INSTANCE_EMPTY.to_string());
    }

    let contents = Contents {
        mods,
        configs,
        packs,
    };

    match format {
        Format::Mml => {
            let online = online_sources(game);
            generate_mml(
                game, &online, &base, &out, &url, options, &contents, progress,
            )
        }
        Format::Hmcl => generate_hmcl(game, &base, &out, &url, options, &contents, progress),
    }
}

/// 校验发布地址并去掉末尾的斜杠
///
/// - `value`: 前端填的地址
///
/// # 返回值
///
/// 规范化后的地址，拼托管文件时直接用；不是 http(s) 地址时返回
/// `err.serverInstanceUrl`
fn normalize_url(value: &str) -> Result<String, String> {
    let url = value.trim().trim_end_matches('/');
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"));
    match rest {
        Some(rest) if !rest.is_empty() => Ok(url.to_string()),
        _ => Err(err_keys::SERVER_INSTANCE_URL.to_string()),
    }
}

/// 准备输出文件夹，并确认它不在实例目录里
///
/// 写进实例目录会把刚生成的文件当成下一次的同步内容，升级流程也会把它们删掉。
///
/// - `instance`: 实例根目录
/// - `value`: 前端选的文件夹
///
/// # 返回值
///
/// 规范化后的文件夹；为空、建不出来或落在实例里时返回 `err.serverInstanceFolder`
fn prepare_folder(instance: &Path, value: &str) -> Result<PathBuf, String> {
    let folder = value.trim();
    if folder.is_empty() {
        return Err(err_keys::SERVER_INSTANCE_FOLDER.to_string());
    }
    let path = PathBuf::from(folder);
    fs::create_dir_all(&path).map_err(|_| err_keys::SERVER_INSTANCE_FOLDER.to_string())?;
    let path = path
        .canonicalize()
        .map_err(|_| err_keys::SERVER_INSTANCE_FOLDER.to_string())?;
    if !path.is_dir() {
        return Err(err_keys::SERVER_INSTANCE_FOLDER.to_string());
    }

    if let Ok(instance) = instance.canonicalize()
        && path.starts_with(&instance)
    {
        return Err(err_keys::SERVER_INSTANCE_FOLDER.to_string());
    }
    Ok(path)
}

/// 生成 MML 服务器实例
///
/// 模组与资源包有在线来源就用平台地址、不复制；没有的复制进输出文件夹由发布者托管。
/// 配置目录按各自的同步方式打包或逐个下发。
///
/// - `game`: 源实例
/// - `online`: 实例记录的在线来源，键是文件名
/// - `base`: 游戏目录
/// - `out`: 输出文件夹
/// - `url`: 发布地址，托管文件的下载地址按它拼
/// - `options`: 生成选项
/// - `contents`: 勾选展开后的内容
/// - `progress`: 进度回调
///
/// # 返回值
///
/// 见 [`ServerInstanceResultDto`]
#[allow(clippy::too_many_arguments)]
fn generate_mml(
    game: &InstanceSettingObj,
    online: &HashMap<String, OnlineInfoObj>,
    base: &Path,
    out: &Path,
    url: &str,
    options: &ServerInstanceOptionsDto,
    contents: &Contents,
    progress: Progress<'_>,
) -> Result<ServerInstanceResultDto, String> {
    // 父目录排在子目录前面：清单里的 Archives 按这个顺序，客户端先解父包再解子包
    let mut selected: Vec<&ConfigDir> = contents.configs.iter().collect();
    selected.sort_by(|a, b| {
        let depth = a.dir.matches('/').count();
        let other = b.dir.matches('/').count();
        depth.cmp(&other).then_with(|| a.dir.cmp(&b.dir))
    });

    // 先把目录内容列出来：进度总数与后面的打包都要用
    let mut config_files = Vec::new();
    for config in selected {
        let mut list = files::dir_files(base, &config.dir)?;
        // 父目录的包要排掉已单独选中的一级子目录，子目录自己的包负责那部分
        let prefix = format!("{}/", config.dir);
        let children: Vec<&str> = contents
            .configs
            .iter()
            .map(|item| item.dir.as_str())
            .filter(|dir| dir.starts_with(&prefix) && !dir[prefix.len()..].contains('/'))
            .collect();
        if !children.is_empty() {
            list.retain(|rel| !children.iter().any(|child| rel.starts_with(*child)));
        }
        config_files.push((config.dir.clone(), config.mode, config.delete_old, list));
    }

    let total = contents.mods.len()
        + config_files
            .iter()
            .map(|(_, mode, _, list)| match mode {
                ConfigMode::Archive => 1,
                ConfigMode::Files => list.len(),
            })
            .sum::<usize>()
        + contents.packs.len();

    let mut now = 0;
    let mut items = Vec::new();
    let mut written = BTreeSet::new();
    let mut ids = mapping::IdMap::load(&game.get_base_path());
    let mut changed = BTreeSet::new();

    // 基础文件：实例根目录下、与 .minecraft 同级的文件，打成一个固定包。
    // 客户端只在第一次同步时解压，后续版本不再拉取。
    let base_files = files::base_files(&game.get_base_path(), &options.base_files)?;
    let base_count = base_files.len();
    let base_item = if base_files.is_empty() {
        None
    } else {
        let name = "base.zip".to_string();
        let file = out.join(&name);
        files::zip_files(&game.get_base_path(), &base_files, &file)?;
        written.insert(name.clone());
        Some(ServerBaseObj {
            file: name.clone(),
            url: format!("{url}/{}", files::encode_url_path(&name)),
            sha1: Some(files::sha1(&file)?),
            sha256: None,
        })
    };

    // 图标也带一份：玩家导入时按清单里的 Icon 去 <发布地址>/icon.png 下载
    let icon = copy_icon(game, out)?;
    if let Some(name) = &icon {
        written.insert(name.clone());
    }
    let mut archived = 0;
    let mut archives = Vec::new();
    let mut mod_snapshot: BTreeMap<String, mapping::EntrySnapshot> = BTreeMap::new();

    for entry in &contents.mods {
        let wire = files::wire_path(&entry.path);
        let key = mod_key(base, &entry.path, &wire);
        progress(now, total, wire.clone());
        now += 1;

        // 只有地址能改，编号一律用实例记录的
        let recorded = online.get(&file_name(&entry.path));
        let download = pick(&entry.url, recorded.map(|info| info.url.as_str()));
        let pid = if entry.edited {
            entry.pid.as_str()
        } else {
            recorded.map(|info| info.modid.as_str()).unwrap_or_default()
        };
        let fid = if entry.edited {
            entry.fid.as_str()
        } else {
            recorded.map(|info| info.fileid.as_str()).unwrap_or_default()
        };

        // 地址优先，其次是"项目号 + 文件号"能解析出来的；都没有就只能自己托管
        let has_ids = recorded.is_some_and(|info| {
            !info.modid.trim().is_empty() || !info.fileid.trim().is_empty()
        });
        let custom = if entry.type_changed || entry.edited {
            entry.custom
        } else {
            !has_ids
        };
        // 自定义编号不能拿去平台解析；没有地址时仍然复制托管
        let online_download =
            !download.is_empty() || (!custom && !pid.is_empty() && !fid.is_empty());
        let mut item = if online_download {
            source_item(&wire, pid, fid, &download, base, &entry.path, custom)?
        } else {
            written.insert(top_of(&wire));
            hosted_item(
                base,
                out,
                &entry.path,
                url,
                custom,
                &entry.pid,
                &entry.fid,
            )?
        };
        // 可选模组由清单下发；客户端首次同步时装成禁用
        item.optional = entry.optional;
        let sha1 = item.sha1.clone().unwrap_or_default();
        mod_snapshot.insert(
            key.clone(),
            mapping::EntrySnapshot {
                path: wire.clone(),
                sha1: sha1.clone(),
                optional: entry.optional,
            },
        );
        if item.custom {
            let assigned = if entry.edited {
                ids.save_edited(&key, &wire, &sha1, &entry.pid, &entry.fid)?
            } else {
                ids.assign(&key, &wire, &sha1)
            };
            item.pid = Some(assigned.project_id);
            item.fid = Some(assigned.file_id);
            if assigned.changed {
                changed.insert(wire.clone());
            }
        }
        items.push(item);
    }
    let mod_changes = compare_mods(&ids, &mod_snapshot);
    ids.set_mods(mod_snapshot);

    for (dir, mode, delete_old, list) in &config_files {
        match mode {
            ConfigMode::Archive => {
                let name = generate::archive_name(dir);
                progress(now, total, name.clone());
                now += 1;

                let zip = out.join(&name);
                files::zip_dir(base, dir, list, &zip)?;
                let sha1 = files::sha1(&zip)?;
                let assigned =
                    ids.assign(&format!("config-archive:{dir}"), dir, &sha1);
                if assigned.changed {
                    changed.insert(dir.clone());
                }
                archives.push(ServerArchiveItemObj {
                    file: name.clone(),
                    dir: dir.clone(),
                    // 由用户在生成窗口里选：勾选时先删目标目录，不勾就解压覆盖
                    delete_old: *delete_old,
                    url: format!("{url}/{}", files::encode_url_path(&name)),
                    sha1: Some(sha1),
                    sha256: None,
                });
                archived += list.len();
                written.insert(name);
            }
            ConfigMode::Files => {
                for rel in list {
                    let wire = files::wire_path(rel);
                    progress(now, total, wire.clone());
                    now += 1;
                    let mut item = hosted_item(base, out, rel, url, true, "", "")?;
                    let assigned = ids.assign(
                        &format!("config:{wire}"),
                        &wire,
                        item.sha1.as_deref().unwrap_or_default(),
                    );
                    item.pid = Some(assigned.project_id);
                    item.fid = Some(assigned.file_id);
                    if assigned.changed {
                        changed.insert(wire.clone());
                        changed.insert(dir.clone());
                    }
                    items.push(item);
                    written.insert(top_of(&wire));
                }
            }
        }
    }

    let mut pack_snapshot: BTreeMap<String, mapping::EntrySnapshot> = BTreeMap::new();
    for pack in &contents.packs {
        let rel = &pack.path;
        let wire = files::wire_path(rel);
        let key = format!("pack:{wire}");
        progress(now, total, wire.clone());
        now += 1;

        // 资源包同理：手填地址 / 编号优先，其次用实例记录
        let recorded = online.get(&file_name(rel));
        let download = pick(&pack.url, recorded.map(|info| info.url.as_str()));
        let pid = if pack.edited {
            pack.pid.as_str()
        } else {
            recorded.map(|info| info.modid.as_str()).unwrap_or_default()
        };
        let fid = if pack.edited {
            pack.fid.as_str()
        } else {
            recorded.map(|info| info.fileid.as_str()).unwrap_or_default()
        };
        let has_ids = recorded.is_some_and(|info| {
            !info.modid.trim().is_empty() || !info.fileid.trim().is_empty()
        });
        let custom = if pack.type_changed || pack.edited {
            pack.custom
        } else {
            !has_ids
        };
        // 自定义编号不能拿去平台解析；没有地址时仍然复制托管
        let online_download =
            !download.is_empty() || (!custom && !pid.is_empty() && !fid.is_empty());
        let mut item = if online_download {
            source_item(&wire, pid, fid, &download, base, rel, custom)?
        } else {
            written.insert(top_of(&wire));
            hosted_item(base, out, rel, url, custom, &pack.pid, &pack.fid)?
        };
        let sha1 = item.sha1.clone().unwrap_or_default();
        pack_snapshot.insert(
            key.clone(),
            mapping::EntrySnapshot {
                path: wire.clone(),
                sha1: sha1.clone(),
                optional: false,
            },
        );
        if item.custom {
            let assigned = if pack.edited {
                ids.save_edited(&key, &wire, &sha1, &pack.pid, &pack.fid)?
            } else {
                ids.assign(&key, &wire, &sha1)
            };
            item.pid = Some(assigned.project_id);
            item.fid = Some(assigned.file_id);
            if assigned.changed {
                changed.insert(wire.clone());
            }
        }
        items.push(item);
    }
    let pack_changes = compare_packs(&ids, &pack_snapshot);
    ids.set_packs(pack_snapshot);

    let count = items.len() + archived + base_count;
    let mut pack = generate::build_server_pack(
        game,
        &options.name,
        &options.version,
        &options.text,
        items,
        archives,
    );
    pack.icon = icon;
    pack.base = base_item;
    let bytes = serde_json::to_vec_pretty(&pack).map_err(|e| e.to_string())?;
    // 客户端拿本地 server.json 的 SHA1 与 server.json.sha1 逐字比对，这里不能带换行
    let sha1 = gen_hash(HashType::Sha1, &bytes);
    write(out, names::SERVER_FILE, &bytes)?;
    write(
        out,
        &format!("{}{}", names::SERVER_FILE, names::SHA1_DOT_EXT),
        sha1.as_bytes(),
    )?;
    ids.set_meta(mapping::Meta {
        name: options.name.clone(),
        version: options.version.clone(),
        text: options.text.clone(),
        url: url.to_string(),
        format: "mml".to_string(),
        folder: options.folder.clone(),
        base_files: options.base_files.clone(),
        configs: Some(
            options
                .configs
                .iter()
                .map(|item| mapping::MetaConfig {
                    dir: item.dir.clone(),
                    mode: item.mode.clone(),
                    delete_old: item.delete_old,
                })
                .collect(),
        ),
    });
    ids.save(&game.get_base_path())?;

    Ok(ServerInstanceResultDto {
        manifest: names::SERVER_FILE.to_string(),
        files: written.into_iter().collect(),
        count,
        changed: changed.into_iter().collect(),
        mod_changes,
        pack_changes,
    })
}

/// 生成 HMCL 服务器实例
///
/// HMCL 没有压缩包，也没有"按项目号去平台下载"这条路，所有文件都得由服务端托管；
/// 配置目录的同步方式对它不起作用，一律逐文件复制。
///
/// - `game`: 源实例，加载器组件从它取
/// - `base`: 游戏目录
/// - `out`: 输出文件夹
/// - `url`: 发布地址，写进 `file_api`，客户端按 `file_api/路径` 下载
/// - `options`: 生成选项
/// - `contents`: 勾选展开后的内容
/// - `progress`: 进度回调
///
/// # 返回值
///
/// 见 [`ServerInstanceResultDto`]；加载器 HMCL 表达不了时返回
/// `err.serverInstanceLoader`
fn generate_hmcl(
    game: &InstanceSettingObj,
    base: &Path,
    out: &Path,
    url: &str,
    options: &ServerInstanceOptionsDto,
    contents: &Contents,
    progress: Progress<'_>,
) -> Result<ServerInstanceResultDto, String> {
    let mut ids = mapping::IdMap::load(&game.get_base_path());
    let addons =
        generate::hmcl_addons(game).ok_or_else(|| err_keys::SERVER_INSTANCE_LOADER.to_string())?;

    // 模组快照只统计模组目录，用于生成后的新增 / 删除 / 修改汇总
    let mut mod_snapshot: BTreeMap<String, mapping::EntrySnapshot> = BTreeMap::new();
    for pick in &contents.mods {
        let rel = &pick.path;
        let wire = files::wire_path(rel);
        let key = mod_key(base, rel, &wire);
        mod_snapshot.insert(
            key,
            mapping::EntrySnapshot {
                path: wire,
                sha1: files::sha1(&base.join(rel))?,
                optional: pick.optional,
            },
        );
    }
    let mod_changes = compare_mods(&ids, &mod_snapshot);
    ids.set_mods(mod_snapshot);

    let mut pack_snapshot: BTreeMap<String, mapping::EntrySnapshot> = BTreeMap::new();
    for pick in &contents.packs {
        let rel = &pick.path;
        let wire = files::wire_path(rel);
        pack_snapshot.insert(
            format!("pack:{wire}"),
            mapping::EntrySnapshot {
                path: wire,
                sha1: files::sha1(&base.join(rel))?,
                optional: false,
            },
        );
    }
    let pack_changes = compare_packs(&ids, &pack_snapshot);
    ids.set_packs(pack_snapshot);

    // 模组 + 各配置目录里的文件 + 资源包，按路径去重
    let mut all: BTreeSet<PathBuf> = contents.mods.iter().map(|item| item.path.clone()).collect();
    for config in &contents.configs {
        all.extend(files::dir_files(base, &config.dir)?);
    }
    all.extend(contents.packs.iter().map(|pick| pick.path.clone()));

    let mut list = Vec::new();
    let mut top = BTreeSet::new();
    let total = all.len();
    for (now, rel) in all.iter().enumerate() {
        let wire = files::wire_path(rel);
        progress(now, total, wire.clone());

        files::copy_into(base, out, rel)?;
        top.insert(top_of(&wire));
        list.push(HMCLServerFileObj {
            path: wire,
            hash: files::sha1(&base.join(rel))?,
        });
    }

    let pack = generate::build_hmcl_pack(
        &options.name,
        &options.version,
        &options.text,
        url,
        addons,
        list,
    );
    let bytes = serde_json::to_vec_pretty(&pack).map_err(|e| e.to_string())?;
    write(out, names::SERVER_MANIFEST_FILE, &bytes)?;
    ids.set_meta(mapping::Meta {
        name: options.name.clone(),
        version: options.version.clone(),
        text: options.text.clone(),
        url: url.to_string(),
        format: "hmcl".to_string(),
        folder: options.folder.clone(),
        base_files: options.base_files.clone(),
        configs: Some(
            options
                .configs
                .iter()
                .map(|item| mapping::MetaConfig {
                    dir: item.dir.clone(),
                    mode: item.mode.clone(),
                    delete_old: item.delete_old,
                })
                .collect(),
        ),
    });
    ids.save(&game.get_base_path())?;

    Ok(ServerInstanceResultDto {
        manifest: names::SERVER_MANIFEST_FILE.to_string(),
        files: top.into_iter().collect(),
        count: total,
        changed: Vec::new(),
        mod_changes,
        pack_changes,
    })
}

/// 取手填值，没填就用实例记录的
///
/// - `typed`: 手动填的内容，空串表示没填
/// - `recorded`: 实例记录的在线来源，可能没有
///
/// # 返回值
///
/// 去掉首尾空白后的值；两边都没有时是空串
fn pick(typed: &str, recorded: Option<&str>) -> String {
    let typed = typed.trim();
    if !typed.is_empty() {
        return typed.to_string();
    }
    recorded
        .map(|value| value.trim().to_string())
        .unwrap_or_default()
}

/// 取模组的稳定逻辑键：优先真实 modid，读不到时退回路径
///
/// - `base`: 游戏目录
/// - `rel`: 模组相对游戏目录的路径
/// - `wire`: 正斜杠相对路径
///
/// # 返回值
///
/// 形如 `mod:jei`；读不到元数据时是 `mod:mods/foo.jar`
fn mod_key(base: &Path, rel: &Path, wire: &str) -> String {
    let modid = mml_game::game_mods::read_mod_info(base.join(rel))
        .ok()
        .and_then(|item| {
            item.info
                .iter()
                .map(|info| info.mod_id.trim().to_string())
                .find(|modid| !modid.is_empty())
        });
    modid
        .map(|modid| format!("mod:{modid}"))
        .unwrap_or_else(|| format!("mod:{wire}"))
}

/// 对比上次快照，算新增 / 删除 / 修改的模组
///
/// - `ids`: 当前映射
/// - `current`: 本次实际写进清单的模组快照
///
/// # 返回值
///
/// 按路径列出的三类变化；第一次生成没有旧快照时全部为空
fn compare_mods(
    ids: &mapping::IdMap,
    current: &BTreeMap<String, mapping::EntrySnapshot>,
) -> ServerInstanceChangesDto {
    compare_snapshot(ids.loaded(), ids.mods(), current)
}

/// 对比上次资源包快照，算新增 / 删除 / 修改
///
/// - `ids`: 当前映射
/// - `current`: 本次实际写进清单的资源包快照
///
/// # 返回值
///
/// 按路径列出的三类变化；第一次生成没有旧快照时全部为空
fn compare_packs(
    ids: &mapping::IdMap,
    current: &BTreeMap<String, mapping::EntrySnapshot>,
) -> ServerInstanceChangesDto {
    compare_snapshot(ids.loaded(), ids.packs(), current)
}

/// 对比旧快照与本次快照
///
/// - `loaded`: 映射文件是否来自上次生成
/// - `old`: 旧快照
/// - `current`: 本次快照
///
/// # 返回值
///
/// 按路径列出的新增 / 删除 / 修改
fn compare_snapshot(
    loaded: bool,
    old: &BTreeMap<String, mapping::EntrySnapshot>,
    current: &BTreeMap<String, mapping::EntrySnapshot>,
) -> ServerInstanceChangesDto {
    let mut changes = ServerInstanceChangesDto::default();
    if !loaded {
        return changes;
    }
    for (key, item) in current {
        match old.get(key) {
            None => changes.added.push(item.path.clone()),
            Some(previous) if previous.sha1 != item.sha1 || previous.path != item.path => {
                changes.modified.push(item.path.clone());
            }
            Some(_) => {}
        }
    }
    for (key, item) in old {
        if !current.contains_key(key) {
            changes.removed.push(item.path.clone());
        }
    }
    changes
}

/// 在线下载的条目：不复制文件，客户端按地址或项目号自己下载
///
/// - `wire`: 相对游戏目录的正斜杠路径
/// - `pid`: 项目号，可为空
/// - `fid`: 文件号，可为空
/// - `download`: 下载地址，可为空
/// - `base`: 游戏目录
/// - `rel`: 相对游戏目录的路径
/// - `custom`: 是否按自定义编号处理；没有编号时自动生成
///
/// # 返回值
///
/// 见 [`ServerItemObj`]；算哈希失败返回错误文案
fn source_item(
    wire: &str,
    pid: &str,
    fid: &str,
    download: &str,
    base: &Path,
    rel: &Path,
    custom: bool,
) -> Result<ServerItemObj, String> {
    let pid = pid.trim();
    let fid = fid.trim();
    let (pid, fid, custom) = if custom && pid.is_empty() && fid.is_empty() {
        let (pid, fid) = custom_ids(wire);
        (Some(pid), Some(fid), true)
    } else {
        (
            Some(pid.to_string()).filter(|id| !id.is_empty()),
            Some(fid.to_string()).filter(|id| !id.is_empty()),
            custom,
        )
    };
    Ok(ServerItemObj {
        file: wire.to_string(),
        pid,
        fid,
        url: Some(download.trim().to_string()).filter(|url| !url.is_empty()),
        sha1: Some(files::sha1(&base.join(rel))?),
        sha256: None,
        custom,
        optional: false,
    })
}

/// 一个要托管的文件：复制进输出文件夹并生成清单条目
///
/// - `base`: 游戏目录
/// - `out`: 输出文件夹
/// - `rel`: 相对游戏目录的路径
/// - `url`: 发布地址
/// - `custom`: 是否生成自定义编号
///
/// # 返回值
///
/// 见 [`ServerItemObj`]；复制或算哈希失败返回错误文案
fn hosted_item(
    base: &Path,
    out: &Path,
    rel: &Path,
    url: &str,
    custom: bool,
    pid: &str,
    fid: &str,
) -> Result<ServerItemObj, String> {
    files::copy_into(base, out, rel)?;
    let wire = files::wire_path(rel);
    let pid = pid.trim();
    let fid = fid.trim();
    let (pid, fid, custom) = if !pid.is_empty() || !fid.is_empty() {
        (
            Some(pid.to_string()).filter(|id| !id.is_empty()),
            Some(fid.to_string()).filter(|id| !id.is_empty()),
            custom,
        )
    } else if custom {
        let (pid, fid) = custom_ids(&wire);
        (Some(pid), Some(fid), true)
    } else {
        (None, None, false)
    };
    Ok(ServerItemObj {
        file: wire.clone(),
        pid,
        fid,
        url: Some(format!("{url}/{}", files::encode_url_path(&wire))),
        sha1: Some(files::sha1(&base.join(rel))?),
        sha256: None,
        custom,
        optional: false,
    })
}

/// 取相对路径的第一段，用来汇报写出了哪些顶层条目
///
/// - `wire`: 已经转成正斜杠的相对路径
///
/// # 返回值
///
/// 形如 `config`；一层都没有时是空串
fn top_of(wire: &str) -> String {
    wire.split('/').next().unwrap_or_default().to_string()
}

/// 实例记录的在线来源，键是文件名
///
/// 有下载地址、或有项目号与文件号都算：前者直接下发地址，后者由客户端按编号解析。
///
/// - `game`: 源实例
///
/// # 返回值
///
/// 文件名 → 来源信息；一份都没有时是空表
fn online_sources(game: &InstanceSettingObj) -> HashMap<String, OnlineInfoObj> {
    let mut map = HashMap::new();
    for info in game.read_online_info().into_values() {
        let indexed = !info.modid.trim().is_empty() && !info.fileid.trim().is_empty();
        if indexed || !info.url.trim().is_empty() {
            map.insert(info.file.clone(), info);
        }
    }
    map
}

/// 取相对路径的文件名，清单按文件名查在线来源
///
/// - `rel`: 相对游戏目录的路径
///
/// # 返回值
///
/// 文件名；没有文件名时是空串
fn file_name(rel: &Path) -> String {
    rel.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// 把实例图标复制进输出文件夹
///
/// 玩家导入时按清单里的 `Icon` 去 `<发布地址>/<文件名>` 下载，所以这里只放一份文件；
/// 实例没有图标文件时返回 `None`。
///
/// - `game`: 源实例
/// - `out`: 输出文件夹
///
/// # 返回值
///
/// 写进清单的文件名；没有图标时是 `None`
fn copy_icon(game: &InstanceSettingObj, out: &Path) -> Result<Option<String>, String> {
    let source = game.get_icon_file_or_default();
    if !source.is_file() {
        return Ok(None);
    }
    let name = names::ICON_FILE.to_string();
    fs::copy(&source, out.join(&name)).map_err(|e| e.to_string())?;
    Ok(Some(name))
}

/// 写一个文件到输出文件夹
///
/// - `out`: 输出文件夹
/// - `name`: 文件名
/// - `bytes`: 文件内容
///
/// # 返回值
///
/// 写盘成功返回 `Ok(())`，失败返回错误文案
fn write(out: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    fs::write(out.join(name), bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "../../../tests/unit/windows_server_instance_mod_tests.rs"]
mod tests;
