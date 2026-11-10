//! 文件比对与下载
//!
//! 判断本地文件是否与清单一致、是否被改名停用，以及下载与解压的底层步骤。

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::Duration,
};

use mml_base::{
    archives::{ArchiveType, BaseArchive},
    file_item::FileItemObj,
    hash_helper::{HashType, gen_hash_from_file},
};
use mml_names::{
    i18_items::error_type::{CoreResult, ErrorType, FileSystemErrorData},
    names,
};
use mml_net::{curseforge_api, modrinth_api};
use mml_sys::path_helper;
use tokio_util::sync::CancellationToken;

use super::{ServerPackSkippedObj, paths, plan};

use crate::{
    gui_hook::AddModPackState,
    launcher::file_online_info_obj::OnlineInfoObj,
    serverpack::serverpack_obj::{ServerArchiveItemObj, ServerItemObj, ServerPackObj},
};

/// 去掉首尾空白，空串按没有处理
///
/// - `value`: 原始值
///
/// # 返回值
///
/// 去空白后的值；空串或 `None` 返回 `None`
pub(super) fn trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|item| !item.is_empty())
}

/// 清单里的相对路径对应的文件名
///
/// - `rel`: 游戏目录下的相对路径，如 `mods/foo.jar`
///
/// # 返回值
///
/// 文件名；路径为空时返回 `None`
pub(super) fn file_name_of(rel: &str) -> Option<String> {
    Path::new(rel)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
}

/// 清单里的相对路径对应的目录
///
/// - `rel`: 游戏目录下的相对路径，如 `mods/foo.jar`
///
/// # 返回值
///
/// 目录部分，如 `mods`；顶层文件返回空串
pub(super) fn dir_of(rel: &str) -> String {
    Path::new(rel)
        .parent()
        .map(|dir| dir.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

/// 在线信息记录指向的本地文件
///
/// - `game_path`: 游戏目录
/// - `info`: 在线信息
///
/// # 返回值
///
/// 记录的绝对路径，不检查是否存在
pub(super) fn local_file(game_path: &Path, info: &OnlineInfoObj) -> PathBuf {
    let dir = info.path.trim();
    if dir.is_empty() {
        game_path.join(&info.file)
    } else {
        game_path.join(dir).join(&info.file)
    }
}

/// 服务器包同步的进度回调：已完成数与总数，单位个
pub type ServerPackProgress<'a> = &'a (dyn Fn(usize, usize) + Sync);

/// 服务器包同步的阶段回调：对比本地文件、开始下载时各调一次
pub type ServerPackPhase<'a> = &'a (dyn Fn(AddModPackState) + Sync);

/// 下载服务器包里的文件
///
/// 整批交给下载器：下载窗口里的**那一个任务**就是这次同步，进度也从同一处取
/// （[`mml_downloader::get_tasks`]），不把一次同步拆成一串小任务。
///
/// - `downloads`: 要下的文件
/// - `cancel`: 取消令牌
/// - `progress`: 进度回调；`Some` 时每 200ms 把任务表里的完成数报一次
///
/// # 返回值
///
/// 全部下完返回 `Ok(())`；被取消返回 `TaskCancel`，有文件失败返回 `DownloadFileFail`
pub(super) async fn download_all(
    downloads: Vec<FileItemObj>,
    cancel: &CancellationToken,
    progress: Option<ServerPackProgress<'_>>,
) -> CoreResult<()> {
    if downloads.is_empty() {
        return Ok(());
    }
    let total = downloads.len();

    let Some(report) = progress else {
        if !mml_downloader::start_download_task(downloads).await {
            return Err(ErrorType::DownloadFileFail);
        }
        return Ok(());
    };

    report(0, total);
    let Some(handle) = mml_downloader::start_download_task_handle(downloads, cancel.clone()) else {
        return Err(ErrorType::DownloadFileFail);
    };
    let done = handle.wait();
    tokio::pin!(done);

    let outcome = loop {
        tokio::select! {
            outcome = &mut done => break outcome,
            _ = tokio::time::sleep(Duration::from_millis(200)) => {
                let status = handle.status();
                report(status.completed, status.total);
            }
        }
    };

    if !outcome.is_done() {
        return Err(if outcome.is_cancelled() {
            ErrorType::TaskCancel
        } else {
            ErrorType::DownloadFileFail
        });
    }
    report(total, total);
    Ok(())
}

/// 本地已有的文件：原始名，或追加了 `.disabled` / `.disable` 后缀的禁用版
///
/// 模组、资源包被禁用后文件名会变，按原始路径判断存在性会把"已经装好但停用"当成没装，
/// 每次同步都覆盖一次，玩家停用的东西就又被装回启用了。
///
/// - `target`: 清单里的原始路径
///
/// # 返回值
///
/// 已存在的路径与是否处于禁用状态；三个都没有时是 `None`
pub(super) fn existing_file(target: &Path) -> Option<(PathBuf, bool)> {
    if target.is_file() {
        return Some((target.to_path_buf(), false));
    }
    // 新文件统一 `.disabled`，旧版留下的 `.disable` 也认
    for suffix in [names::DISABLED_DOT_EXT, names::DISABLE_DOT_EXT] {
        let path = plan::with_suffix(target, suffix);
        if path.is_file() {
            return Some((path, true));
        }
    }
    None
}

/// 文件内容是否与清单一致
///
/// 清单给了 SHA1 就比 SHA1，只给 SHA256 就比 SHA256；没有校验值、文件不存在或读不到时
/// 一律当作需要下载，交给后面的下载流程覆盖。
///
/// - `item`: 清单条目
/// - `path`: 本地文件，可能是带禁用后缀的那一个
///
/// # 返回值
///
/// 文件存在且校验值一致返回 `true`
pub(super) fn matches_hash(item: &ServerItemObj, path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let check = item
        .sha1
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| (HashType::Sha1, value))
        .or_else(|| {
            item.sha256
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| (HashType::Sha256, value))
        });
    let Some((hash_type, expected)) = check else {
        return false;
    };
    matches!(
        gen_hash_from_file(hash_type, path),
        Ok(hash) if hash.eq_ignore_ascii_case(expected)
    )
}

