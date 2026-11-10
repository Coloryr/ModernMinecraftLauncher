//! 服务器包升级
//!
//! 拉新清单、比对本地文件、下载缺失项，最后把版本写回实例设置。

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use mml_base::{
    archives::{ArchiveType, BaseArchive},
    file_item::{FileItemObj, LaterRun},
};
use mml_names::i18_items::error_type::{CoreResult, ErrorType, FileSystemErrorData};
use mml_sys::path_helper;
use tokio_util::sync::CancellationToken;

use crate::{
    gui_hook::AddModPackState,
    launcher::instance_setting_obj::InstanceSettingObj,
    serverpack::serverpack_obj::{
        ServerArchiveItemObj, ServerBaseObj, ServerItemObj, ServerPackObj,
    },
};

use super::files::{
    ServerPackPhase, ServerPackProgress, archive_has_no_files, archive_unchanged,
    delete_with_disabled, download_all, existing_file, extract_base, matches_hash,
    resolve_download_urls, skipped_markers, under,
};
use super::{paths, plan};

impl InstanceSettingObj {
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
        // 上次跳过更新留下的本地旧文件，存在实例的 gui_setting.json 里
        let previous_skips = self.skipped_state();

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
                        let marker = previous_skips
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
        }

        // 上次跳过、这次重新勾上的条目：清掉当时留下的旧文件，否则两个版本会同时躺在 mods 里
        let wanted: HashSet<&str> = new_pack
            .online_list
            .iter()
            .map(|item| item.file.as_str())
            .collect();
        for marker in &previous_skips {
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

        // 先判断哪些压缩包可以跳过。父包会被重新解压（Over）时，
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
        // 条目从新清单消失不删目录，只有这次会重解的压缩包才需要让子目录跟着重解
        let cleared: Vec<&str> = new_pack
            .archive_list
            .iter()
            .filter(|item| {
                (item.delete_old || item.action.is_delete()) && !skipped.contains(item.dir.as_str())
            })
            .map(|item| item.dir.as_str())
            .collect();

        let mut downloads: Vec<FileItemObj> = Vec::new();
        // 第二个元素是下载到的临时包；None 表示 Url 为空，只按 Path 删除
        let mut archives: Vec<(&ServerArchiveItemObj, Option<PathBuf>)> = Vec::new();

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
            // 删除条目：不下载，按 Path 把目标删掉
            if item.action.is_delete() {
                archives.push((item, None));
                continue;
            }
            let temp = paths::file_path(&temp_path, &item.file)?;
            downloads.push(FileItemObj {
                url: item.url.clone().unwrap_or_default(),
                name: item.file.clone(),
                file: temp.clone(),
                hash: plan::make_hash(&item.sha1, &item.sha256),
                later: LaterRun::None,
            });
            archives.push((item, Some(temp)));
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
            // 删除条目：DeletePath 删整棵目录、DeleteFile 删单个文件，都只按 Path 处理，不存在就跳过
            let Some(temp) = temp else {
                if output.exists() {
                    path_helper::move_to_trash(&output)?;
                }
                continue;
            };
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

            // 空包 + 覆盖：旧目录已移走，不再解压出一个空目录
            if item.delete_old && archive_has_no_files(temp, archive_type) {
                path_helper::delete(temp)?;
                continue;
            }

            BaseArchive::decompress(archive_type, temp, &output, None)?;

            // 清理临时压缩包
            path_helper::delete(temp)?;
        }

        // 这次没更新的条目记进实例 GUI 设置：发布方换了内容才失效，确认窗到时再问一次
        let records = skipped_markers(&game_path, old.as_ref(), &new_pack.online_list, &skip_set);
        self.save_skipped_state(&records);

        // 保存新版本信息，清理旧文件
        self.save_serverpack(&new_pack);
        path_helper::delete(self.get_server_pack_old_file())?;

        // 清单里带项目号的文件回写在线信息，导入后才能当平台来源用
        self.sync_online_info(&new_pack, &game_path);
        // 升版常常连加载器一起往上走，实例得跟着改，否则用旧加载器配新模组
        self.apply_pack_setting(&new_pack);

        Ok(())
    }
}
