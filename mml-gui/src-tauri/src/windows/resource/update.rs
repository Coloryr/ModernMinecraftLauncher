//! 资源管理窗口的模组更新检查：查有在线信息的模组在平台上有没有新文件
//!
//! 只认实例 `online_info.json` 里记着的模组，按 SHA1 关联，项目号与文件号都非空才算；
//! 手动放进 mods 的包不在表里，无从判断有没有更新，直接跳过。
//!
//! 判定口径：按实例的游戏版本与加载器过滤，取平台上**最新**的那个文件，
//! 文件号与装着的不一样就算有更新。平台在当前版本 / 加载器下一个文件都没有时不报 ——
//! 那说明这个包在本实例上没有可用的新版本，不是漏了。
//!
//! 检查要重扫一遍 mods 目录（在线信息按 SHA1 关联），代价与打开模组分类相同；
//! 之后逐个问平台，进度经 `resource-mods-update-progress` 事件上报。

use std::collections::HashMap;
use std::sync::Arc;

use std::path::Path;

use mml_game::GameInstance;
use mml_game::game_mods::{ModObj, add_disable_suffix};
use mml_game::launcher::file_online_info_obj::OnlineInfoObj;
use mml_game::launcher::{ModPackType, get_source_type};
use mml_game::loader::LoaderType;
use mml_game::{curseforge, modrinth};
use mml_names::names;
use mml_net::curseforge_api::{self, CurseFogreArg, file_obj::CurseForgeFileDataObj};
use mml_net::modrinth_api::{self, version_obj::ModrinthVersionObj};
use tauri::{Emitter, WebviewWindow};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::dtos::{
    ModUpdateApplyDto, ModUpdateCheckDto, ModUpdateDoneDto, ModUpdateDto, ModUpdateProgressDto,
};
use crate::err_keys;
use crate::listens;

use super::mods::{find_mod, sha1_of};
use super::{block_on_instance, parse_instance};

/// 同时在查的模组数上限
///
/// CurseForge 不限速，串行查几百个要一分多钟；Modrinth 的每分钟配额在客户端内部排队，
/// 并发开多大都压在同一个限速器上，所以这个值只影响 CurseForge 那一半。
const MAX_CONCURRENT: usize = 8;

/// 更新进度事件，事件名 `resource-mods-update-progress`
///
/// - `window`: 发起更新的那个资源窗口，由 Tauri 注入，进度只发给它
/// - `dto`: 进度载荷，已完成与总数
///
/// 检查与下载两个阶段共用：检查逐个问平台可能要一分钟（Modrinth 限 300 次 / 分钟），
/// 下载也要时间，没有进度用户都会以为卡死。
#[gui_macros::emit]
pub fn emit_resource_mods_update_progress(window: &WebviewWindow, dto: ModUpdateProgressDto) {
    let _ = window.emit(listens::RESOURCE_MODS_UPDATE_PROGRESS, dto);
}

/// 更新下载完成事件，事件名 `resource-mods-update-done`
///
/// - `window`: 发起更新的那个资源窗口，由 Tauri 注入
/// - `dto`: 成功与失败的个数
///
/// 下载走全局下载器，进度在「下载」窗口里看得见；这个事件只用来告诉资源窗口
/// "文件已经换过了，可以重拉列表" —— 不重拉的话界面上还是旧文件名、旧版本号。
#[gui_macros::emit]
pub fn emit_resource_mods_update_done(window: &WebviewWindow, dto: ModUpdateDoneDto) {
    let _ = window.emit(listens::RESOURCE_MODS_UPDATE_DONE, dto);
}