/// 解压基础文件包到实例根目录
///
/// 基础包可以包含 .minecraft，但顶层仍是 game.json、icon.png、server.json 这类
/// 启动器自己的文件时拒绝；.minecraft 里的 server.json / server.json.sha1 /
/// server.json.old 也拒绝，避免覆盖同步元数据。
///
/// - `base`: 实例根目录
/// - `file`: 已经下载到临时目录的基础包
///
/// # 返回值
///
/// 解压成功返回 `Ok(())`；包内有保留文件或解压失败返回对应错误
pub(super) fn extract_base(base: &Path, file: &Path) -> CoreResult<()> {
    let archive = BaseArchive::open_readonly(file)?;
    for item in archive.entries() {
        let name = item.name.replace('\\', "/");
        let parts: Vec<&str> = name.split('/').collect();
        let first = parts.first().copied().unwrap_or_default();
        let blocked = [
            names::GAME_FILE,
            names::ICON_FILE,
            names::SERVER_FILE,
            names::SERVER_INSTANCE_ID_FILE,
        ];
        if blocked
            .iter()
            .any(|reserved| first.eq_ignore_ascii_case(reserved))
            || first.eq_ignore_ascii_case(&format!("{}.sha1", names::SERVER_FILE))
            || first.eq_ignore_ascii_case(&format!("{}.old", names::SERVER_FILE))
            || (first.eq_ignore_ascii_case(names::GAME_DIR)
                && parts.get(1).is_some_and(|second| server_metadata(second)))
        {
            return Err(ErrorType::UnsafePath {
                root: base.to_path_buf(),
                path: base.join(&name),
            });
        }
    }
    drop(archive);

    let archive_type = ArchiveType::try_from_path(file).ok_or_else(|| {
        ErrorType::ArchiveOpenError(FileSystemErrorData {
            path: file.to_path_buf(),
            error: String::new(),
        })
    })?;
    BaseArchive::decompress(archive_type, file, base, None)
}

