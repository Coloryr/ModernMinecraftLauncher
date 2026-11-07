//! 服务器整合包管理

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::Duration,
};

use mml_base::{
    archives::{ArchiveType, BaseArchive},
    file_item::{FileItemObj, LaterRun},
    hash_helper::{HashType, gen_hash_from_file},
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
    gui_hook::AddModPackState,
    launcher::{
        ModPackType, file_online_info_obj::OnlineInfoObj, instance_setting_obj::InstanceSettingObj,
    },
    serverpack::serverpack_obj::{
        ServerArchiveItemObj, ServerBaseObj, ServerItemObj, ServerPackObj, ServerSkippedObj,
    },
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

    /// 跳过同步时把旧清单挪回原位
    ///
    /// 同步开始前 `server.json` 已经被挪成 `.old`，中途取消时本地就没有清单了；
    /// 挪回来界面上照样能读到模组与资源包信息，下次启动也会重新比对。
    ///
    /// # 返回值
    ///
    /// 移动成功返回 `Ok(())`；移动失败返回对应错误
    pub fn restore_serverpack_from_old(&self) -> CoreResult<()> {
        let file = self.get_server_pack_file();
        let old = self.get_server_pack_old_file();
        if file.exists() || !old.exists() {
            return Ok(());
        }
        path_helper::move_file(old, file)
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

    /// 同步保存服务器包信息
    ///
    /// 导入流程紧接着要读清单时用，异步队列可能还没落盘。
    ///
    /// - `pack`: 服务器包信息
    ///
    /// # 返回值
    ///
    /// 写盘失败时返回对应错误
    pub fn save_serverpack_sync(&self, pack: &ServerPackObj) -> CoreResult<()> {
        serialize_tools::json_to_file(pack, self.get_server_pack_file())
    }

    /// 本地清单的包版本是否与给定版本相同
    ///
    /// - `version`: 远端清单的包版本
    ///
    /// # 返回值
    ///
    /// 本地有清单、版本非空且与参数相同返回 `true`
    pub fn same_pack_version(&self, version: &str) -> bool {
        let version = version.trim();
        if version.is_empty() {
            return false;
        }
        serialize_tools::json_from_file::<ServerPackObj>(self.get_server_pack_file())
            .ok()
            .is_some_and(|old| old.pack_version.trim() == version)
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
        self.upgrade_serverpack_with(new_pack, cancel, None, None, &[])
            .await
    }

    /// 执行升级操作，并把下载进度报给调用方
    ///
    /// 与 [`upgrade_serverpack`](Self::upgrade_serverpack) 同一条链路，多一个进度回调：
    /// 导入在线实例时要用它把"下了几个文件"推给界面，否则进度条会一直停在 0。
    ///
    /// - `new_pack`: 新版服务器包信息
    /// - `cancel`: 取消令牌
    /// - `progress`: 进度回调，参数是已完成数与总数，单位个；`None` 表示不关心
    /// - `skip`: 这次先不更新的文件，取新清单里的相对路径；空表示全部更新
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；下载 / 解压失败或被取消返回对应错误
    pub async fn upgrade_serverpack_with(
        &self,
        new_pack: ServerPackObj,
        cancel: CancellationToken,
        progress: Option<ServerPackProgress<'_>>,
        phase: Option<ServerPackPhase<'_>>,
        skip: &[String],
    ) -> CoreResult<()> {
        // 从比对本地文件到下完所有文件是一个任务，但前半段可能比下载还久
        // （几万个文件要逐个算哈希），先报出来，界面别一直停在"下载缺失文件 0"
        if let Some(phase) = phase {
            phase(AddModPackState::CheckFile);
        }
        let old = self.get_old_serverpack()?;
        // 旧清单里被移除、且要求删目录的配置目录：后面判断子目录要不要跟着重解时要用
        let mut removed_dirs: Vec<String> = Vec::new();

        let game_path = self.get_game_path();
        let temp_path = self.get_temp_path();
        paths::validate_pack(&new_pack, &game_path, &temp_path)?;
        if let Some(old) = &old {
            paths::validate_pack(old, &game_path, &temp_path)?;
        }

        // 玩家这次没勾选的条目：路径来自新清单，按身份判断哪些旧文件要跟着留下
        let skip_set: HashSet<&str> = skip.iter().map(String::as_str).collect();
        let skipped_keys: HashSet<String> = new_pack
            .online_list
            .iter()
            .filter(|item| skip_set.contains(item.file.as_str()))
            .map(plan::mod_key)
            .collect();

        // 删旧文件之前先记下玩家启用着的条目：可选模组换文件名时按它延续启用状态，
        // 否则更新一次版本就被重新装成禁用了
        let mut kept_enabled: HashSet<String> = HashSet::new();
        if let Some(old) = &old {
            for item in &old.online_list {
                // 上次跳过、本地留的是旧版本时，那份文件记在跳过记录里，按它的路径看状态
                let local = paths::file_path(&game_path, &item.file)
                    .ok()
                    .and_then(|target| existing_file(&target))
                    .or_else(|| {
                        let marker = old
                            .skipped
                            .iter()
                            .find(|marker| marker.file == item.file)?;
                        let kept = marker.kept.trim();
                        if kept.is_empty() {
                            return None;
                        }
                        let path = paths::file_path(&game_path, kept).ok()?;
                        existing_file(&path)
                    });
                if local.is_some_and(|(_, disabled)| !disabled) {
                    kept_enabled.insert(plan::mod_key(item));
                }
            }
        }

        if let Some(old) = &old {
            // 删什么由纯函数算，见 `serverpack::plan`，这里只负责执行
            let plan = plan::plan_removals(old, &new_pack);

            // 删除已移除或已更换路径的文件，`file` 是游戏目录下的相对路径
            for item in &plan.files {
                if cancel.is_cancelled() {
                    return Err(ErrorType::TaskCancel);
                }
                // 这次不更新的条目：旧文件留着，别删完又不装新的
                if skipped_keys.contains(&plan::mod_key(item)) {
                    continue;
                }
                let target = paths::file_path(&game_path, &item.file)?;
                delete_with_disabled(&target);
                // 服务器包同步会删掉客户端里的旧模组，映射记录也要一起清
                mml_link::remove_link(self.uuid, &self.get_base_path(), &target);
            }

            // 删除已移除的配置文件，仅删除由该配置独享的目录；先删更深的，
            // 否则父目录进回收站后子目录已经不存在
            let mut dirs: Vec<&ServerArchiveItemObj> = plan.dirs.clone();
            dirs.sort_by_key(|item| std::cmp::Reverse(item.dir.matches('/').count()));
            for item in dirs {
                if cancel.is_cancelled() {
                    return Err(ErrorType::TaskCancel);
                }
                let dir = paths::file_path(&game_path, &item.dir)?;
                if dir.exists() {
                    path_helper::move_to_trash(dir)?;
                }
                removed_dirs.push(item.dir.clone());
            }
        }

        // 上次跳过、这次重新勾上的条目：清掉当时留下的旧文件，否则两个版本会同时躺在 mods 里
        if let Some(old) = &old {
            let wanted: HashSet<&str> = new_pack
                .online_list
                .iter()
                .map(|item| item.file.as_str())
                .collect();
            for marker in &old.skipped {
                // 这次还是没勾：继续留着
                if skip_set.contains(marker.file.as_str()) {
                    continue;
                }
                let kept = marker.kept.trim();
                // 新版本用的还是这个路径时交给下载覆盖，不先删
                if kept.is_empty() || wanted.contains(kept) {
                    continue;
                }
                let Ok(path) = paths::file_path(&game_path, kept) else {
                    continue;
                };
                delete_with_disabled(&path);
                mml_link::remove_link(self.uuid, &self.get_base_path(), &path);
            }
        }

        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        // 先按本地哈希筛出真正要下的文件：只删不增的更新在这里就没有待下载项，
        // 后面既不解析下载地址、也不进下载阶段
        let check_total = new_pack.online_list.len();
        let mut pending: Vec<&ServerItemObj> = Vec::new();
        for (index, item) in new_pack.online_list.iter().enumerate() {
            // 对比期间每 128 个报一次：几万个文件逐个算哈希，界面不能一直停在 0
            if index % 128 == 0
                && let Some(report) = progress
            {
                report(index, check_total);
            }
            // 玩家这次没勾：不下载，本地旧文件原样留着
            if skip_set.contains(item.file.as_str()) {
                continue;
            }
            let target = paths::file_path(&game_path, &item.file)?;
            // 禁用中的文件同样算已装好：只比内容，不把玩家停用的模组又装回启用
            let existing = existing_file(&target);
            if let Some((path, _)) = &existing
                && matches_hash(item, path)
            {
                continue;
            }
            pending.push(item);
        }

        // 地址解析只针对真要下的那几个：整份清单逐个查接口，在只删不增的更新里纯属白跑
        let need_resolve: Vec<&ServerItemObj> = pending
            .iter()
            .copied()
            .filter(|i| !i.custom && i.url.as_deref().map_or(true, |u| u.is_empty()))
            .collect();
        let url_map = resolve_download_urls(&need_resolve, &cancel).await;

        // 旧清单里按压缩包名索引：同名的才可能内容没变
        let previous_archives: HashMap<&str, &ServerArchiveItemObj> = old
            .as_ref()
            .map(|old| {
                old.archive_list
                    .iter()
                    .map(|item| (item.file.as_str(), item))
                    .collect()
            })
            .unwrap_or_default();

        // 先判断哪些压缩包可以跳过。父包被重新解压（Over）或被删除时，
        // 子目录即使哈希没变也必须重解，否则父包会把它擦掉。
        let mut skipped: HashSet<&str> = HashSet::new();
        for item in &new_pack.archive_list {
            if archive_unchanged(
                previous_archives.get(item.file.as_str()).copied(),
                item,
                &game_path,
            ) {
                skipped.insert(item.dir.as_str());
            }
        }
        let mut cleared: Vec<&str> = removed_dirs.iter().map(String::as_str).collect();
        for item in &new_pack.archive_list {
            if !skipped.contains(item.dir.as_str()) && item.delete_old {
                cleared.push(item.dir.as_str());
            }
        }

        let mut downloads: Vec<FileItemObj> = Vec::new();
        let mut archives: Vec<(&ServerArchiveItemObj, PathBuf)> = Vec::new();

        // 基础文件包只发一次：旧清单已经有基础包就跳过，后续版本不再拉取
        let base_pending = new_pack
            .base
            .as_ref()
            .filter(|_| old.as_ref().and_then(|pack| pack.base.as_ref()).is_none());
        let mut base_task: Option<(&ServerBaseObj, PathBuf)> = None;
        if let Some(base) = base_pending {
            let temp = paths::file_path(&temp_path, &base.file)?;
            downloads.push(FileItemObj {
                url: base.url.clone(),
                name: base.file.clone(),
                file: temp.clone(),
                hash: plan::make_hash(&base.sha1, &base.sha256),
                later: LaterRun::None,
            });
            base_task = Some((base, temp));
        }

        for item in &pending {
            // 玩家之前启用着这条（可能换了文件名）时，新版本照样装成启用
            let keep_enabled = kept_enabled.contains(&plan::mod_key(item));
            let target = paths::file_path(&game_path, &item.file)?;
            let existing = existing_file(&target);
            let file = plan::install_path(
                &target,
                existing.as_ref().map(|(path, _)| path.as_path()),
                item.optional && !keep_enabled,
            );
            let url = item
                .url
                .clone()
                .filter(|u| !u.is_empty())
                .or_else(|| item.fid.as_ref().and_then(|f| url_map.get(f).cloned()))
                .unwrap_or_default();
            downloads.push(FileItemObj {
                url,
                name: item.file.clone(),
                file,
                hash: plan::make_hash(&item.sha1, &item.sha256),
                later: LaterRun::None,
            });
        }

        for item in &new_pack.archive_list {
            // 内容没变、目标目录也还在时可以跳过；但祖先目录会被清掉时不能跳
            let skip = skipped.contains(item.dir.as_str())
                && !cleared
                    .iter()
                    .any(|ancestor| under(item.dir.as_str(), ancestor));
            if skip {
                continue;
            }
            let temp = paths::file_path(&temp_path, &item.file)?;
            downloads.push(FileItemObj {
                url: item.url.clone(),
                name: item.file.clone(),
                file: temp.clone(),
                hash: plan::make_hash(&item.sha1, &item.sha256),
                later: LaterRun::None,
            });
            archives.push((item, temp));
        }

        // 真要下载时才切「下载缺失文件」：只删不增的更新没有待下载项，别让界面停在下载上
        if !downloads.is_empty()
            && let Some(phase) = phase
        {
            phase(AddModPackState::DownloadFile);
        }
        download_all(downloads, &cancel, progress).await?;

        // 基础包解压到实例根目录（.minecraft 所在的那一级），只解这一次
        if let Some((_, temp)) = &base_task {
            let result = extract_base(&self.get_base_path(), temp);
            path_helper::delete(temp)?;
            result?;
        }

        // 父目录的包先解，子目录的包后解；同层保持清单顺序
        archives.sort_by_key(|(item, _)| item.dir.matches('/').count());

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

        // 这次没更新的条目记进本地清单：发布方换了内容才失效，确认窗到时再问一次
        let mut saved = new_pack;
        saved.skipped = skipped_markers(&game_path, old.as_ref(), &saved.online_list, &skip_set);

        // 保存新版本信息，清理旧文件
        self.save_serverpack(&saved);
        path_helper::delete(self.get_server_pack_old_file())?;

        // 清单里带项目号的文件回写在线信息，导入后才能当平台来源用
        self.sync_online_info(&saved, &game_path);
        // 升版常常连加载器一起往上走，实例得跟着改，否则用旧加载器配新模组
        self.apply_pack_setting(&saved);

        Ok(())
    }

    /// 把清单里的游戏版本与加载器写回实例设置
    ///
    /// 写回走注册表：调用方手里的 `self` 是快照，`get_instance` 拿到的才是锁里那份。
    ///
    /// - `pack`: 本次同步的清单
    fn apply_pack_setting(&self, pack: &ServerPackObj) {
        let Some(instance) = crate::get_instance(&self.uuid) else {
            return;
        };
        let mut game = instance.write().unwrap();
        if game.modpack_type != ModPackType::ServerPack {
            return;
        }

        let mut changed = false;
        let version = pack.version.trim();
        if !version.is_empty() && game.version != version {
            game.version = version.to_string();
            changed = true;
        }
        if game.loader != pack.loader {
            game.loader = pack.loader;
            changed = true;
        }
        let loader_version = pack
            .loader_version
            .as_deref()
            .map(str::trim)
            .filter(|item| !item.is_empty());
        if let Some(loader_version) = loader_version
            && game.loader_version.as_deref() != Some(loader_version)
        {
            game.loader_version = Some(loader_version.to_string());
            changed = true;
        }

        if changed {
            game.save();
        }
    }

    /// 用清单里的项目号 / 文件号刷新实例的在线信息
    ///
    /// 表里只放**本地真的存在**的文件：清单里有编号但没落盘的不记，用户自己加的模组原样留着。
    /// 只收项目号与文件号都有的条目 —— 这张表的下游（更新检查、依赖检测、项目编号列、
    /// 生成服务器实例）全都按编号查平台，只有下载地址的配置文件记进来只是噪音。
    ///
    /// - `new_pack`: 本次同步的清单
    /// - `game_path`: 游戏目录
    fn sync_online_info(&self, new_pack: &ServerPackObj, game_path: &Path) {
        let old = self.read_online_info();
        let mut info = old.clone();

        // 清单接管的文件先按文件名摘掉旧记录，再按本次的编号重建
        let owned: HashSet<String> = new_pack
            .online_list
            .iter()
            .filter_map(|item| file_name_of(&item.file))
            .collect();
        info.retain(|_, item| {
            !owned.contains(&item.file) && existing_file(&local_file(game_path, item)).is_some()
        });

        for item in new_pack.online_list.iter().filter(|item| !item.custom) {
            let (Some(pid), Some(fid)) =
                (trimmed(item.pid.as_deref()), trimmed(item.fid.as_deref()))
            else {
                continue;
            };
            // 本地没有这个文件就没什么可记的；禁用中的也算，玩家随时会再启用
            let Ok(target) = paths::file_path(game_path, &item.file) else {
                continue;
            };
            let Some((local, _)) = existing_file(&target) else {
                continue;
            };

            // 表的主键是 SHA1，按本地文件为准：清单没给校验值就现算
            let sha1 = match trimmed(item.sha1.as_deref()) {
                Some(value) => value.to_string(),
                None => match gen_hash_from_file(HashType::Sha1, &local) {
                    Ok(value) => value,
                    Err(_) => continue,
                },
            };

            let entry = OnlineInfoObj {
                path: dir_of(&item.file),
                name: file_name_of(&item.file).unwrap_or_default(),
                file: file_name_of(&item.file).unwrap_or_default(),
                sha1: sha1.clone(),
                url: item.url.clone().unwrap_or_default(),
                modid: pid.to_string(),
                fileid: fid.to_string(),
            };
            info.insert(sha1, entry);
        }

        // 内容没变就别写盘：每次启动都会同步，白写一遍十几万字节没意义
        if info != old {
            self.save_online_info(&info);
        }
    }
}

/// 去掉首尾空白，空串按没有处理
///
/// - `value`: 原始值
///
/// # 返回值
///
/// 去空白后的值；空串或 `None` 返回 `None`
fn trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|item| !item.is_empty())
}