/// 一个待检查的模组，即"有在线信息的顶层条目"
struct Target {
    /// 当前条目 uuid，文件路径的 v5
    uuid: String,
    /// 当前文件名，禁用状态下带后缀
    file: String,
    /// 是否被禁用
    disable: bool,
    /// 显示名，元数据缺失时为空
    name: String,
    /// 当前版本号，没写时为空串
    version: String,
    /// 下载源
    source: ModPackType,
    /// 项目编号
    project_id: String,
    /// 当前文件编号
    file_id: String,
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
fn collect_targets(list: &[ModObj], online: &HashMap<String, OnlineInfoObj>) -> Vec<Target> {
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
#[tauri::command]
pub async fn resource_mods_update_check(
    window: WebviewWindow,
    uuid: String,
) -> Result<ModUpdateCheckDto, String> {
    let model = super::model::window_model(&uuid)?;
    let Some((targets, version, loader)) = model.with_cache(|cache| {
        (
            collect_targets(&cache.list, &cache.online),
            cache.version.clone(),
            cache.loader,
        )
    }) else {
        // 模组列表还没加载过：没有"现有数据"可查，别去扫一遍凑数
        return Err(err_keys::MODS_NOT_LOADED.to_string());
    };

    // 关窗时要能收手：任务挂在这个窗口的模型上，见 super::model
    let op = model.begin();
    let token = op.token();

    let total = targets.len();
    // 先把总数报出去：前端据此显示 `0/N`，否则第一次进度回来前只有一个转圈
    emit_resource_mods_update_progress(&window, ModUpdateProgressDto { done: 0, total });

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
                emit_resource_mods_update_progress(&window, ModUpdateProgressDto { done, total });
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
        mml_log::error(format!("模组更新检查：{failed} 个查询失败，首个：{err}"));
    }

    Ok(ModUpdateCheckDto {
        checked: total as u32,
        failed: failed as u32,
        updates,
    })
}

/// 下载更新：把选中的模组换成平台上的新文件
///
/// 逐条处理：问平台拿新文件的下载地址与哈希 → 下载校验 → 删掉旧文件（进回收站）→
/// 写回在线信息。**旧文件在下载成功之后才删**：下载失败时原文件原样留着，
/// 免得更新一下反而没得用；原本禁用着的模组更新完仍是禁用状态，不会顺手被启用。
///
/// 下载走全局下载器，进度在「下载」窗口可见；本命令校验完参数就返回，
/// 全部处理完再发完成事件，前端据此重拉列表。
///
/// - `window`: 发起更新的那个资源窗口，进度与完成事件只发给它
/// - `uuid`: 实例 uuid
/// - `items`: 勾选要更新的条目；空数组什么都不做
///
/// # 返回值
///
/// uuid 非法或实例不存在返回错误串；其余情况立即返回 `Ok(())`，
/// 单条失败只计数、不让一条坏掉整批，结果在完成事件里
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_mods_update_apply(
    window: WebviewWindow,
    uuid: String,
    items: Vec<ModUpdateApplyDto>,
) -> Result<(), String> {
    if items.is_empty() {
        return Ok(());
    }
    let instance = parse_instance(&uuid)?;
    // 关窗时要能收手：任务挂在这个窗口的模型上
    let op = super::model::window_model(&uuid)?.begin();
    let token = op.token();
    let total = items.len();
    emit_resource_mods_update_progress(&window, ModUpdateProgressDto { done: 0, total });

    tauri::async_runtime::spawn(async move {
        // 守卫跟着任务走：任务结束（或被丢掉）它才把令牌从模型里摘掉
        let _op = op;
        let mut ok = 0u32;
        let mut failed = 0u32;
        for (index, item) in items.into_iter().enumerate() {
            // 窗口关了：剩下的不再下，已经下完的保持原样
            if token.is_cancelled() {
                return;
            }
            match apply_one(instance.clone(), &item).await {
                Ok(()) => ok += 1,
                Err(err) => {
                    failed += 1;
                    // 失败原因逐条记：用户报"更新不动"时这是唯一的线索
                    mml_log::error(format!("模组更新失败 {}：{err}", item.project_id));
                }
            }
            emit_resource_mods_update_progress(
                &window,
                ModUpdateProgressDto {
                    done: index + 1,
                    total,
                },
            );
        }
        emit_resource_mods_update_done(&window, ModUpdateDoneDto { ok, failed });
    });

    Ok(())
}

/// 换掉一个模组：下新文件、删旧文件、更新在线信息
///
/// - `instance`: 实例句柄
/// - `item`: 要更新的那一条
///
/// # 返回值
///
/// 旧文件找不到、平台查不到新文件、下载校验失败或写盘失败返回错误串
async fn apply_one(instance: GameInstance, item: &ModUpdateApplyDto) -> Result<(), String> {
    let source = ModPackType::from_string(&item.source);
    if !matches!(source, ModPackType::CurseForge | ModPackType::Modrinth) {
        return Err(err_keys::SOURCE_TYPE.to_string());
    }

    // 旧文件按 uuid 定位；顺带记下它是不是禁用着，更新完要接着禁用
    let old = find_mod(instance.clone(), &item.uuid).await?;
    let dir = block_on_instance(instance.clone(), |game| game.get_mods_path()).await?;

    let (obj, info) = match source {
        ModPackType::CurseForge => {
            let fid: u64 = item
                .new_file_id
                .parse()
                .map_err(|_| err_keys::BAD_FILE_ID.to_string())?;
            let mut list = curseforge_api::get_files(vec![fid])
                .await
                .map_err(|err| err.to_string())?;
            let mut data = list
                .pop()
                .ok_or_else(|| err_keys::FILE_NOT_FOUND.to_string())?;
            let obj = curseforge::make_file_item_obj(&mut data, &dir);
            let info = curseforge::make_file_online_info_obj(&mut data, names::GAME_MODS_DIR);
            (obj, info)
        }
        ModPackType::Modrinth => {
            let data = modrinth_api::get_version(&item.project_id, &item.new_file_id)
                .await
                .map_err(|err| err.to_string())?;
            // Modrinth 偶尔给一个 files 都不带的版本；下面两个构造函数都会直接取 files[0]
            if data.files.is_empty() {
                return Err(err_keys::FILE_NOT_FOUND.to_string());
            }
            let obj = modrinth::make_download_obj(&data, &dir);
            let info = modrinth::make_file_online_obj(&data, names::GAME_MODS_DIR);
            (obj, info)
        }
        _ => return Err(err_keys::SOURCE_TYPE.to_string()),
    };

    // 下载与哈希校验；没成功就到此为止，旧文件一个字都不动
    let outcome = mml_downloader::start_download_task_outcome(vec![obj.clone()]).await;
    if !outcome.is_done() {
        return Err(err_keys::MOD_UPDATE_FAILED.to_string());
    }

    if let Some(stale) = stale_file(&old.file, &obj.file) {
        old_delete(stale)?;
    }
    // 原本禁用着：给新文件补上后缀，别让更新把用户禁用的模组启用了
    if old.disable {
        add_disable_suffix(&obj.file).map_err(|err| err.to_string())?;
    }

    let game = instance.read().unwrap();
    let mut online = game.read_online_info();
    online.insert(item.project_id.clone(), info);
    game.save_online_info(&online);
    Ok(())
}

/// 更新完之后要删掉的旧文件，新旧同名时返回 `None`
///
/// 平台换了内容却没改名是有的：这时下载已经覆盖了同一个路径，
/// 再删就是把刚下好的那份删掉。
///
/// - `old`: 旧文件路径
/// - `new`: 新文件路径
///
/// # 返回值
///
/// 要删的旧文件；新旧同名时没有
fn stale_file<'a>(old: &'a Path, new: &Path) -> Option<&'a Path> {
    (old != new).then_some(old)
}

