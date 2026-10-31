//! Modrinth 整合包安装器

use std::path::Path;

use async_trait::async_trait;
use mml_base::{file_item::FileItemObj, serialize_tools, tools};
use mml_names::{
    i18_items::error_type::{CoreResult, DataNotFoundData, ErrorType},
    names,
};
use mml_net::{input_file::InputFile, urls};
use mml_sys::path_helper;
use uuid::Uuid;

use super::diff;
use super::manifest;
use crate::{
    GameInstance,
    launcher::{
        ModPackType, file_online_info_obj::OnlineInfoObj, instance_setting_obj::InstanceSettingObj,
    },
    launcher_path::version_path,
    loader::LoaderType,
    modpack::{BaseModPackWorker, ModPackWorker},
    modrinth::{self, pack_obj::ModrinthPackObj},
};

/// Modrinth 整合包安装器
pub struct ModrinthPackWorker {
    /// 解析出的 modrinth.index.json，`read_info` 之前为 `None`
    info: Option<ModrinthPackObj>,
    /// 安装流程的公共部分
    base: BaseModPackWorker,
}

impl ModrinthPackWorker {
    /// 创建 Modrinth 整合包安装器
    ///
    /// - `base`: 基础安装器
    ///
    /// # 返回值
    ///
    /// 返回新建的安装器
    pub fn new(base: BaseModPackWorker) -> Self {
        Self { info: None, base }
    }
}

#[async_trait]
impl ModPackWorker for ModrinthPackWorker {
    /// 读取 modrinth.index.json
    fn read_info(&mut self) -> CoreResult<()> {
        if let Some(item) = self
            .base
            .archive
            .entries()
            .iter()
            .filter(|item| item.name.eq_ignore_ascii_case(names::MODRINTH_FILE))
            .next()
        {
            let data1 = self
                .base
                .archive
                .read(&item.name)
                .and_then(|data| serialize_tools::json_from_bytes::<ModrinthPackObj>(&data))?;

            self.info = Some(data1);
            Ok(())
        } else {
            Err(ErrorType::DataNotFound(DataNotFoundData::Info))
        }
    }

    /// 从依赖里取游戏版本与加载器，并校验版本 json
    async fn read_version(&mut self) -> CoreResult<()> {
        if self.info.is_none() {
            return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
        }

        let info = self.info.as_ref().unwrap();

        if let Some(version) = info.dependencies.get(names::MINECRAFT_KEY) {
            self.base.game_version = version.clone();
        }

        // mrpack 规范的依赖键：minecraft / forge / neoforge / fabric-loader / quilt-loader
        if let Some(version) = info.dependencies.get(names::FORGE_KEY) {
            self.base.loader = LoaderType::Forge;
            self.base.loader_version = version.clone();
        }
        if let Some(version) = info.dependencies.get(names::FABRIC_LOADER_KEY) {
            self.base.loader = LoaderType::Fabric;
            self.base.loader_version = version.clone();
        }
        if let Some(version) = info.dependencies.get(names::NEOFORGE_KEY) {
            self.base.loader = LoaderType::NeoForge;
            self.base.loader_version = version.clone();
        }
        if let Some(version) = info.dependencies.get(names::QUILT_LOADER_KEY) {
            self.base.loader = LoaderType::Quilt;
            self.base.loader_version = version.clone();
        }

        version_path::check_update(&self.base.game_version).await?;

        Ok(())
    }

