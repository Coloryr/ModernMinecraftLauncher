//! CurseForge 整合包安装器

use std::{collections::HashMap, path::Path, sync::Mutex};

use async_trait::async_trait;
use mml_base::{
    file_item::{FileHash, FileItemObj, LaterRun},
    serialize_tools,
};
use mml_names::{
    i18_items::error_type::{CoreResult, DataNotFoundData, ErrorType},
    names,
};
use mml_net::{
    curseforge_api,
    input_file::InputFile,
};
use mml_sys::path_helper;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::diff;
use super::manifest;
use crate::{
    GameInstance,
    curseforge::{
        self,
        pack_obj::CurseForgePackObj,
    },
    gui_hook::AddModPackGui,
    launcher::{
        ModPackType, file_online_info_obj::OnlineInfoObj, instance_setting_obj::InstanceSettingObj,
    },
    launcher_path::{instance_path::OnlineInfoList, version_path},
    loader::LoaderType,
    modpack::{BaseModPackWorker, ModPackWorker},
};

/// CurseForge 整合包安装器
pub struct CurseForgeWorker {
    /// 整合包信息，`read_info` 之前为 `None`
    info: Option<CurseForgePackObj>,
    /// 基础安装器，负责实例创建、解压与下载列表
    base: BaseModPackWorker,
}

impl CurseForgeWorker {
    /// 创建 CurseForge 整合包安装器
    ///
    /// - `base`: 基础安装器
    ///
    /// # 返回值
    ///
    /// 返回安装器实例，`info` 初始为空
    pub fn new(base: BaseModPackWorker) -> Self {
        Self { info: None, base }
    }
}

#[async_trait]
impl ModPackWorker for CurseForgeWorker {
    /// 获取主信息，从压缩包里读出 `manifest.json`
    fn read_info(&mut self) -> CoreResult<()> {
        let data = self
            .base
            .archive
            .entries()
            .iter()
            .filter(|item| item.name.eq_ignore_ascii_case(names::MANIFEST_FILE))
            .next();

        if let Some(item) = data {
            let data1 =
                self.base.archive.read(&item.name).and_then(|data| {
                    serialize_tools::json_from_bytes::<CurseForgePackObj>(&data)
                })?;

            self.info = Some(data1);
            Ok(())
        } else {
            Err(ErrorType::DataNotFound(DataNotFoundData::Info))
        }
    }

    /// 获取版本数据，解析 modLoaders 得到加载器与其版本，并确认游戏版本可用
    async fn read_version(&mut self) -> CoreResult<()> {
        if self.info.is_none() {
            return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
        }

        let info = self.info.as_ref().unwrap();

        for item in info.minecraft.mod_loaders.iter() {
            if item.id.starts_with(names::FORGE_KEY) {
                self.base.loader = LoaderType::Forge;
                self.base.loader_version = item.id.replace(&format!("{}-", names::FORGE_KEY), "");
            } else if item.id.starts_with(names::FABRIC_KEY) {
                self.base.loader = LoaderType::Fabric;
                self.base.loader_version = item.id.replace(&format!("{}-", names::FABRIC_KEY), "");
            } else if item.id.starts_with(names::NEOFORGE_KEY) {
                self.base.loader = LoaderType::NeoForge;
                self.base.loader_version =
                    item.id.replace(&format!("{}-", names::NEOFORGE_KEY), "");
            } else if item.id.starts_with(names::QUILT_KEY) {
                self.base.loader = LoaderType::Quilt;
                self.base.loader_version = item.id.replace(&format!("{}-", names::QUILT_KEY), "");
            }
        }

        let minecraft = &self.info.as_ref().unwrap().minecraft.version;
        let version = &self.base.loader_version;

        if version.starts_with(&format!("{}-", minecraft)) && version.len() > minecraft.len() + 1 {
            self.base.loader_version = version[(minecraft.len() + 1)..].to_string();
        }

        self.base.game_version = minecraft.clone();

        version_path::check_update(minecraft).await?;

        Ok(())
    }

    /// 创建游戏实例
    ///
    /// - `name`: 实例名；`None` 时用 [`ModPackWorker::default_instance_name`]
    /// - `group`: 所属分组 uuid；`None` 表示不分组
    ///
    /// # 返回值
    ///
    /// 返回新建实例的 uuid
    async fn create_instance(&self, name: Option<String>, group: Option<Uuid>) -> CoreResult<Uuid> {
        let name = match name {
            Some(name) => name,
            None => self
                .default_instance_name()
                .ok_or(ErrorType::DataNotFound(DataNotFoundData::Info))?,
        };
        let game = InstanceSettingObj {
            name,
            version: self.base.game_version.clone(),
            is_modpack: true,
            loader: self.base.loader,
            modpack_type: ModPackType::CurseForge,
            loader_version: Some(self.base.loader_version.clone()),
            ..Default::default()
        };
        // 分组不在实例配置里，创建时一并交给分组表，不存在的组会被建出来
        let game = game
            .create_instance_in_group(self.base.instance_gui.clone(), group)
            .await?;

        // 与 Modrinth 安装器同一套：图标在实例建好后立刻写，写失败只记日志
        if let Some(icon) = &self.base.icon {
            if let Err(err) =
                InstanceSettingObj::save_icon(&game, InputFile::Url(icon.clone())).await
            {
                mml_log::error_type(err);
            }
        }
        Ok(game.read().unwrap().uuid)
    }

