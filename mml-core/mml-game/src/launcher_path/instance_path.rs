//! 实例目录与实例内文件路径

use std::{
    collections::HashMap,
    fs::{self},
    path::{Path, PathBuf},
    sync::OnceLock,
};

use mml_base::serialize_tools;
use mml_config::config_save;
use mml_names::{
    i18_items::error_type::{CoreResult, ErrorType, FileSystemErrorData},
    names, uuids,
};
use mml_sys::path_helper;

use crate::launcher::{
    custom_game_arg_obj::CustomGameArgObj, file_online_info_obj::OnlineInfoObj,
    game_time_obj::GameTimeObj, instance_setting_obj::InstanceSettingObj,
};

/// 在线文件信息表，键为 mod_id
pub type OnlineInfoList = HashMap<String, OnlineInfoObj>;
/// 自定义启动配置表，键为文件名
pub type CustomGameArgList = HashMap<String, CustomGameArgObj>;

/// 实例根目录
static BASE_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 初始化实例路径
///
/// - `dir`: 运行路径
///
/// # 返回值
///
/// 创建目录失败时返回对应错误
pub(crate) fn init<P: AsRef<Path>>(dir: P) -> CoreResult<()> {
    let dir = BASE_DIR.get_or_init(|| dir.as_ref().join(names::INSTANCE_DIR));

    if !dir.exists() {
        path_helper::create_dir_all(&dir)?;
    }

    Ok(())
}

/// 获取实例目录
///
/// # 返回值
///
/// 返回实例根目录；未初始化会 panic
pub fn get_instance_dir() -> PathBuf {
    BASE_DIR.get().unwrap().clone()
}

/// 读取所有实例
///
/// # 返回值
///
/// 返回实例列表；读取目录失败返回对应错误
pub(crate) fn load_instance_dir() -> CoreResult<Vec<InstanceSettingObj>> {
    let dir = BASE_DIR.get().unwrap();
    let dirs = fs::read_dir(dir).map_err(|err| {
        ErrorType::FileSystemError(FileSystemErrorData {
            path: dir.clone(),
            error: err.to_string(),
        })
    })?;

    let mut list = Vec::new();

    for item in dirs {
        if let Ok(dir) = item
            && let Some(instance) = load_instance(dir.path())
        {
            list.push(instance);
        }
    }

    Ok(list)
}

/// 从文件夹路径加载实例
///
/// - `dir`: 实例目录
///
/// # 返回值
///
/// 目录或配置文件不存在时返回 `None`
pub(crate) fn load_instance<P: AsRef<Path>>(dir: P) -> Option<InstanceSettingObj> {
    let file = dir.as_ref();
    if !file.is_dir() {
        return None;
    }

    let config = file.join(names::GAME_FILE);
    if !config.exists() || !config.is_file() {
        return None;
    }

    if let Ok(mut obj) = serialize_tools::json_from_file::<InstanceSettingObj>(&config) {
        let path = file.file_name().unwrap_or_default();
        let path = path.to_string_lossy();
        if !path.eq(&obj.dir) {
            obj.dir = path.to_string();
            obj.save();
        }

        Some(obj)
    } else {
        None
    }
}

impl InstanceSettingObj {
    /// 保存
    pub fn save(&self) {
        config_save::save(self.uuid, self, &self.get_json_file());
    }

    /// 获取基础路径
    pub fn get_base_path(&self) -> PathBuf {
        BASE_DIR.get().unwrap().join(&self.dir)
    }

