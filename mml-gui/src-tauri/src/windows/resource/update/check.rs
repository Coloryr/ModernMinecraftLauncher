//! 更新检查：收集候选模组并逐个问平台
//!
//! 只认实例在线信息里记着的模组，按 SHA1 关联，项目号与文件号都非空才算。

use std::collections::HashMap;
use std::sync::Arc;

use mml_game::game_mods::ModObj;
use mml_game::launcher::file_online_info_obj::OnlineInfoObj;
use mml_game::launcher::{ModPackType, get_source_type};
use tauri::WebviewWindow;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::dtos::{ModUpdateCheckDto, ModUpdateProgressDto};
use crate::err_keys;
use crate::log_text::LogText;

use super::emit::{STAGE_CHECK, emit_resource_mods_update_progress};
use super::query::check_one;
use super::run::MAX_CONCURRENT;
use crate::windows::resource::mods::{mod_note_key, sha1_of};
use crate::windows::resource::parse_instance;

/// 一个待检查的模组，即"有在线信息的顶层条目"
pub(super) struct Target {
    /// 当前条目 uuid，文件路径的 v5
    pub(super) uuid: String,
    /// 当前文件名，禁用状态下带后缀
    pub(super) file: String,
    /// 是否被禁用
    pub(super) disable: bool,
    /// 显示名，元数据缺失时为空
    pub(super) name: String,
    /// 当前版本号，没写时为空串
    pub(super) version: String,
    /// 下载源
    pub(super) source: ModPackType,
    /// 项目编号
    pub(super) project_id: String,
    /// 当前文件编号
    pub(super) file_id: String,
}

/// 挑出有在线信息、可以检查更新的模组
///
/// 只收顶层条目：内置 jar-in-jar 没有独立文件，也没有自己的在线信息。
///
/// - `list`: 模组扫描结果
/// - `online`: 实例的在线信息表，键是游戏路径，值里的 SHA1 用来和扫描结果对上
///
/// # 返回值
///
/// 待检查的模组，按扫描顺序；项目号或文件号缺一项的都跳过
pub(super) fn collect_targets(
    list: &[ModObj],
    online: &HashMap<String, OnlineInfoObj>,
) -> Vec<Target> {
    // 在线信息表按项目号 / 路径索引，这里要按 SHA1 反查，先建一份索引
    let online_of: HashMap<&str, &OnlineInfoObj> = online
        .values()
        .filter(|info| !info.sha1.is_empty())
        .map(|info| (info.sha1.as_str(), info))
        .collect();

    list.iter()
        .filter_map(|item| {
            let info = online_of.get(sha1_of(&item.hash).as_str())?;
            if info.modid.is_empty() || info.fileid.is_empty() {
                return None;
            }
            let meta = item.info.first();
            Some(Target {
                uuid: item.uuid.to_string(),
                file: item
                    .file
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default(),
                disable: item.disable,
                name: meta.map(|item| item.name.clone()).unwrap_or_default(),
                version: meta
                    .and_then(|item| item.version.clone())
                    .unwrap_or_default(),
                source: get_source_type(&info.modid, &info.fileid),
                project_id: info.modid.clone(),
                file_id: info.fileid.clone(),
            })
        })
        .collect()
}

/// 模组更新检查：查有在线信息的模组在平台上有没有新文件
///
/// **不重扫 mods 目录**：用的是列表命令留在窗口模型里的那份扫描结果，
/// 检查只是拿它逐个问平台，点下去立刻就有总数。
///
/// - `window`: 发起检查的那个资源窗口，由 Tauri 注入，进度只发给它
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 见 [`ModUpdateCheckDto`]；实例不存在或模组列表还没加载时返回错误串
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub async fn resource_mods_update_check(
    window: WebviewWindow,
    uuid: String,
) -> Result<ModUpdateCheckDto, String> {
    let model = crate::windows::resource::model::window_model(&uuid)?;
    // 手动指定的下载源在实例的图形设置里，先读出来；没指定的按项目号 / 文件号形态判定
    let sources = {
        let instance = parse_instance(&uuid)?;
        let game = instance.read().unwrap();
        crate::gui_setting::load(&game).mods.mod_source
    };
    let Some((targets, version, loader)) = model.with_cache(|cache| {
        let mut targets = collect_targets(&cache.list, &cache.online);
        for target in targets.iter_mut() {
            if let Some(source) = sources
                .get(mod_note_key(&target.file))
                .filter(|s| !s.is_empty())
            {
                target.source = ModPackType::from_string(source);
            }
        }
        (targets, cache.version.clone(), cache.loader)
    }) else {
        // 模组列表还没加载过：没有"现有数据"可查，别去扫一遍凑数
        return Err(err_keys::MODS_NOT_LOADED.to_string());
    };

    // 关窗时要能收手：任务挂在这个窗口的模型上，见 crate::windows::resource::model
    let op = model.begin();
    let token = op.token();

    let total = targets.len();
    // 先把总数报出去：前端据此显示 `0/N`，否则第一次进度回来前只有一个转圈
    emit_resource_mods_update_progress(
        &window,
        ModUpdateProgressDto {
            stage: STAGE_CHECK.to_string(),
            done: 0,
            total,
        },
    );

    let mut updates = Vec::new();
    let mut failed = 0usize;
    // 只留第一条错误：整片失败多半是网络 / 接口的问题，逐条记会把日志刷满
    let mut first_error: Option<String> = None;

    // 配额等待在客户端内部串行，这里只限制同时压在 CurseForge 上的请求数
    let sem = Arc::new(Semaphore::new(MAX_CONCURRENT));
    let mut set = JoinSet::new();
    for target in targets {
        let sem = sem.clone();
        let version = version.clone();
        set.spawn(async move {
            let _permit = sem.acquire().await.ok();
            check_one(&target, &version, loader).await
        });
    }

    let mut done = 0usize;
    loop {
        tokio::select! {
            // 窗口关了：剩下的不再等；丢下 JoinSet 会把没跑完的请求一起 abort
            _ = token.cancelled() => return Err(err_keys::CANCELLED.to_string()),
            joined = set.join_next() => {
                let Some(joined) = joined else { break };
                done += 1;
                emit_resource_mods_update_progress(
                    &window,
                    ModUpdateProgressDto {
                        stage: STAGE_CHECK.to_string(),
                        done,
                        total,
                    },
                );
                match joined {
                    Ok(Ok(Some(update))) => updates.push(update),
                    Ok(Ok(None)) => {}
                    // 单个模组查失败不影响其它：记个数，整份结果照常返回
                    Ok(Err(err)) => {
                        failed += 1;
                        first_error.get_or_insert(err);
                    }
                    Err(err) => {
                        failed += 1;
                        first_error.get_or_insert(err.to_string());
                    }
                }
            }
        }
    }

    if let Some(err) = first_error {
        mml_log::error(LogText::ModUpdateCheckFailed { failed, error: err }.text());
    }

    Ok(ModUpdateCheckDto {
        checked: total as u32,
        failed: failed as u32,
        updates,
    })
}
