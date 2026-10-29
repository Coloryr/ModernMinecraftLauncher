//! 资源管理窗口的「自动标记」：按文件 SHA512 去 Modrinth 认模组，回填在线信息
//!
//! 在线信息是更新检查的底座：没有它就认不出项目号与文件号。手动放进 mods 目录的包
//! 只能按文件哈希认人，走 Modrinth 的 `version_files`（一次一批哈希）。
//!
//! 认不出来的（不在 Modrinth 上、或哈希对不上）只计数，不当失败 —— 一个整合包里
//! 总有几个只在 CurseForge 发的包。SHA512 得现读每个 jar，与扫描 mods 目录同一代价，
//! 所以逐条报进度，关窗时在两条之间收手。

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use mml_base::hash_helper::{self, HashType};
use mml_names::names;
use mml_net::modrinth_api;
use tauri::{Emitter, WebviewWindow};

use crate::dtos::{ModAutoMarkDto, ModAutoMarkProgressDto};
use crate::err_keys;
use crate::listens;
use crate::log_text::LogText;

use super::mods::sha1_of;
use super::parse_instance;

/// 一次问多少个哈希：Modrinth 的 `version_files` 吃一批，太大也没意义
const HASH_BATCH: usize = 50;

/// 一个待标记的模组
struct Target {
    /// 现在的 SHA1：用来删掉旧条目（在线信息按 SHA1 认人）
    sha1: String,
    /// jar 的绝对路径：现读 SHA512 用
    path: PathBuf,
}

/// 自动标记进度事件，事件名 `resource-mods-auto-mark-progress`
///
/// - `window`: 发起这次标记的那个资源窗口，由 Tauri 注入
/// - `dto`: 进度载荷，已完成与总数
#[gui_macros::emit]
pub fn emit_resource_mods_auto_mark_progress(window: &WebviewWindow, dto: ModAutoMarkProgressDto) {
    let _ = window.emit(listens::RESOURCE_MODS_AUTO_MARK_PROGRESS, dto);
}

/// 自动标记：按 SHA512 认 Modrinth 上的模组，回填在线信息
///
/// - `window`: 发起这次标记的那个资源窗口，进度只发给它
/// - `uuid`: 实例 uuid
/// - `overwrite`: 是否连已有在线信息的模组一起重认；false 只补没有的那些
///
/// # 返回值
///
/// 见 [`ModAutoMarkDto`]：认出来的 / 没认出来的 / 读文件失败的条数；
/// 实例不存在、模组列表还没加载或期间窗口被关返回错误串
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub async fn resource_mods_auto_mark(
    window: WebviewWindow,
    uuid: String,
    overwrite: bool,
) -> Result<ModAutoMarkDto, String> {
    let model = super::model::window_model(&uuid)?;
    let instance = parse_instance(&uuid)?;
    let op = model.begin();
    let token = op.token();

    // 选目标：解析失败的条目本来就认不出来；不覆盖时只挑还没有在线信息的
    let targets: Vec<Target> = model
        .with_cache(|cache| {
            let known: HashSet<String> = cache
                .online
                .values()
                .filter(|info| !info.sha1.is_empty() && !info.modid.is_empty())
                .map(|info| info.sha1.clone())
                .collect();
            cache
                .list
                .iter()
                .filter(|item| !item.fail)
                .filter(|item| overwrite || !known.contains(&sha1_of(&item.hash)))
                .map(|item| Target {
                    sha1: sha1_of(&item.hash),
                    path: item.file.clone(),
                })
                .collect()
        })
        .ok_or_else(|| err_keys::MODS_NOT_LOADED.to_string())?;

    let total = targets.len() as u32;
    emit_resource_mods_auto_mark_progress(&window, ModAutoMarkProgressDto { done: 0, total });

    // 1) 现读 SHA512：与扫描同一代价，逐条报进度，关窗就收手
    let mut by_hash: HashMap<String, Target> = HashMap::new();
    let mut failed = 0u32;
    for (index, target) in targets.into_iter().enumerate() {
        if token.is_cancelled() {
            return Err(err_keys::CANCELLED.to_string());
        }
        let path = target.path.clone();
        let hash = tokio::task::spawn_blocking(move || {
            hash_helper::gen_hash_from_file(HashType::Sha512, &path)
        })
        .await
        .map_err(|err| err.to_string())?;
        match hash {
            Ok(hash) => {
                by_hash.insert(hash, target);
            }
            Err(err) => {
                failed += 1;
                mml_log::warn(
                    LogText::AutoMarkReadFailed {
                        path: target.path.display().to_string(),
                        error: err.to_string(),
                    }
                    .text(),
                );
            }
        }
        emit_resource_mods_auto_mark_progress(
            &window,
            ModAutoMarkProgressDto {
                done: index as u32 + 1,
                total,
            },
        );
    }

    // 2) 分批问 Modrinth：一批 50 个哈希
    let mut marked = 0u32;
    let mut online = {
        let game = instance.read().unwrap();
        game.read_online_info()
    };
    let mut entries: Vec<(String, Target)> = by_hash.into_iter().collect();
    for chunk in entries.chunks_mut(HASH_BATCH) {
        if token.is_cancelled() {
            return Err(err_keys::CANCELLED.to_string());
        }
        let hashes: Vec<String> = chunk.iter().map(|(hash, _)| hash.clone()).collect();
        match modrinth_api::get_version_from_sha512(hashes).await {
            Ok(data) => {
                for (hash, version) in data {
                    let Some((_, target)) = chunk.iter().find(|(each, _)| *each == hash) else {
                        continue;
                    };
                    let info =
                        mml_game::modrinth::make_file_online_obj(&version, names::GAME_MODS_DIR);
                    // 旧的按 SHA1 删掉（旧键是旧项目号），新的按项目号写入
                    online.retain(|_, item| item.sha1 != target.sha1);
                    online.insert(version.project_id.clone(), info.clone());
                    model.set_online(&target.sha1, &version.project_id, info);
                    marked += 1;
                }
            }
            Err(err) => {
                // 整批失败只当"这批没认出来"，不中断后面几批
                mml_log::warn(
                    LogText::AutoMarkQueryFailed {
                        error: err.to_string(),
                    }
                    .text(),
                );
            }
        }
    }
    // 没进结果的就是没认出来的（不在 Modrinth 上、或哈希对不上）
    let unknown = total.saturating_sub(marked).saturating_sub(failed);

    // 3) 写盘：整份在线信息覆盖一次即可
    if marked > 0 {
        let game = instance.read().unwrap();
        game.save_online_info(&online);
    }

    Ok(ModAutoMarkDto {
        total,
        marked,
        unknown,
        failed,
    })
}