/// 把旧文件移进回收站
///
/// - `path`: 旧文件路径
///
/// # 返回值
///
/// 删除失败返回错误串
fn old_delete(path: &Path) -> Result<(), String> {
    ModObj {
        file: path.to_path_buf(),
        ..Default::default()
    }
    .delete()
    .map_err(|err| err.to_string())
}

/// 查一个模组，判断装上的是不是最新
///
/// - `target`: 待检查的模组
/// - `version`: 实例的游戏版本
/// - `loader`: 实例的加载器
///
/// # 返回值
///
/// 有更新时是那条新文件；已是最新，或平台在当前版本 / 加载器下没有文件时返回 `Ok(None)`；
/// 请求失败返回 `Err`
async fn check_one(
    target: &Target,
    version: &str,
    loader: LoaderType,
) -> Result<Option<ModUpdateDto>, String> {
    let latest = match target.source {
        ModPackType::CurseForge => latest_curseforge(&target.project_id, version, loader).await?,
        ModPackType::Modrinth => latest_modrinth(&target.project_id, version, loader).await?,
        // 其余来源没有查询接口，也不该带着在线信息出现在这里
        _ => return Ok(None),
    };
    let Some(latest) = latest else {
        return Ok(None);
    };
    // 文件号一样就是同一个文件；平台已按版本与加载器过滤，剩下的都是本实例能装的
    if latest.id == target.file_id {
        return Ok(None);
    }
    Ok(Some(ModUpdateDto {
        uuid: target.uuid.clone(),
        file: target.file.clone(),
        disable: target.disable,
        name: target.name.clone(),
        version: target.version.clone(),
        source: target.source.to_string(),
        project_id: target.project_id.clone(),
        file_id: target.file_id.clone(),
        new_file_id: latest.id,
        new_version: latest.version,
        new_file: latest.file,
        date: latest.date,
    }))
}