    /// 创建游戏实例
    ///
    /// - `name`: 实例名，传 `None` 时用 [`ModPackWorker::default_instance_name`]
    /// - `group`: 目标分组 uuid，`None` 表示不分组
    ///
    /// # 返回值
    ///
    /// 返回新实例的 UUID；创建失败返回对应错误
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
            modpack_type: ModPackType::Modrinth,
            loader_version: Some(self.base.loader_version.clone()),
            ..Default::default()
        };
        // 分组不在实例配置里，创建时一并交给分组表，不存在的组会被建出来
        let game = game
            .create_instance_in_group(self.base.instance_gui.clone(), group)
            .await?;

        if let Some(icon) = &self.base.icon {
            // 图标是装饰性的：写失败只记日志，不让整个安装失败，与 add_game 的处理一致
            if let Err(err) =
                InstanceSettingObj::save_icon(&game, InputFile::Url(icon.clone())).await
            {
                mml_log::error_type(err);
            }
        }
        Ok(game.read().unwrap().uuid)
    }

    /// 默认实例名，取整合包名与版本号拼出的那一个
    ///
    /// # 返回值
    ///
    /// 已读到整合包信息时返回 `{包名}-{版本号}`，否则返回 `None`
    fn default_instance_name(&self) -> Option<String> {
        self.info
            .as_ref()
            .map(|info| format!("{}-{}", info.name, info.version_id))
    }

    /// 解压整合包覆盖文件到游戏目录。
    ///
    /// `overrides/` 下的文件去除前缀后写入游戏根目录；其余文件直接
    /// 写入游戏路径。
    ///
    /// - `unselect`: 不解压的文件列表
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；解压失败返回对应错误
    async fn extract(&self, unselect: Option<Vec<String>>) -> CoreResult<()> {
        self.base.extract_pack_files(names::OVERRIDE_DIR, unselect)
    }

    /// 获取模组下载信息。
    ///
    /// 批量解析 manifest 中的文件列表，构建下载项并存入
    /// `base.downloads`，后续由 [`download`] 统一下载。
    ///
    /// # 返回值
    ///
    /// 返回是否成功取得全部模组信息；查询失败返回对应错误
    async fn get_info(&self) -> CoreResult<bool> {
        let Some(info) = &self.info else {
            return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
        };
        let Some(game) = &self.base.game else {
            return Err(ErrorType::DataNotFound(DataNotFoundData::GameInstance));
        };

        let path = game.read().unwrap().get_game_path();
        let list = modrinth::get_mod_info(
            path,
            info,
            self.base.pack_gui.clone(),
            self.base.cancel.clone(),
        )
        .await?;

        // 构建下载列表
        let downloads = list.list;
        let mods = list.online;

        game.read().unwrap().save_online_info(&mods);

        let mut guard = self.base.downloads.lock().unwrap();
        *guard = downloads;

        Ok(!guard.is_empty() || info.files.is_empty())
    }

    /// 统一下载所有模组文件。
    async fn download(&self) {
        // 取出下载列表并在 .await 前释放 MutexGuard；取走后列表为空，重复调用不会重复下载
        let mut items = {
            let Ok(mut guard) = self.base.downloads.lock() else {
                return;
            };
            std::mem::take(&mut *guard)
        };
        if items.is_empty() {
            return;
        }

        let mut plans = Vec::new();
        if let Some(game) = self.base.game.clone() {
            plans = crate::game_link::prepare_downloads(&game, &mut items);
        }
        if !items.is_empty() {
            // 挂上取消令牌：安装任务取消时同步中断剩余文件的下载，并把字节进度写进任务子进度
            crate::add_game::download_with_progress(
                items,
                self.base.pack_gui.clone(),
                self.base.cancel.clone(),
            )
            .await;
        }
        crate::game_link::finish_downloads(plans);
    }

    /// 更新游戏实例版本信息
    fn update_game(&mut self, game: &GameInstance) {
        self.base.game = Some(game.clone());

        let mut game = game.write().unwrap();
        game.loader = self.base.loader;
        game.loader_version = Some(self.base.loader_version.clone());
        game.version = self.base.game_version.clone();

        game.save();
    }

    /// 检查整合包更新。
    ///
    /// 对比当前整合包 manifest 与上一次安装的 manifest：
    /// - 存在旧 manifest：按 SHA1 对比，找出新增/变更/删除的文件
    /// - 无旧 manifest：通过 API 获取文件信息后按 mod_id 比对
    ///
    /// 最终将需要下载的文件存入 `base.downloads`。
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；读取失败返回对应错误
    async fn check_upgrade(&self) -> CoreResult<()> {
        let Some(info) = &self.info else {
            return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
        };
        let Some(game) = &self.base.game else {
            return Err(ErrorType::DataNotFound(DataNotFoundData::GameInstance));
        };

        // 读取上次安装时保存的整合包 manifest，存在 base 目录
        // 缺失 / 打不开 / 非法 JSON 一律当作"没有旧清单"，见 `modpack::manifest`
        let old_info = {
            let game = game.read().unwrap();
            let old_manifest_path = game.get_base_path().join(names::MODRINTH_FILE);
            manifest::read_manifest::<ModrinthPackObj>(&old_manifest_path)
        };

        // 获取新整合包的模组信息，含下载列表与在线信息
        let path = game.read().unwrap().get_game_path();
        let res = modrinth::get_mod_info(
            &path,
            info,
            self.base.pack_gui.clone(),
            self.base.cancel.clone(),
        )
        .await?;

        let mut online_info = game.read().unwrap().read_online_info();

        // 需要下载的文件列表
        let mut new_downloads: Vec<FileItemObj> = Vec::new();

        if let Some(old_info) = old_info {
            // 差异计算已抽到 `modpack::diff`，纯函数、离线可测；判定规则一字未改
            let diff = diff::diff_modrinth_files(&info.files, &old_info.files);
            let add_list = diff.add;
            let remove_list = diff.remove;

            // 删除被移除的文件
            for item in &remove_list {
                delete_with_disabled(path.join(&item.path));

                let url = item
                    .downloads
                    .iter()
                    .find(|u| u.starts_with(&format!("{}data/", urls::MODRINTH_DOWNLOAD)));
                if let Some(url) = url {
                    let modid = tools::get_string(url, "data/", "/ver");
                    online_info.remove(&modid);
                }
            }

            // 构建下载列表并更新在线信息
            for item in &add_list {
                let Some(download) = res
                    .list
                    .iter()
                    .find(|d| d.hash.get_sha1().as_deref() == Some(item.hashes.sha1.as_str()))
                    .cloned()
                else {
                    continue;
                };
                new_downloads.push(download);

                let url = item
                    .downloads
                    .iter()
                    .find(|u| u.starts_with(&format!("{}data/", urls::MODRINTH_DOWNLOAD)));
                if let Some(url) = url {
                    let modid = tools::get_string(url, "data/", "/ver");
                    let fileid = tools::get_string(url, "versions/", "/");

                    let path_part = tools::get_path_part(&item.path);
                    online_info.remove(&modid);
                    online_info.insert(
                        modid.clone(),
                        OnlineInfoObj {
                            path: path_part.parent,
                            name: path_part.file.clone(),
                            file: path_part.file,
                            sha1: item.hashes.sha1.clone(),
                            url: url.clone(),
                            modid,
                            fileid,
                        },
                    );
                }
            }
        } else {
            // 差异计算已抽到 `modpack::diff`，纯函数、离线可测；判定规则一字未改
            let diff = diff::diff_modrinth_online(&res.online, &online_info);
            let add_list = diff.add;
            let remove_list = diff.remove;

            // 删除旧文件
            for item in &remove_list {
                delete_with_disabled(path.join(&item.path).join(&item.file));
                online_info.remove(&item.modid);
            }

            // 构建下载列表并更新在线信息
            for item in &add_list {
                if let Some(download) = res
                    .list
                    .iter()
                    .find(|d| d.hash.get_sha1().as_deref() == Some(item.sha1.as_str()))
                    .cloned()
                {
                    new_downloads.push(download);
                }
                online_info.insert(item.modid.clone(), item.clone());
            }
        }

        // 保存更新后的在线信息
        game.read().unwrap().save_online_info(&online_info);

        // 写入当前整合包 manifest，放在最后一步：前面任何 `?` 提前返回都不会动到旧清单
        manifest::write_manifest(
            game.read()
                .unwrap()
                .get_base_path()
                .join(names::MODRINTH_FILE),
            info,
        )?;

        // 更新下载列表
        *self.base.downloads.lock().unwrap() = new_downloads;

        Ok(())
    }
}

/// 删除文件，若文件已被禁用则连同 `.disable`/`.disabled` 后缀那份一起删。
///
/// - `file`: 文件路径
fn delete_with_disabled<P: AsRef<Path>>(file: P) {
    let file = file.as_ref();
    // `delete` 在文件不存在时是无操作
    let _ = path_helper::delete(file);
    let _ = path_helper::delete(format!("{}{}", file.display(), names::DISABLE_DOT_EXT));
    let _ = path_helper::delete(format!("{}{}", file.display(), names::DISABLED_DOT_EXT));
}
