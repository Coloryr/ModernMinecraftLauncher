use super::*;

impl InstanceSettingObj {
    /// 保存
    pub fn save(&self) {
        config_save::save(self.uuid, self, &self.get_json_file());
    }

    /// 获取基础路径
    ///
    /// # 返回值
    ///
    /// 实例根目录的绝对路径
    pub fn get_base_path(&self) -> PathBuf {
        BASE_DIR.get().unwrap().join(&self.dir)
    }

    /// 获取存档备份路径
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_backup_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::BACKUP_DIR)
    }

    /// 获取临时文件路径
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_temp_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::TEMP_DIR)
    }

    /// 获取游戏缓存路径
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_cache_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::CACHE_DIR)
    }

    /// 获取自定义启动配置目录
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_json_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::JSON_DIR)
    }

    /// 获取自定义加载器运行库路径
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_libraries_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::LIBRARIES_DIR)
    }

    /// 获取游戏路径 .minecraft
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_game_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
    }

    /// 获取截图路径
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_screenshots_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
            .join(names::GAME_SCREENSHOTS_DIR)
    }

    /// 获取资源包路径
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_resourcepacks_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
            .join(names::GAME_RESOURCEPACKS_DIR)
    }

    /// 获取光影包路径
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_shaderpacks_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
            .join(names::GAME_SHADERPACKS_DIR)
    }

    /// 获取模组路径
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_mods_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
            .join(names::GAME_MODS_DIR)
    }

    /// 获取存档路径
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_saves_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
            .join(names::GAME_SAVES_DIR)
    }

    /// 获取结构文件路径
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_schematics_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
            .join(names::GAME_SCHEMATICS_DIR)
    }

    /// 获取配置文件路径
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_config_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
            .join(names::GAME_CONFIG_DIR)
    }

    /// 获取日志路径
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_logs_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
            .join(names::GAME_LOGS_DIR)
    }

    /// 获取崩溃日志路径
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_crash_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
            .join(names::GAME_CRASH_DIR)
    }

    /// kubejs路径
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_kubejs_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
            .join(names::KUBEJS_DIR)
    }

    /// local路径
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_local_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
            .join(names::LOCAL_DIR)
    }

    /// 获取储存文件
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_json_file(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_FILE)
    }
}