    /// 默认实例名，取整合包名与版本拼出的那一个
    ///
    /// # 返回值
    ///
    /// 已读到整合包信息时返回 `{包名}-{版本}`，否则返回 `None`
    fn default_instance_name(&self) -> Option<String> {
        self.info
            .as_ref()
            .map(|info| format!("{}-{}", info.name, info.version))
    }

    /// 解压整合包覆盖文件到游戏目录
    ///
    /// `overrides/` 下的文件去除前缀后写入游戏根目录，其余文件直接写入游戏路径。
    ///
    /// - `unselect`: 不参与覆盖的文件名列表，`None` 表示全部覆盖
    ///
    /// # 返回值
    ///
    /// 解压失败时返回对应错误
    async fn extract(&self, unselect: Option<Vec<String>>) -> CoreResult<()> {
        let Some(info) = &self.info else {
            return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
        };
        self.base.extract_pack_files(&info.overrides, unselect)
    }

    /// 获取模组下载信息
    ///
    /// 批量解析 manifest 中的文件列表，构建下载项并存入 `base.downloads`，
    /// 后续由 [`Self::download`] 统一下载。
    ///
    /// # 返回值
    ///
    /// 有下载项或整合包本身不含文件时返回 `true`
    async fn get_info(&self) -> CoreResult<bool> {
        let Some(game) = &self.base.game else {
            return Err(ErrorType::DataNotFound(DataNotFoundData::GameInstance));
        };
        let Some(info) = &self.info else {
            return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
        };

        let list = curseforge::get_modpack_info(game, info, &self.base.pack_gui).await?;

        let downloads = list.list;
        let mods = list.online;

        game.read().unwrap().save_online_info(&mods);

        let mut guard = self.base.downloads.lock().unwrap();
        *guard = downloads;

        Ok(!guard.is_empty() || info.files.is_empty())
    }

    /// 统一下载所有模组文件
    async fn download(&self) {
        // 先取出下载列表再 await：取走后列表为空，重复调用不会重复下载
        let items = {
            let Ok(mut guard) = self.base.downloads.lock() else {
                return;
            };
            std::mem::take(&mut *guard)
        };
        if items.is_empty() {
            return;
        }
        // 挂上取消令牌：安装任务取消时同步中断剩余文件的下载
        mml_downloader::start_download_task_cancellable(items, self.base.cancel.clone()).await;
    }

    /// 更新游戏实例版本信息
    ///
    /// - `game`: 目标实例
    fn update_game(&mut self, game: &GameInstance) {
        self.base.game = Some(game.clone());

        let mut game = game.write().unwrap();
        game.loader = self.base.loader;
        game.loader_version = Some(self.base.loader_version.clone());
        game.version = self.base.game_version.clone();

        game.save();
    }

    /// 检查整合包更新
    ///
    /// 对比当前整合包 manifest 与上一次安装的 manifest：
    /// - 存在旧 manifest：用 fileID 快速比对，找出新增、变更与删除的模组
    /// - 无旧 manifest：通过 API 获取文件信息后比对 SHA1
    ///
    /// 最终将需要下载的文件存入 `base.downloads`。
    ///
    /// # 返回值
    ///
    /// 取消或查询失败时返回对应错误
    async fn check_upgrade(&self) -> CoreResult<()> {
        let Some(game) = &self.base.game else {
            return Err(ErrorType::DataNotFound(DataNotFoundData::GameInstance));
        };
        let Some(info) = &self.info else {
            return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
        };

        // 读旧清单：缺失、打不开或非法 JSON 一律当作"没有旧清单"，见 `modpack::manifest`
        let old_info = {
            let game = game.read().unwrap();
            let old_manifest_path = game.get_base_path().join(names::MANIFEST_FILE);
            manifest::read_manifest::<CurseForgePackObj>(&old_manifest_path)
        };

        if let Some(old_info) = old_info {
            // 有旧 manifest：用 fileID 比对
            check_upgrade_with_old_manifest(
                info,
                &old_info,
                game,
                &self.base.downloads,
                &self.base.pack_gui,
                &self.base.cancel,
            )
            .await?;
        } else {
            // 无旧 manifest：通过 SHA1 比对
            check_upgrade_sha1(
                info,
                game,
                &self.base.downloads,
                &self.base.pack_gui,
                &self.base.cancel,
            )
            .await?;
        }

        // 写入当前整合包 manifest，作为下一次升级比对用的旧清单
        // 放在最后一步：前面任何 `?` 提前返回都不会动到旧清单
        manifest::write_manifest(
            game.read().unwrap().get_base_path().join(names::MANIFEST_FILE),
            info,
        )?;

        Ok(())
    }
}