/// 清单里的相对路径对应的文件名
///
/// - `rel`: 游戏目录下的相对路径，如 `mods/foo.jar`
///
/// # 返回值
///
/// 文件名；路径为空时返回 `None`
fn file_name_of(rel: &str) -> Option<String> {
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
fn dir_of(rel: &str) -> String {
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
fn local_file(game_path: &Path, info: &OnlineInfoObj) -> PathBuf {
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
fn existing_file(target: &Path) -> Option<(PathBuf, bool)> {
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
fn matches_hash(item: &ServerItemObj, path: &Path) -> bool {
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
fn extract_base(base: &Path, file: &Path) -> CoreResult<()> {
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
fn server_metadata(name: &str) -> bool {
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
fn under(dir: &str, ancestor: &str) -> bool {
    dir.starts_with(ancestor) && dir.as_bytes().get(ancestor.len()) == Some(&b'/')
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
fn archive_unchanged(
    previous: Option<&ServerArchiveItemObj>,
    current: &ServerArchiveItemObj,
    game_path: &Path,
) -> bool {
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
/// 写进本地清单的跳过记录，顺序与清单一致
fn skipped_markers(
    game_path: &Path,
    old: Option<&ServerPackObj>,
    items: &[ServerItemObj],
    skip: &HashSet<&str>,
) -> Vec<ServerSkippedObj> {
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
            ServerSkippedObj {
                file: item.file.clone(),
                sha1: item.sha1.clone().unwrap_or_default(),
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
fn relative_to(base: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(base).ok()?;
    Some(rel.to_string_lossy().replace('\\', "/"))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 禁用文件两个后缀都认，优先新后缀；模组以外的文件改名停用同样算
    #[test]
    fn existing_file_accepts_legacy_suffix() {
        let dir = mml_testutil::temp_path(format!("mml-existing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("mods")).unwrap();
        let target = dir.join("mods/a.jar");

        std::fs::write(dir.join("mods/a.jar.disable"), b"old").unwrap();
        let found = existing_file(&target).expect("旧后缀也要认");
        assert!(found.1);
        assert!(found.0.ends_with("a.jar.disable"));

        std::fs::write(dir.join("mods/a.jar.disabled"), b"new").unwrap();
        let found = existing_file(&target).expect("新后缀要认");
        assert!(found.1);
        assert!(found.0.ends_with("a.jar.disabled"), "两个都在时用新后缀");

        std::fs::write(dir.join("config.toml.disable"), b"x").unwrap();
        assert!(
            existing_file(&dir.join("config.toml")).is_some(),
            "不限于 .jar：任何被改名停用的文件都要认"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn under_only_matches_descendants() {
        assert!(under("config/setting", "config"));
        assert!(under("config/setting/deep", "config/setting"));
        assert!(!under("config", "config"));
        assert!(!under("config2", "config"));
        assert!(!under("config/setting", "config2"));
    }
}