    /// 获取存档备份路径
    pub fn get_backup_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::BACKUP_DIR)
    }

    /// 获取临时文件路径
    pub fn get_temp_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::TEMP_DIR)
    }

    /// 获取游戏缓存路径
    pub fn get_cache_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::CACHE_DIR)
    }

    /// 获取自定义启动配置目录
    pub fn get_json_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::JSON_DIR)
    }

    /// 获取自定义加载器运行库路径
    pub fn get_libraries_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::LIBRARIES_DIR)
    }

    /// 获取游戏路径 .minecraft
    pub fn get_game_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
    }

    /// 获取截图路径
    pub fn get_screenshots_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
            .join(names::GAME_SCREENSHOTS_DIR)
    }

    /// 获取资源包路径
    pub fn get_resourcepacks_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
            .join(names::GAME_RESOURCEPACKS_DIR)
    }

    /// 获取光影包路径
    pub fn get_shaderpacks_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
            .join(names::GAME_SHADERPACKS_DIR)
    }

    /// 获取模组路径
    pub fn get_mods_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
            .join(names::GAME_MODS_DIR)
    }

    /// 获取存档路径
    pub fn get_saves_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
            .join(names::GAME_SAVES_DIR)
    }

    /// 获取结构文件路径
    pub fn get_schematics_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
            .join(names::GAME_SCHEMATICS_DIR)
    }

    /// 获取配置文件路径
    pub fn get_config_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
            .join(names::GAME_CONFIG_DIR)
    }

    /// 获取日志路径
    pub fn get_logs_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
            .join(names::GAME_LOGS_DIR)
    }

    /// 获取崩溃日志路径
    pub fn get_crash_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
            .join(names::GAME_CRASH_DIR)
    }

    /// kubejs路径
    pub fn get_kubejs_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
            .join(names::KUBEJS_DIR)
    }

    /// local路径
    pub fn get_local_path(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
            .join(names::LOCAL_DIR)
    }

    /// 获取储存文件
    pub fn get_json_file(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_FILE)
    }

    /// 获取实例 GUI 设置文件 gui_setting.json，与 game.json 并列
    ///
    /// **归属 GUI**：实例 GUI 设置，含日志窗口、模组分组、方块背景、自开日志窗与
    /// 排列顺序，是界面状态，内核不读也不写它，读写实现在
    /// `mml-gui/src-tauri/src/gui_setting.rs`。这里只提供路径：
    /// "实例目录在哪"是内核的知识，"目录里放什么 GUI 文件"不是。
    pub fn get_gui_setting_file(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GUI_SETTING_FILE)
    }

    /// 获取图标文件，兼容老数据：`Icon` 为空时回退到默认的 `icon.png`
    ///
    /// 给"显示图标"用。配置里 `Icon` 就是个空字段，区分不出"没配"与"配了空值"，
    /// 所以这里一律给默认名，由调用方判断文件是否存在。
    pub fn get_icon_file_or_default(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(self.icon.as_deref().unwrap_or(names::ICON_FILE))
    }

    /// 获取存档备份信息文件
    pub fn get_backup_file(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::SAVE_BACKUP_FILE)
    }

    /// 获取在线文件信息文件
    pub fn get_online_info_file(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::MOD_INFO_FILE)
    }

    /// 获取服务器实例文件
    pub fn get_server_pack_file(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::SERVER_FILE)
    }

    /// 获取旧服务器实例文件
    pub fn get_server_pack_old_file(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::SERVER_OLD_FILE)
    }

    /// 获取启动记录数据文件
    pub fn get_launch_file(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::LAUNCH_COUNT_FILE)
    }

    /// 获取安全Log4j文件
    pub fn get_log4j_file(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::LOG4J_FILE)
    }

    /// 获取自定义加载器文件
    pub fn get_loader_file(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::LOADER_FILE)
    }

    /// 获取游戏配置文件
    pub fn get_option_file(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
            .join(names::OPTION_FILE)
    }

    /// 获取服务器储存文件
    pub fn get_servers_file(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
            .join(names::GAME_SERVER_FILE)
    }

    /// 读取在线文件信息
    ///
    /// # 返回值
    ///
    /// 返回在线文件信息表；文件不存在或解析失败返回空表
    pub fn read_online_info(&self) -> OnlineInfoList {
        let file = self.get_online_info_file();
        if file.exists() && file.is_file() {
            let json = serialize_tools::json_from_file::<OnlineInfoList>(&file);
            if let Ok(data) = json {
                return data;
            }
        }

        HashMap::new()
    }

    /// 保存在线文件信息
    ///
    /// - `info`: 在线文件信息表
    pub fn save_online_info(&self, info: &OnlineInfoList) {
        config_save::save(
            uuids::mix_uuid(self.uuid, uuids::ONLINE_FILE_UUID),
            info,
            &self.get_online_info_file(),
        );
    }

    /// 读取自定义游戏启动配置
    ///
    /// # 返回值
    ///
    /// 返回配置表；目录不存在时返回空表
    pub fn read_custom_json(&self) -> CustomGameArgList {
        let file = self.get_json_path();
        let mut list = HashMap::new();
        if file.exists()
            && file.is_dir()
            && let Ok(dir) = fs::read_dir(&file)
        {
            for item in dir {
                if let Ok(item) = item {
                    let json = serialize_tools::json_from_file::<CustomGameArgObj>(&item.path());
                    if let Ok(data) = json {
                        list.insert(item.file_name().to_string_lossy().to_string(), data);
                    }
                }
            }
        }

        list
    }

    /// 保存自定义启动配置
    ///
    /// - `info`: 配置表，键为文件名
    ///
    /// # 返回值
    ///
    /// 写入失败时返回对应错误
    pub fn save_custom_json(&self, info: &CustomGameArgList) -> CoreResult<()> {
        let dir = self.get_json_path();
        path_helper::create_dir_all(&dir)?;
        for (key, value) in info.iter() {
            serialize_tools::json_to_file(value, dir.join(key))?;
        }

        Ok(())
    }

    /// 读取启动统计数据
    ///
    /// # 返回值
    ///
    /// 返回统计数据；文件不存在或解析失败返回默认值
    pub fn read_launch_count_data(&self) -> GameTimeObj {
        let file = self.get_launch_file();
        if file.exists() && file.is_file() {
            let obj = serialize_tools::json_from_file::<GameTimeObj>(&file);
            if let Ok(obj) = obj {
                return obj;
            }
        }

        Default::default()
    }

    /// 保存启动统计数据
    ///
    /// - `obj`: 统计数据
    pub fn save_launch_count_data(&self, obj: &GameTimeObj) {
        config_save::save(
            uuids::mix_uuid(self.uuid, uuids::LAUNCH_COUNT_DATA_FILE_UUID),
            obj,
            &self.get_launch_file(),
        );
    }
}