/// 有旧 manifest 时：对比两边的 `Files` 列表，找出新增、删除、变更
///
/// 1. 以 `project_id` 为键比对新旧文件列表
/// 2. 同 project 但不同 file_id 视为变更，下载新版本并删除旧版本
/// 3. 仅在新 manifest 中视为新增
/// 4. 仅在旧 manifest 中视为删除
/// 5. 对每个需要下载的文件调用 API 解析，构建下载项并更新在线信息
///
/// - `new_info`: 新版整合包信息
/// - `old_info`: 旧版整合包信息
/// - `game`: 目标实例
/// - `downloads`: 下载列表，写入需要下载的文件
/// - `pack_gui`: 整合包安装界面回调
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 取消或查询失败时返回对应错误
async fn check_upgrade_with_old_manifest(
    new_info: &CurseForgePackObj,
    old_info: &CurseForgePackObj,
    game: &GameInstance,
    downloads: &Mutex<Vec<FileItemObj>>,
    pack_gui: &AddModPackGui,
    cancel: &CancellationToken,
) -> CoreResult<()> {
    // 差异计算已抽到 `modpack::diff`，纯函数、离线可测；判定规则一字未改
    let diff = diff::diff_curseforge_files(&new_info.files, &old_info.files);
    let add_list = diff.add;
    let remove_list = diff.remove;

    // 删文件前先看一次取消，避免任务已取消还去动磁盘
    if cancel.is_cancelled() {
        return Err(ErrorType::TaskCancel);
    }

    // 删除已移除的模组
    {
        let game = game.read().unwrap();
        let game_path = game.get_game_path();
        let mut online_info = game.read_online_info();

        for item in &remove_list {
            let project_id_str = item.project_id.to_string();
            if let Some(mod_info) = online_info.remove(&project_id_str) {
                let local = game_path.join(&mod_info.path).join(&mod_info.file);
                delete_with_disabled(local);
            }
        }

        game.save_online_info(&online_info);
    }

    if cancel.is_cancelled() {
        return Err(ErrorType::TaskCancel);
    }

    // 无需下载，直接完成
    if add_list.is_empty() {
        downloads.lock().unwrap().clear();
        return Ok(());
    }

    // 逐个解析新增/变更的文件并构建下载列表
    let (mods_path, mut online_info) = {
        let game = game.read().unwrap();
        (game.get_mods_path(), game.read_online_info())
    };

    let total = add_list.len();
    if let Some(pgui) = pack_gui {
        pgui.set_sub_now(0, Some(total));
    }

    let mut new_downloads: Vec<FileItemObj> = Vec::with_capacity(total);

    for (b, item) in add_list.iter().enumerate() {
        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        let pid = item.project_id.to_string();
        let fid = item.file_id.to_string();

        let res = curseforge_api::get_mod(&pid, &fid).await?;

        let mut data = res.data;
        let mod_id_str = data.mod_id.to_string();
        let sha1 = data.sha1_hash();

        data.fix_download_url();

        let download = FileItemObj {
            url: data.download_url.clone().unwrap_or_default(),
            name: data.display_name.clone(),
            file: mods_path.join(&data.file_name),
            hash: FileHash::Sha1(sha1.clone()),
            later: LaterRun::None,
        };
        new_downloads.push(download);

        // 更新在线信息：移除旧条目后插入新条目
        online_info.remove(&mod_id_str);
        online_info.insert(
            mod_id_str,
            OnlineInfoObj {
                path: names::GAME_MODS_DIR.to_string(),
                name: data.display_name.clone(),
                file: data.file_name.clone(),
                sha1,
                url: data.download_url.unwrap_or_default(),
                modid: data.mod_id.to_string(),
                fileid: data.id.to_string(),
            },
        );

        if let Some(pgui) = pack_gui {
            pgui.set_sub_now(b + 1, Some(total));
        }
    }

    // 保存更新后的在线信息
    game.read().unwrap().save_online_info(&online_info);
    *downloads.lock().unwrap() = new_downloads;

    Ok(())
}