/// 平台上的一个候选新文件
struct LatestFile {
    /// 文件编号
    id: String,
    /// 版本号；CurseForge 没有这个字段，取文件的显示名
    version: String,
    /// 文件名
    file: String,
    /// 发布时间，ISO 8601
    date: String,
}

/// 取 CurseForge 上最新的那个文件
///
/// - `pid`: 项目编号
/// - `version`: 游戏版本
/// - `loader`: 加载器
///
/// # 返回值
///
/// 见 [`LatestFile`]；当前版本 / 加载器下一个文件都没有时返回 `Ok(None)`
async fn latest_curseforge(
    pid: &str,
    version: &str,
    loader: LoaderType,
) -> Result<Option<LatestFile>, String> {
    let list = curseforge_api::get_files_page(CurseFogreArg {
        id: Some(pid.to_string()),
        version: Some(version.to_string()),
        loader: curseforge::to_loader_id(&loader),
        ..Default::default()
    })
    .await
    .map_err(|err| err.to_string())?;

    Ok(newest_curseforge(list.data))
}

/// 在 CurseForge 的文件列表里挑发布时间最晚的那个
///
/// 不直接取第一个：接口没在文档里承诺排序，按 `fileDate` 挑才不赌它。
///
/// - `list`: 接口返回的那一页文件
///
/// # 返回值
///
/// 见 [`LatestFile`]；空列表返回 `None`
fn newest_curseforge(list: Vec<CurseForgeFileDataObj>) -> Option<LatestFile> {
    list.into_iter()
        .max_by(|a, b| a.file_date.cmp(&b.file_date))
        .map(|item| LatestFile {
            id: item.id.to_string(),
            version: item.display_name,
            file: item.file_name,
            date: item.file_date,
        })
}

/// 取 Modrinth 上最新的那个版本
///
/// - `pid`: 项目编号
/// - `version`: 游戏版本
/// - `loader`: 加载器
///
/// # 返回值
///
/// 见 [`LatestFile`]；当前版本 / 加载器下一个版本都没有时返回 `Ok(None)`
async fn latest_modrinth(
    pid: &str,
    version: &str,
    loader: LoaderType,
) -> Result<Option<LatestFile>, String> {
    let loader = modrinth::to_loader_id(&loader);
    let list = modrinth_api::get_file_versions(pid, Some(version), loader.as_deref())
        .await
        .map_err(|err| err.to_string())?;

    Ok(newest_modrinth(list))
}