/// 是否是服务器包清单文件
///
/// - `name`: 文件名
///
/// # 返回值
///
/// server.json、server.json.sha1、server.json.old 返回 `true`
pub(super) fn server_metadata(name: &str) -> bool {
    name.eq_ignore_ascii_case(names::SERVER_FILE)
        || name.eq_ignore_ascii_case(&format!("{}.sha1", names::SERVER_FILE))
        || name.eq_ignore_ascii_case(&format!("{}.old", names::SERVER_FILE))
}

/// `dir` 是否位于 `ancestor` 目录之下
///
/// - `dir`: 相对游戏目录的路径，如 `config/setting`
/// - `ancestor`: 祖先目录，如 `config`
///
/// # 返回值
///
/// 严格位于祖先目录里时返回 `true`；相等或只是名称前缀时返回 `false`
pub(super) fn under(dir: &str, ancestor: &str) -> bool {
    dir.starts_with(ancestor) && dir.as_bytes().get(ancestor.len()) == Some(&b'/')
}

/// 压缩包是否没有任何文件条目
///
/// 目录条目不算内容：空包 + 覆盖解压按"删除目录"处理，不再解出一个空目录。
/// 打不开时按有内容处理，交给正常解压报错，不能把损坏的包当成删除信号。
///
/// - `path`: 压缩包路径
/// - `archive_type`: 压缩包格式
///
/// # 返回值
///
/// 没有文件条目时返回 `true`
pub(super) fn archive_has_no_files(path: &Path, archive_type: ArchiveType) -> bool {
    BaseArchive::open_readonly_as(path, archive_type)
        .map(|archive| archive.entries().iter().all(|item| item.is_dir))
        .unwrap_or(false)
}