/// 无旧 manifest 时：通过 API 获取最新文件信息，以 mod_id 和 SHA1 比对
///
/// 1. 批量解析 manifest 中的全部文件
/// 2. 构建 mod_id 到在线信息的映射
/// 3. 与现有 `online_info` 比对：同 mod_id 但 fileid 或 sha1 不同视为变更
/// 4. 仅在在线信息中不存在视为新增
/// 5. 删除旧文件，构建下载列表
///
/// - `new_info`: 新版整合包信息
/// - `game`: 目标实例
/// - `downloads`: 下载列表，写入需要下载的文件
/// - `pack_gui`: 整合包安装界面回调
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 取消或查询失败时返回对应错误
async fn check_upgrade_sha1(
    new_info: &CurseForgePackObj,
    game: &GameInstance,
    downloads: &Mutex<Vec<FileItemObj>>,
    pack_gui: &AddModPackGui,
    cancel: &CancellationToken,
) -> CoreResult<()> {
    let file_ids: Vec<u64> = new_info.files.iter().map(|f| f.file_id).collect();
    if file_ids.is_empty() {
        downloads.lock().unwrap().clear();
        return Ok(());
    }

    // 批量解析文件信息，含进度回调
    let resolved_files = {
        if let Some(pgui) = pack_gui {
            pgui.set_sub_now(0, Some(new_info.files.len()));
        }

        let mut files = Vec::new();

        // 批量获取
        if let Ok(items) = curseforge_api::get_files(file_ids.clone()).await {
            files = items;
        } else {
            // 逐文件降级
            let total = file_ids.len();
            for (idx, &fid) in file_ids.iter().enumerate() {
                if cancel.is_cancelled() {
                    return Err(ErrorType::TaskCancel);
                }
                let fid_str = fid.to_string();
                if let Ok(data) = curseforge_api::get_mod(&fid_str, &fid_str).await {
                    files.push(data.data);
                }
                if let Some(pgui) = pack_gui {
                    pgui.set_sub_now(idx + 1, Some(total));
                }
            }
        }

        if files.is_empty() {
            return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
        }
        files
    };

    if cancel.is_cancelled() {
        return Err(ErrorType::TaskCancel);
    }

    // 从解析结果构建 mod_id 到在线信息与下载项的映射
    let (game_path, mods_path) = {
        let game = game.read().unwrap();
        (game.get_game_path(), game.get_mods_path())
    };

    // new_online_map 是 mod_id 到在线文件信息，new_download_map 是 mod_id 到下载项
    let mut new_online_map: OnlineInfoList = HashMap::new();
    let mut new_download_map: HashMap<String, FileItemObj> = HashMap::new();

    for file_data in &resolved_files {
        let mod_id_str = file_data.mod_id.to_string();
        let sha1 = file_data.sha1_hash();

        let download = FileItemObj {
            url: file_data.download_url.clone().unwrap_or_default(),
            name: file_data.display_name.clone(),
            file: mods_path.join(&file_data.file_name),
            hash: FileHash::Sha1(sha1.clone()),
            later: LaterRun::None,
        };

        let online = OnlineInfoObj {
            path: names::GAME_MODS_DIR.to_string(),
            name: file_data.display_name.clone(),
            file: file_data.file_name.clone(),
            sha1,
            url: file_data.download_url.clone().unwrap_or_default(),
            modid: mod_id_str.clone(),
            fileid: file_data.id.to_string(),
        };

        new_online_map.insert(mod_id_str.clone(), online);
        new_download_map.insert(mod_id_str, download);
    }

    // 比对现有在线信息
    let mut online_info = {
        let game = game.read().unwrap();
        game.read_online_info()
    };

    // 差异计算已抽到 `modpack::diff`，纯函数、离线可测；判定规则一字未改
    let diff = diff::diff_curseforge_online(&new_online_map, &online_info);
    let add_list = diff.add;
    let remove_list = diff.remove;

    // 删除旧文件
    for item in &remove_list {
        delete_with_disabled(game_path.join(&item.path).join(&item.file));
        online_info.remove(&item.modid);
    }

    // 构建下载列表
    let mut new_downloads: Vec<FileItemObj> = Vec::new();

    for item in &add_list {
        if let Some(download) = new_download_map.get(&item.modid) {
            new_downloads.push(download.clone());
            online_info.insert(item.modid.clone(), item.clone());
        }
    }

    // 保存更新后的在线信息
    game.read().unwrap().save_online_info(&online_info);
    *downloads.lock().unwrap() = new_downloads;

    Ok(())
}

/// 删除文件，若文件已被禁用并追加了 `.disable` 或 `.disabled` 后缀则一并删除
///
/// - `file`: 待删除的文件路径
fn delete_with_disabled<P: AsRef<Path>>(file: P) {
    let file = file.as_ref();
    // 文件不存在时 delete 是无操作
    let _ = path_helper::delete(file);
    let _ = path_helper::delete(format!("{}{}", file.display(), names::DISABLE_DOT_EXT));
    let _ = path_helper::delete(format!("{}{}", file.display(), names::DISABLED_DOT_EXT));
}
