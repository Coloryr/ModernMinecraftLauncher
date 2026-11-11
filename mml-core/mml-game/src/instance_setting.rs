use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use mml_names::{
    i18_items::error_type::{ArgEmptyData, CoreResult, ErrorType, FileSystemErrorData},
    names,
};

use mml_net::{get_work_client, input_file::InputFile};
use mml_sys::path_helper;
use uuid::Uuid;

use crate::{
    gui_hook::{AddInstanceGui, ProgressGui},
    launcher::instance_setting_obj::InstanceSettingObj,
};

use crate::instance_edit::icon_extension;
use crate::*;

impl InstanceSettingObj {
    /// 创建实例，落在默认分组
    ///
    /// 重名实例经 GUI 询问覆盖或改名；创建实例目录结构并保存设置。
    ///
    /// - `gui`: 添加实例界面回调；无界面时重名实例自动改名
    ///
    /// # 返回值
    ///
    /// 返回新实例句柄；名字为空或用户取消返回对应错误
    pub async fn create_instance(self, gui: AddInstanceGui) -> CoreResult<GameInstance> {
        self.create_instance_in_group(gui, None).await
    }

    /// 创建实例并指定分组
    ///
    /// 与 [`create_instance`](Self::create_instance) 只差分组：分组归属不在实例配置里，
    /// 见 [`crate::game_group`]，只能在登记时一并指定，所以导入或安装整合包这类
    /// "用户已经选了目标分组"的流程走这里。
    ///
    /// - `gui`: 添加实例界面回调；无界面时重名实例自动改名
    /// - `group`: 目标分组 uuid；`None` 表示默认分组，不存在或已删的组也会落到默认分组
    ///
    /// # 返回值
    ///
    /// 返回新实例句柄；名字为空或用户取消返回对应错误
    pub async fn create_instance_in_group(
        mut self,
        gui: AddInstanceGui,
        group: Option<Uuid>,
    ) -> CoreResult<GameInstance> {
        let _watch = path_watch::WatchPause::new();

        // 首尾空白不算名字的一部分，落盘与目录名都用去掉空白后的值
        self.name = self.name.trim().to_string();

        let old = get_instance_by_name(&self.name);
        // 是否覆盖重名实例；无界面或用户拒绝覆盖时改为自动改名，保留原实例
        let mut overwrite = false;
        if let Some(instance) = &old {
            if let Some(gui) = &gui {
                overwrite = gui.overwrite(instance.clone()).await;
                if !overwrite && !gui.name_replace(&self.name).await {
                    return Err(ErrorType::TaskCancel);
                }
            }

            if !overwrite {
                let mut a = 1;
                let mut name = format!("{}{a}", self.name);
                while have_instance_name(&name) {
                    a += 1;
                    name = format!("{}{a}", self.name);
                }

                self.name = name;
            }
        }

        if self.name.is_empty() {
            return Err(ErrorType::ArgEmpty(ArgEmptyData::Name));
        }

        // 只在用户选择覆盖时才删除重名的原实例
        if overwrite {
            if let Some(instance) = old {
                let uuid = {
                    let r = instance.read().unwrap();
                    r.uuid
                };

                delete_instance(&uuid)?;
            }
        }

        self.dir = path_helper::replace_path_name(&self.name);

        let dir = self.get_base_path();
        if dir.exists() {
            path_helper::move_to_trash(&dir)?
        }

        path_helper::create_dir_all(dir)?;
        path_helper::create_dir_all(self.get_game_path())?;
        path_helper::create_dir_all(self.get_mods_path())?;
        path_helper::create_dir_all(self.get_config_path())?;
        path_helper::create_dir_all(self.get_logs_path())?;
        path_helper::create_dir_all(self.get_saves_path())?;
        path_helper::create_dir_all(self.get_resourcepacks_path())?;

        self.save_online_info(&HashMap::new());

        self.save();

        // 分组归属不在实例配置里，落在独立的 group_save.json，`None` 即默认分组
        Ok(add_to_group(self, group))
    }

