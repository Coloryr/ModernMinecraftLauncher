//! 服务器实例生成
//!
//! 把本地实例导出成"在线实例"的服务端清单：MML 的 `server.json` 与 `server.json.sha1`，
//! 要托管的文件一并写进用户选的文件夹。HMCL 玩家改用「生成服务器包」的 MMC 包，不再生成
//! `server-manifest.json`。
//!
//! 独立窗口，标签 `mml-server_instance`，入口在主窗口实例右键菜单，
//! 见 `ServerInstanceWindow.vue`。窗口已开着时再次打开只推送目标实例，不重建。
//!
//! 生成是一次性的阻塞任务，关窗不会打断它：任务在后台跑完，期间再次生成会返回忙。
//!
//! 实现分在 `config.rs`（同步选项与配置快照）与 `generate.rs`（清单拼装与写盘），
//! 本文件保留 IPC 命令、进度事件与忙碌守卫。

mod config;
mod files;
mod generate;
mod mapping;
mod pack;

use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

use mml_game::launcher::ModPackType;
use tauri::{AppHandle, Emitter};

use super::instance_snapshot;
use crate::{
    dtos::log_dto::LogFocusDto,
    dtos::server_instance_dto::{
        ServerInstanceConfigDiffDto, ServerInstanceCustomDto, ServerInstanceIdDto,
        ServerInstanceMetaDto, ServerInstanceOptionsDto, ServerInstanceProgressDto,
        ServerInstanceResultDto, ServerInstanceSnapshotDto, ServerInstanceSnapshotEntryDto,
    },
    err_keys,
};

use self::config::{ConfigMode, collect_config_files};
use self::generate::generate;

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
            folder: meta.folder.clone(),
            base_files: meta.base_files.clone(),
            configs: meta.configs.as_ref().map(|configs| {
                configs
                    .iter()
                    .map(|item| crate::dtos::server_instance_dto::ServerInstanceConfigDto {
                        dir: item.dir.clone(),
                        mode: item.mode.clone(),
                        exclude: item.exclude.clone(),
                        delete: item.delete.clone(),
                        delete_old: item.delete_old,
                    })
                    .collect()
            }),
            mod_texts: meta.mod_texts.clone(),
            custom: meta
                .custom
                .iter()
                .map(|item| ServerInstanceCustomDto {
                    path: item.path.clone(),
                    action: item.action.clone(),
                    content: item.content.clone(),
                })
                .collect(),
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

/// 配置文件相对上次生成的变化
///
/// 对比上次生成保存的 `config_files` 快照：新增 / 内容变化 / 已删除，
/// 另给一份当前参与同步的全部路径，打开时的搜索与分组都用它。
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 见 [`ServerInstanceConfigDiffDto`]；没生成过或快照为空时 `generated` 为 `false`
#[gui_macros::ipc_group("server_instance")]
#[gui_macros::command]
pub async fn server_instance_config_diff(
    uuid: String,
) -> Result<ServerInstanceConfigDiffDto, String> {
    let game = instance_snapshot(&uuid)?;
    let ids = mapping::IdMap::load(&game.get_base_path());
    let Some(meta) = ids.meta() else {
        return Ok(ServerInstanceConfigDiffDto::default());
    };
    let saved = meta.config_files.clone();
    if saved.is_empty() {
        return Ok(ServerInstanceConfigDiffDto::default());
    }
    let dirs: Vec<(String, ConfigMode)> = meta
        .configs
        .as_ref()
        .map(|configs| {
            configs
                .iter()
                .filter_map(|item| {
                    ConfigMode::parse(&item.mode)
                        .ok()
                        .map(|mode| (item.dir.clone(), mode))
                })
                .collect()
        })
        .unwrap_or_default();
    let base = game.get_game_path();
    tokio::task::spawn_blocking(move || {
        let current = collect_config_files(
            &base,
            dirs.iter().map(|(dir, mode)| (dir.as_str(), *mode)),
            false,
        )
        .unwrap_or_default();
        let mut added = Vec::new();
        let mut modified = Vec::new();
        for (path, sha1) in &current {
            match saved.get(path) {
                None => added.push(path.clone()),
                Some(old) if old != sha1 => modified.push(path.clone()),
                _ => {}
            }
        }
        let deleted = saved
            .keys()
            .filter(|path| !current.contains_key(*path))
            .cloned()
            .collect();
        Ok::<_, String>(ServerInstanceConfigDiffDto {
            generated: true,
            added,
            modified,
            deleted,
            files: current.into_keys().collect(),
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
#[path = "../../../tests/unit/windows_server_instance_mod_tests.rs"]
mod tests;