/// 在 Modrinth 的版本列表里挑发布时间最晚的那个
///
/// - `list`: 接口返回的版本列表
///
/// # 返回值
///
/// 见 [`LatestFile`]；空列表返回 `None`，版本里一个文件都没带时文件名是空串
fn newest_modrinth(list: Vec<ModrinthVersionObj>) -> Option<LatestFile> {
    list.into_iter()
        .max_by(|a, b| a.date_published.cmp(&b.date_published))
        .map(|item| {
            // 取 primary 那一份；Modrinth 偶尔给一个主文件都不带的版本，退回第一个
            let file = item
                .files
                .iter()
                .find(|file| file.primary)
                .or_else(|| item.files.first())
                .map(|file| file.filename.clone())
                .unwrap_or_default();
            LatestFile {
                id: item.id,
                version: item.version_number,
                file,
                date: item.date_published,
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mml_base::file_item::FileHash;
    use mml_game::game_mods::ModItemObj;
    use mml_net::modrinth_api::version_obj::ModrinthVersionFileObj;
    use std::path::PathBuf;

    /// 造一个顶层模组条目，只填判定用得到的字段
    fn mod_item(sha1: &str, name: &str, version: &str) -> ModObj {
        ModObj {
            hash: FileHash::Sha1(sha1.to_string()),
            file: PathBuf::from(format!("{name}.jar")),
            info: vec![ModItemObj {
                name: name.to_string(),
                version: Some(version.to_string()),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    /// 造一条在线信息
    fn online(sha1: &str, modid: &str, fileid: &str) -> OnlineInfoObj {
        OnlineInfoObj {
            sha1: sha1.to_string(),
            modid: modid.to_string(),
            fileid: fileid.to_string(),
            ..Default::default()
        }
    }

    /// 造一个 CurseForge 文件
    fn cf_file(id: u64, date: &str) -> CurseForgeFileDataObj {
        CurseForgeFileDataObj {
            id,
            display_name: format!("file {id}"),
            file_name: format!("file-{id}.jar"),
            file_date: date.to_string(),
            ..Default::default()
        }
    }

    /// 造一个 Modrinth 版本，文件列表按 `(文件名, 是否 primary)` 给
    fn mr_version(id: &str, date: &str, files: &[(&str, bool)]) -> ModrinthVersionObj {
        ModrinthVersionObj {
            id: id.to_string(),
            version_number: format!("v-{id}"),
            date_published: date.to_string(),
            files: files
                .iter()
                .map(|(name, primary)| ModrinthVersionFileObj {
                    filename: name.to_string(),
                    primary: *primary,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }

    /// 只收有在线信息的模组：没在表里的、项目号或文件号缺一项的都要跳过
    #[test]
    fn collect_targets_keeps_online_only() {
        let list = vec![
            mod_item("aa", "sodium", "0.5.0"),
            mod_item("bb", "manual", "1.0"),
            mod_item("cc", "half", "1.0"),
        ];
        let mut table: HashMap<String, OnlineInfoObj> = HashMap::new();
        table.insert("100".into(), online("aa", "100", "200"));
        // bb 是手动放进去的，表里没有它的 SHA1
        // cc 只有项目号、没有文件号：认不出装的是哪个文件
        table.insert("101".into(), online("cc", "101", ""));

        let targets = collect_targets(&list, &table);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].file, "sodium.jar");
        assert_eq!(targets[0].name, "sodium");
        assert_eq!(targets[0].version, "0.5.0");
        assert_eq!(targets[0].project_id, "100");
        assert_eq!(targets[0].file_id, "200");
        // 项目号与文件号都是纯数字 → CurseForge
        assert_eq!(targets[0].source, ModPackType::CurseForge);
    }

    /// 没算出 SHA1 的文件不参与检查
    #[test]
    fn collect_targets_skips_without_hash() {
        let mut item = mod_item("dd", "nohash", "1.0");
        item.hash = FileHash::None;
        let mut table: HashMap<String, OnlineInfoObj> = HashMap::new();
        table.insert("102".into(), online("", "102", "103"));

        assert!(collect_targets(&[item], &table).is_empty());
    }

    /// CurseForge：挑 `fileDate` 最晚的那个，不按列表顺序
    #[test]
    fn newest_curseforge_picks_latest_date() {
        let list = vec![
            cf_file(1, "2024-01-01T00:00:00Z"),
            cf_file(3, "2024-03-01T00:00:00Z"),
            cf_file(2, "2024-02-01T00:00:00Z"),
        ];

        let latest = newest_curseforge(list).unwrap();
        assert_eq!(latest.id, "3");
        assert_eq!(latest.file, "file-3.jar");
        assert_eq!(latest.version, "file 3");
        assert_eq!(latest.date, "2024-03-01T00:00:00Z");
    }

    /// CurseForge：空列表没有候选
    #[test]
    fn newest_curseforge_empty() {
        assert!(newest_curseforge(Vec::new()).is_none());
    }

    /// Modrinth：挑 `date_published` 最晚的那个，文件名取 primary 那一份
    #[test]
    fn newest_modrinth_picks_latest_date() {
        let list = vec![
            mr_version("a", "2024-01-01T00:00:00Z", &[("old.jar", true)]),
            mr_version(
                "c",
                "2024-03-01T00:00:00Z",
                &[("first.jar", false), ("new.jar", true)],
            ),
            mr_version("b", "2024-02-01T00:00:00Z", &[("mid.jar", true)]),
        ];

        let latest = newest_modrinth(list).unwrap();
        assert_eq!(latest.id, "c");
        assert_eq!(latest.version, "v-c");
        assert_eq!(latest.file, "new.jar");
        assert_eq!(latest.date, "2024-03-01T00:00:00Z");
    }

    /// Modrinth：没有 primary 时退回第一个文件，空列表没有候选
    #[test]
    fn newest_modrinth_without_primary() {
        let list = vec![mr_version("a", "2024-01-01T00:00:00Z", &[("only.jar", false)])];
        assert_eq!(newest_modrinth(list).unwrap().file, "only.jar");

        assert!(newest_modrinth(Vec::new()).is_none());
    }

    /// 新旧文件同名时不删旧文件：下载已经覆盖了同一个路径
    #[test]
    fn stale_file_skips_same_path() {
        let same = Path::new("mods/sodium.jar");
        assert!(stale_file(same, same).is_none());

        let old = Path::new("mods/sodium-0.5.jar");
        let new = Path::new("mods/sodium-0.6.jar");
        assert_eq!(stale_file(old, new), Some(old));
    }
}