    /// 删除实例文件
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`，实例目录移入回收站；失败返回对应错误
    pub fn delete_files(&self) -> CoreResult<()> {
        path_helper::move_to_trash(self.get_base_path())
    }

    /// 复制实例设置为新的对象
    ///
    /// `InstanceSettingObj` 刻意不实现 `Clone`，避免被随意整对象复制；
    /// 仅在确有需要构建一个独立新对象时显式调用本方法，如复制实例。
    ///
    /// # 返回值
    ///
    /// 返回字段逐项克隆出的新对象，uuid 仍是原实例的
    fn copy_self(&self) -> Self {
        Self {
            uuid: self.uuid,
            name: self.name.clone(),
            dir: self.dir.clone(),
            version: self.version.clone(),
            loader: self.loader,
            loader_version: self.loader_version.clone(),
            jvm_arg: self.jvm_arg.clone(),
            jvm_name: self.jvm_name.clone(),
            jvm_local: self.jvm_local.clone(),
            window: self.window.clone(),
            start_server: self.start_server.clone(),
            auto_save: self.auto_save.clone(),
            proxy_host: self.proxy_host.clone(),
            advance_jvm: self.advance_jvm.clone(),
            is_modpack: self.is_modpack,
            modpack_type: self.modpack_type,
            game_type: self.game_type,
            pid: self.pid.clone(),
            fid: self.fid.clone(),
            icon: self.icon.clone(),
            server_url: self.server_url.clone(),
            custom_loader: self.custom_loader.clone(),
            encoding: self.encoding,
        }
    }

    /// 复制数据到新的实例
    ///
    /// - `name`: 新的实例名字
    /// - `gui`: 添加实例界面回调
    ///
    /// # 返回值
    ///
    /// 返回新实例句柄；新实例与原实例同组
    pub async fn copy_to_other(&self, name: &str, gui: AddInstanceGui) -> CoreResult<GameInstance> {
        // 复制品与原实例同组：分组不在实例配置里，得单独问一次
        let group = get_instance_group(&self.uuid);

        let mut instance = self.copy_self();
        instance.name = name.to_string();
        let instance = instance.create_instance_in_group(gui, group).await?;

        let online = self.read_online_info();
        let custom = self.read_custom_json();

        let read = instance.read().unwrap();
        read.save_custom_json(&custom)?;
        read.save_online_info(&online);

        Ok(instance.clone())
    }

    /// 更新在线文件信息
    ///
    /// 清理掉磁盘上已不存在的文件记录；被禁用而追加了 `.disabled` / `.disable` 后缀的视作仍存在。
    pub async fn update_online(&self) {
        let mut online = self.read_online_info();
        let dir = self.get_game_path();
        online.retain(|_, value| {
            let file = dir.join(&value.path).join(&value.file);
            if file.exists() {
                false
            } else {
                // 新后缀是 `.disabled`，旧版留下的 `.disable` 同样算还存在
                for suffix in [names::DISABLED_DOT_EXT, names::DISABLE_DOT_EXT] {
                    let disabled_file = PathBuf::from(format!("{}{}", file.display(), suffix));
                    if disabled_file.exists() {
                        return false;
                    }
                }
                true
            }
        });

        self.save_online_info(&online);
    }

    /// 将文件复制到其他地方
    ///
    /// - `path`: 目标目录
    /// - `skip`: 需要跳过的相对路径列表，文件或目录均可；`None` 表示一个都不跳过
    /// - `is_base`: `true` 复制实例整个目录，`false` 只复制游戏目录
    /// - `gui`: 进度界面回调
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；复制失败返回对应错误
    pub async fn copy_files<P: AsRef<Path>>(
        &self,
        path: P,
        skip: Option<Vec<PathBuf>>,
        is_base: bool,
        gui: ProgressGui,
    ) -> CoreResult<()> {
        let dir = if is_base {
            self.get_base_path()
        } else {
            self.get_game_path()
        };

        path_helper::create_dir_all(path.as_ref())?;

        let mut index = 0usize;
        let list = path_helper::get_all_files(&dir);
        if let Some(gui) = &gui {
            gui.set_progress_now(index, Some(list.len()));
        }
        for item in list.iter() {
            let file = item.strip_prefix(&dir).map_err(|err| {
                ErrorType::FileSystemError(FileSystemErrorData {
                    path: item.clone(),
                    error: err.to_string(),
                })
            })?;

            if let Some(skip) = &skip {
                // 目录项按组件前缀跳过：assets / libraries 传进来就整棵跳过
                if skip
                    .iter()
                    .any(|item| file == item || file.starts_with(item))
                {
                    index += 1;
                    if let Some(gui) = &gui {
                        gui.set_progress_now(index, Some(list.len()));
                    }
                    continue;
                }
            }

            if let Some(gui) = &gui {
                gui.set_progress_text(Some(item.display().to_string()));
            }

            let now = path.as_ref().join(file);
            path_helper::copy_file_async(item, &now).await?;
        }

        Ok(())
    }

    /// 写入实例图标文件，并把实际写进去的文件名记回实例
    ///
    /// 故意不做成 `&mut self` 方法：那样调用方就得在 `.await` 期间持有实例写锁，
    /// 而实例锁的守卫不是 `Send`，在 `#[async_trait]` 里编译不过，它要求 `future: Send`。
    /// 这里自己控制加锁范围，只在锁内问出"图标写到哪个文件"，写盘时不持锁。
    ///
    /// 调用方也不要带着实例锁进来，`std` 的 `RwLock` 不可重入，会死锁。
    ///
    /// - `game`: 目标实例
    /// - `icon`: 图标输入源
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；写盘失败返回对应错误
    pub async fn save_icon(game: &GameInstance, icon: InputFile) -> CoreResult<()> {
        // 在线整合包图标先读字节判格式：GIF 必须保留 .gif，不能固定写成 icon.png 再转静态图
        if let InputFile::Url(url) = &icon {
            let data = get_work_client().get_bytes(url).await?;
            let ext = icon_extension(&data);
            let file = game
                .read()
                .unwrap()
                .get_base_path()
                .join(format!("icon.{ext}"));
            path_helper::write_bytes_async(&file, &data).await?;
            let name = file
                .file_name()
                .map(|item| item.to_string_lossy().into_owned());
            let mut game = game.write().unwrap();
            game.icon = name;
            game.save();
            return Ok(());
        }

        let (file, name) = {
            let game = game.read().unwrap();
            // 上传图片 = 图标二选一里的"图片"那一支：写默认的 icon.png
            let file = game.get_icon_file_or_default();
            let name = file
                .file_name()
                .map(|item| item.to_string_lossy().into_owned());
            (file, name)
        };

        icon.save_file(file).await?;

        let mut game = game.write().unwrap();
        game.icon = name;
        game.save();

        Ok(())
    }
}
