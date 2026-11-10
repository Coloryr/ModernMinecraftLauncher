//! 存档的基本操作：路径、删除、导出、备份，以及 level.dat 的写回

use super::*;

impl SaveObj {
    /// 获取数据包文件夹
    ///
    /// # 返回值
    ///
    /// 返回数据包目录路径
    pub fn get_datapack_path(&self) -> PathBuf {
        self.path.join(names::GAME_DATAPACK_DIR)
    }

    /// 获取存档信息文件
    ///
    /// # 返回值
    ///
    /// 返回 level.dat 文件路径
    pub fn get_level_file(&self) -> PathBuf {
        self.path.join(names::LEVEL_FILE)
    }

    /// 删除存档
    ///
    /// # 返回值
    ///
    /// 移入回收站失败时返回对应错误
    pub fn delete(&self) -> CoreResult<()> {
        path_helper::move_to_trash(&self.path)
    }

    /// 导出存档
    ///
    /// - `path`: 导出保存位置
    /// - `archive_type`: 压缩类型
    /// - `gui`: 压缩进度回调
    ///
    /// # 返回值
    ///
    /// 压缩失败时返回对应错误
    pub fn export<P: AsRef<Path>>(
        &self,
        path: P,
        archive_type: ArchiveType,
        gui: BaseArchiveGui,
    ) -> CoreResult<()> {
        BaseArchive::compress(
            archive_type,
            path.as_ref(),
            self.path.as_ref(),
            None::<&Path>,
            &None,
            gui,
        )?;
        Ok(())
    }

    /// 备份存档
    ///
    /// - `instance`: 所属实例
    /// - `gui`: 压缩进度回调
    ///
    /// # 返回值
    ///
    /// 返回备份文件名，形如 `level_name_年_月_日_时_分_秒.zip`；
    /// 压缩或保存备份信息失败时返回对应错误
    pub fn backup(&self, instance: &InstanceSettingObj, gui: BaseArchiveGui) -> CoreResult<String> {
        let path = instance.get_backup_path();
        path_helper::create_dir_all(&path)?;

        let time = Local::now();
        let name = format!(
            "{}_{}_{}_{}_{}_{}_{}.zip",
            self.level_name,
            time.year(),
            time.month(),
            time.day(),
            time.hour(),
            time.minute(),
            time.second()
        );
        let file = path.join(&name);

        BaseArchive::compress(ArchiveType::Zip, &file, &self.path, None, &None, gui)?;

        let mut info = instance.get_backups()?;
        if let Some(obj) = info.get_mut(&self.level_name) {
            obj.back.push(name.clone());
        } else {
            let path = instance.get_saves_path();
            let dir = self
                .path
                .strip_prefix(path)
                .map_err(|err| {
                    ErrorType::FileSystemError(FileSystemErrorData {
                        path: self.path.clone(),
                        error: err.to_string(),
                    })
                })?
                .to_string_lossy()
                .to_string();
            let back: Vec<String> = vec![name.clone()];
            info.insert(self.level_name.clone(), SaveBackupObj { dir, back });
        }

        instance.save_backups(&info);

        Ok(name)
    }

    /// 保存 NBT 数据到文件
    ///
    /// # 返回值
    ///
    /// 写入失败时返回对应错误
    pub(crate) fn save_nbt(&self) -> CoreResult<()> {
        let file = self.get_level_file();
        let mut stream = path_helper::open_write(file)?;

        self.nbt.write(&mut stream)?;

        Ok(())
    }

    /// 改存档的基本信息，写回 level.dat
    ///
    /// 只动 `Data` 下的 `GameType`、`Difficulty`、`hardcore`、`LevelName` 与种子，其余字段
    /// 原样写回；缺的字段按 level.dat 的标准类型补上，否则游戏读不到、等于白改。
    ///
    /// 种子写两份：`RandomSeed` 给旧版本，`WorldGenSettings.seed` 给 1.16 起的新版本 ——
    /// 新版本以后者为准，只写前者不生效；没有这个复合标签的老存档就只写前者。
    ///
    /// 调用前要确认实例没在运行 —— 游戏退出时会用它内存里的值覆盖这个文件。
    ///
    /// - `info`: 新的信息，见 [`SaveEditObj`]
    ///
    /// # 返回值
    ///
    /// NBT 里没有 `Data` 复合标签或写盘失败时返回对应错误
    pub fn set_info(&mut self, info: &SaveEditObj) -> CoreResult<()> {
        let Some(data) = self
            .nbt
            .nbt
            .as_compound_mut()
            .and_then(|nbt| nbt.get_compound_mut("Data"))
        else {
            return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
        };

        data.data.insert(
            String::from("GameType"),
            NbtInt::new(info.game_type).to_nbt(),
        );
        data.data.insert(
            String::from("Difficulty"),
            NbtByte::new(info.difficulty).to_nbt(),
        );
        // 键名是小写的 `hardcore`，与读取处一致
        data.data.insert(
            String::from("hardcore"),
            NbtByte::new(if info.hard_core { 1 } else { 0 }).to_nbt(),
        );
        data.data.insert(
            String::from("LevelName"),
            NbtString::new(info.level_name.clone()).to_nbt(),
        );
        data.data
            .insert(String::from("RandomSeed"), NbtLong::new(info.seed).to_nbt());
        if let Some(world_gen) = data.get_compound_mut("WorldGenSettings") {
            world_gen
                .data
                .insert(String::from("seed"), NbtLong::new(info.seed).to_nbt());
        }

        self.game_type = info.game_type;
        self.difficulty = info.difficulty;
        self.hard_core = if info.hard_core { 1 } else { 0 };
        self.level_name = info.level_name.clone();
        self.random_seed = info.seed;

        self.save_nbt()
    }
}