/// 压缩包是否与上一次同步一致且目标目录还在
///
/// 同名、同目标目录、同删除设置，并且 SHA1（没有时 SHA256）相同，目标目录仍存在时，
/// 客户端不需要重新下载 / 解压这个配置目录。
///
/// - `previous`: 旧清单里的同名条目，没有时传 `None`
/// - `current`: 新清单条目
/// - `game_path`: 游戏目录
///
/// # 返回值
///
/// 可以跳过下载时返回 `true`
pub(super) fn archive_unchanged(
    previous: Option<&ServerArchiveItemObj>,
    current: &ServerArchiveItemObj,
    game_path: &Path,
) -> bool {
    // 删除条目没有压缩包可省：每次同步都要确认目标已经不在
    if current.action.is_delete() {
        return false;
    }
    let Some(previous) = previous else {
        return false;
    };
    if previous.dir.trim() != current.dir.trim() || previous.delete_old != current.delete_old {
        return false;
    }
    let same_hash = match (
        trimmed(previous.sha1.as_deref()),
        trimmed(current.sha1.as_deref()),
    ) {
        (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
        _ => match (
            trimmed(previous.sha256.as_deref()),
            trimmed(current.sha256.as_deref()),
        ) {
            (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
            _ => false,
        },
    };
    if !same_hash {
        return false;
    }
    // 目录被手动删掉或上次没解出来时要补一次
    paths::file_path(game_path, &current.dir).is_ok_and(|dir| dir.is_dir())
}

/// 这次跳过的条目：记下新清单里的路径、内容与本地留下的旧文件
///
/// - `game_path`: 游戏目录
/// - `old`: 升级前的清单，用来找本地留下的旧版本；没有时传 `None`
/// - `items`: 新清单里的在线文件
/// - `skip`: 这次不更新的路径
///
/// # 返回值
///
/// 要写进 `gui_setting.json` 的跳过记录，顺序与清单一致
pub(super) fn skipped_markers(
    game_path: &Path,
    old: Option<&ServerPackObj>,
    items: &[ServerItemObj],
    skip: &HashSet<&str>,
) -> Vec<ServerPackSkippedObj> {
    items
        .iter()
        .filter(|item| skip.contains(item.file.as_str()))
        .map(|item| {
            // 本地留下的是旧版本那份，文件名可能已经变了：先按旧清单的身份找，
            // 找不到再按新路径找；禁用中的那份带着后缀，按实际文件名记
            let kept = old
                .and_then(|old| {
                    old.online_list
                        .iter()
                        .find(|row| plan::mod_key(row) == plan::mod_key(item))
                })
                .and_then(|row| paths::file_path(game_path, &row.file).ok())
                .and_then(|target| existing_file(&target))
                .or_else(|| {
                    paths::file_path(game_path, &item.file)
                        .ok()
                        .and_then(|target| existing_file(&target))
                })
                .and_then(|(path, _)| relative_to(game_path, &path))
                .unwrap_or_default();
            ServerPackSkippedObj {
                file: item.file.clone(),
                kept,
            }
        })
        .collect()
}

/// 路径相对游戏目录的写法，分隔符统一成 `/`
///
/// - `base`: 游戏目录
/// - `path`: 绝对路径
///
/// # 返回值
///
/// 形如 `mods/foo.jar`；不在游戏目录下时返回 `None`
pub(super) fn relative_to(base: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(base).ok()?;
    Some(rel.to_string_lossy().replace('\\', "/"))
}

/// 删除文件；若文件已被禁用，即追加了 `.disable`/`.disabled` 后缀，则一并删除
///
/// - `file`: 文件路径
pub(super) fn delete_with_disabled<P: AsRef<Path>>(file: P) {
    let file = file.as_ref();
    // `delete` 在文件不存在时是无操作
    let _ = path_helper::delete(file);
    let _ = path_helper::delete(format!("{}{}", file.display(), names::DISABLE_DOT_EXT));
    let _ = path_helper::delete(format!("{}{}", file.display(), names::DISABLED_DOT_EXT));
}

/// 解析空 url 的文件下载地址：通过 pid/fid 从 CurseForge 或 Modrinth 获取
///
/// 按编号格式判断来源：纯数字为 CurseForge，否则为 Modrinth。
/// 优先批量获取，失败或缺失的文件再逐文件降级，返回 `fid → url` 映射。
///
/// - `items`: 缺少下载地址的文件列表
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 返回 `fid → url` 映射
pub(super) async fn resolve_download_urls(
    items: &[&ServerItemObj],
    cancel: &CancellationToken,
) -> HashMap<String, String> {
    let mut urls: HashMap<String, String> = HashMap::new();
    if items.is_empty() {
        return urls;
    }

    // 拆分来源
    let mut curseforge: Vec<&ServerItemObj> = Vec::new();
    let mut modrinth: Vec<&ServerItemObj> = Vec::new();
    for item in items {
        if plan::is_curseforge(item) {
            curseforge.push(item);
        } else {
            modrinth.push(item);
        }
    }

    // ── CurseForge：批量获取文件信息 ──
    let cf_ids: Vec<u64> = curseforge
        .iter()
        .filter_map(|i| i.fid.as_ref().and_then(|f| f.parse().ok()))
        .collect();
    if !cf_ids.is_empty() {
        if let Ok(files) = curseforge_api::get_files(cf_ids).await {
            for mut data in files {
                data.fix_download_url();
                if let Some(url) = data.download_url {
                    urls.insert(data.id.to_string(), url);
                }
            }
        }
    }

    // ── Modrinth：批量获取版本信息 ──
    let mo_ids: Vec<String> = modrinth.iter().filter_map(|i| i.fid.clone()).collect();
    if !mo_ids.is_empty() {
        if let Ok(versions) = modrinth_api::get_versions(mo_ids).await {
            for version in versions {
                if let Some(url) = plan::modrinth_file_url(&version) {
                    urls.insert(version.id, url);
                }
            }
        }
    }

    // 批量失败或缺失的，逐文件降级
    for item in items {
        if cancel.is_cancelled() {
            break;
        }
        let Some(pid) = &item.pid else { continue };
        let Some(fid) = &item.fid else { continue };
        if urls.contains_key(fid) {
            continue;
        }
        if plan::is_curseforge(item) {
            if let Ok(res) = curseforge_api::get_mod(pid, fid).await {
                let mut data = res.data;
                data.fix_download_url();
                if let Some(url) = data.download_url {
                    urls.insert(fid.clone(), url);
                }
            }
        } else if let Ok(version) = modrinth_api::get_version(pid, fid).await {
            if let Some(url) = plan::modrinth_file_url(&version) {
                urls.insert(fid.clone(), url);
            }
        }
    }

    urls
}
