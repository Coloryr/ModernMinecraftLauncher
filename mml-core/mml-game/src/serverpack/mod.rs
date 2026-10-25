//! 服务器整合包管理

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::Duration,
};

use mml_base::{
    archives::{ArchiveType, BaseArchive},
    file_item::{FileItemObj, LaterRun},
    serialize_tools,
};
use mml_config::config_save;
use mml_names::{
    i18_items::error_type::{CoreResult, ErrorType, FileSystemErrorData},
    names, uuids,
};
use mml_net::{curseforge_api, modrinth_api};
use mml_sys::path_helper;
use tokio_util::sync::CancellationToken;

use crate::{
    launcher::instance_setting_obj::InstanceSettingObj,
    serverpack::serverpack_obj::{ServerArchiveItemObj, ServerItemObj, ServerPackObj},
};

pub mod generate;
mod paths;
pub mod plan;
pub mod serverpack_obj;

impl InstanceSettingObj {
    /// 将服务器实例信息标记为旧版
    ///
    /// 第一次同步时本地还没有清单，没有可搬的旧版，直接当成功处理：
    /// 否则新玩家第一次启动必然卡在这里。
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；移动失败返回对应错误
    pub fn move_serverpack_to_old(&self) -> CoreResult<()> {
        let file = self.get_server_pack_file();
        if !file.exists() {
            return Ok(());
        }
        path_helper::move_file(file, self.get_server_pack_old_file())
    }

    /// 读取旧版服务器实例信息
    ///
    /// # 返回值
    ///
    /// 返回旧版服务器包信息；文件不存在返回 `None`，解析失败返回对应错误
    fn get_old_serverpack(&self) -> CoreResult<Option<ServerPackObj>> {
        let file = self.get_server_pack_old_file();
        if !file.exists() || file.is_dir() {
            return Ok(None);
        }
        let obj = serialize_tools::json_from_file::<ServerPackObj>(file)?;

        Ok(Some(obj))
    }

    /// 保存服务器包信息
    ///
    /// - `pack`: 服务器包信息
    pub fn save_serverpack(&self, pack: &ServerPackObj) {
        config_save::save(
            uuids::mix_uuid(self.uuid, uuids::SERVERPACK_FILE_UUID),
            pack,
            self.get_server_pack_file(),
        );
    }

    /// 执行升级操作
    ///
    /// - `new_pack`: 新版服务器包信息
    /// - `cancel`: 取消令牌
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；下载 / 解压失败或被取消返回对应错误
    pub async fn upgrade_serverpack(
        &self,
        new_pack: ServerPackObj,
        cancel: CancellationToken,
    ) -> CoreResult<()> {
        self.upgrade_serverpack_with(new_pack, cancel, None).await
    }

    /// 执行升级操作，并把下载进度报给调用方
    ///
    /// 与 [`upgrade_serverpack`](Self::upgrade_serverpack) 同一条链路，多一个进度回调：
    /// 导入在线实例时要用它把"下了几个文件"推给界面，否则进度条会一直停在 0。
    ///
    /// - `new_pack`: 新版服务器包信息
    /// - `cancel`: 取消令牌
    /// - `progress`: 进度回调，参数是已完成数与总数，单位个；`None` 表示不关心
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；下载 / 解压失败或被取消返回对应错误
    pub async fn upgrade_serverpack_with(
        &self,
        new_pack: ServerPackObj,
        cancel: CancellationToken,
        progress: Option<ServerPackProgress<'_>>,
    ) -> CoreResult<()> {
        let old = self.get_old_serverpack()?;

        let game_path = self.get_game_path();
        let temp_path = self.get_temp_path();
        paths::validate_pack(&new_pack, &game_path, &temp_path)?;
        if let Some(old) = &old {
            paths::validate_pack(old, &game_path, &temp_path)?;
        }

        if let Some(old) = &old {
            // 删什么由纯函数算，见 `serverpack::plan`，这里只负责执行
            let plan = plan::plan_removals(old, &new_pack);

            // 删除已移除或已更换路径的文件，`file` 是游戏目录下的相对路径
            for item in &plan.files {
                if cancel.is_cancelled() {
                    return Err(ErrorType::TaskCancel);
                }
                delete_with_disabled(paths::file_path(&game_path, &item.file)?);
            }

            // 删除已移除的配置文件，仅删除由该配置独享的目录
            for item in &plan.dirs {
                if cancel.is_cancelled() {
                    return Err(ErrorType::TaskCancel);
                }
                path_helper::move_to_trash(paths::file_path(&game_path, &item.dir)?)?;
            }
        }

        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        // 在线文件的 url 为空时从 pid/fid 解析；配置文件暂存到实例临时目录，下载完成后解压
        let need_resolve: Vec<&ServerItemObj> = new_pack
            .online_list
            .iter()
            .filter(|i| i.url.as_deref().map_or(true, |u| u.is_empty()))
            .collect();
        let url_map = resolve_download_urls(&need_resolve, &cancel).await;

        // 下什么、从哪下、按什么哈希校验：同样由纯函数算
        let plan = plan::plan_downloads(&new_pack, &url_map);

        let mut downloads: Vec<FileItemObj> = Vec::new();
        let mut archives: Vec<(&ServerArchiveItemObj, PathBuf)> = Vec::new();

        for planned in &plan.files {
            downloads.push(FileItemObj {
                url: planned.url.clone(),
                name: planned.item.file.clone(),
                file: paths::file_path(&game_path, &planned.item.file)?,
                hash: plan::make_hash(&planned.item.sha1, &planned.item.sha256),
                later: LaterRun::None,
            });
        }

        for planned in &plan.archives {
            let temp = paths::file_path(&temp_path, &planned.item.file)?;
            downloads.push(FileItemObj {
                url: planned.url.clone(),
                name: planned.item.file.clone(),
                file: temp.clone(),
                hash: plan::make_hash(&planned.item.sha1, &planned.item.sha256),
                later: LaterRun::None,
            });
            archives.push((planned.item, temp));
        }

        download_all(downloads, &cancel, progress).await?;

        for (item, temp) in archives.iter() {
            if cancel.is_cancelled() {
                return Err(ErrorType::TaskCancel);
            }

            let output = path_helper::safe_join(&game_path, &item.dir)?;
            // 覆盖解压时先删除旧目录
            if item.delete_old && output.exists() {
                path_helper::move_to_trash(&output)?;
            }

            let archive_type = ArchiveType::try_from_path(temp).ok_or_else(|| {
                ErrorType::ArchiveOpenError(FileSystemErrorData {
                    path: temp.clone(),
                    error: String::new(),
                })
            })?;
            BaseArchive::decompress(archive_type, temp, &output, None)?;

            // 清理临时压缩包
            path_helper::delete(temp)?;
        }

        // 保存新版本信息，清理旧文件
        self.save_serverpack(&new_pack);
        path_helper::delete(self.get_server_pack_old_file())?;

        Ok(())
    }
}

/// 服务器包同步的进度回调：已完成数与总数，单位个
pub type ServerPackProgress<'a> = &'a (dyn Fn(usize, usize) + Sync);

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
async fn download_all(
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

/// 删除文件；若文件已被禁用，即追加了 `.disable`/`.disabled` 后缀，则一并删除
///
/// - `file`: 文件路径
fn delete_with_disabled<P: AsRef<Path>>(file: P) {
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
async fn resolve_download_urls(
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
