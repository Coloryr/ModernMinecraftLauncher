//! 实例级存档操作：存档列表、备份信息、还原备份与导入存档

use super::*;

impl InstanceSettingObj {
    /// 获取实例存档列表
    ///
    /// # 返回值
    ///
    /// 返回存档列表；读取失败的存档被跳过
    pub async fn get_saves(&self) -> Vec<SaveObj> {
        let dir = self.get_saves_path();
        let dirs = path_helper::get_dirs(dir);

        tokio::task::spawn_blocking(move || {
            let list = Mutex::new(Vec::new());

            dirs.par_iter().for_each(|item| {
                let file = item.join(names::LEVEL_FILE);
                if file.exists() && file.is_file() {
                    if let Ok(mut stream) = path_helper::open_read(&file) {
                        let data = read_save(&mut stream);
                        match data {
                            Ok(mut obj) => {
                                let file = item.join(names::ICON_FILE);
                                if file.exists() {
                                    obj.icon = Some(file);
                                }

                                obj.path = item.clone();
                                list.lock().unwrap().push(obj);
                            }
                            Err(err) => {
                                mml_log::error_type(err);
                            }
                        }
                    }
                }
            });

            list.into_inner().unwrap()
        })
        .await
        .unwrap_or_default()
    }

    /// 还原备份，靠 `info.dir` 定位是哪个存档
    ///
    /// 备份 zip 在备份目录里，还原目标却是存档目录 `saves/<info.dir>`；
    /// 这里必须显式拼出目标路径，否则只会在备份目录里再展开一份，真正的存档毫无变化。
    ///
    /// - `info`: 备份信息，`dir` 是存档目录名
    /// - `file`: 备份文件名，来自 [`get_backups`](Self::get_backups) 的 `back`
    /// - `gui`: 压缩进度回调
    ///
    /// # 返回值
    ///
    /// 现有存档移入回收站或解压失败时返回对应错误
    pub fn restore_backup(
        &self,
        info: &SaveBackupObj,
        file: &str,
        gui: BaseArchiveGui,
    ) -> CoreResult<()> {
        let backup_file = self.get_backup_path().join(file);
        let target = self.get_saves_path().join(&info.dir);

        // 先把现有存档整目录移进回收站而不是直接删：还原是破坏性操作，用户选错了还能捞回来
        if target.exists() && target.is_dir() {
            path_helper::move_to_trash(&target)?;
        }

        BaseArchive::decompress(ArchiveType::Zip, &backup_file, &target, gui)
    }

    /// 获取备份文件列表
    ///
    /// # 返回值
    ///
    /// 返回存档名到备份信息的映射；文件不存在返回空表，读取失败返回对应错误
    pub fn get_backups(&self) -> CoreResult<HashMap<String, SaveBackupObj>> {
        let file = self.get_backup_file();
        if file.exists() && file.is_file() {
            let stream = path_helper::open_read(&file)?;
            serialize_tools::json_from_stream(&stream)
        } else {
            Ok(HashMap::new())
        }
    }

    /// 保存备份信息
    ///
    /// - `info`: 存档名到备份信息的映射
    pub fn save_backups(&self, info: &HashMap<String, SaveBackupObj>) {
        let file = self.get_backup_file();
        config_save::save(uuids::mix_uuid(uuids::BACKUP_UUID, self.uuid), info, &file);
    }

    /// 导入存档
    ///
    /// - `file`: 存档压缩包路径
    ///
    /// # 返回值
    ///
    /// 打开或解压失败时返回对应错误
    pub fn import_save<P: AsRef<Path>>(&self, file: P) -> CoreResult<()> {
        let saves_dir = self.get_saves_path();
        if !saves_dir.exists() {
            path_helper::create_dir_all(&saves_dir)?;
        }

        let archive = BaseArchive::open_readonly(file.as_ref())?;

        // 如果包内所有条目都套在同一个顶层文件夹里，直接解压到 saves 目录；
        // 否则以压缩包文件名新建一个文件夹再解压进去。
        let output_dir = if archive.single_top_dir().is_some() {
            // 有唯一顶层文件夹：直接解压，顶层文件夹就是存档目录
            saves_dir
        } else {
            // 没有套壳：用压缩包文件名去掉后缀建一个新目录
            let file_path = file.as_ref();
            let stem = file_path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "save".to_string());
            // 二次去后缀，处理 .tar.gz 这类双后缀
            let stem = stem.trim_end_matches(".tar").to_string();
            saves_dir.join(&stem)
        };

        path_helper::create_dir_all(&output_dir)?;
        archive.extract_all(&output_dir, None, None, None)
    }
}
