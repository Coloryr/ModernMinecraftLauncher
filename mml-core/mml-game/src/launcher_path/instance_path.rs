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
    i18_items::{
        error_type::{CoreResult, ErrorType, FileSystemErrorData},
        info_type::InfoType,
    },
    names, uuids,
};
use mml_sys::path_helper;

use crate::launcher::{
    custom_game_arg_obj::CustomGameArgObj,
    file_online_info_obj::OnlineInfoObj,
    instance_setting_obj::CustomLoaderObj,
    instance_setting_obj::InstanceSettingObj,
    mod_update_obj::{ModUpdateList, ModUpdateRecordObj},
};
use crate::other_launcher::{is_mmc_version, mmc_obj::MMCObj};

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
            // 没有 game.json 时再试 MMC：Labymod 这类工具会直接按 MMC 格式在实例目录下建目录
            && let Some(instance) = load_instance(dir.path()).or_else(|| adopt_mmc(&dir.path()))
        {
            list.push(instance);
        }
    }

    Ok(list)
}

/// 把标准的 MMC 实例目录接管成 mml 实例
///
/// Labymod 之类的工具会直接按 MMC 格式在 `instances/` 下建目录（`mmc-pack.json` + `instance.cfg`），
/// 这种目录没有 `game.json`，正常加载会整个跳过 —— 用户看到的现象就是"实例没被识别"。
/// 这里按 MMC 元数据补出实例配置并落盘：**目录原地保留、文件不动**，只多一个 `game.json`。
///
/// - `dir`: 实例目录
///
/// # 返回值

///
/// 补好配置的实例；不是 MMC 实例或元数据读不出来时返回 `None`
/// 目录里有没有 MMC 补丁，即 `patches/*.json`
///
/// - `dir`: 实例目录
///
/// # 返回值
///
/// 至少有一个补丁文件返回 `true`
fn has_patches(dir: &Path) -> bool {
    fs::read_dir(dir.join(names::JSON_DIR))
        .map(|items| {
            items.flatten().any(|item| {
                let path = item.path();
                path.is_file() && path.extension().is_some_and(|ext| ext == names::JSON_EXT)
            })
        })
        .unwrap_or(false)
}

/// 把 MMC 实例目录读成 M²L 的实例配置
///
/// - `dir`: 实例目录
///
/// # 返回值
///
/// 不是 MMC 版本目录或配置读不出来时返回 `None`
fn adopt_mmc(dir: &Path) -> Option<InstanceSettingObj> {
    if !is_mmc_version(dir) {
        return None;
    }

    let mmc = serialize_tools::json_from_file::<MMCObj>(dir.join(names::MMCJSON_FILE)).ok()?;
    let cfg = crate::game_options::read_options_from_file(dir.join(names::MMCCFG_FILE), Some('='))
        .ok()?;

    let mut instance = mmc.to_instance(cfg);
    let name = dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    instance.dir = name.clone();
    if instance.name.is_empty() {
        instance.name = name;
    }
    // 目录里有 patches/*.json 就按 MMC 模式启动：补丁是 MMC 改启动配置的方式
    // （Labymod 就靠它注入 launchwrapper + LabyMod-4.jar）。
    // to_instance 只在碰到未知组件时才打开这个开关，而 Labymod 的 components 只有 net.minecraft，
    // 不补这一步的话 patches 会被忽略、实例退化成原版
    if has_patches(dir) {
        instance.custom_loader = Some(CustomLoaderObj {
            custom_json: true,
            ..Default::default()
        });
    }

    instance.save();
    mml_log::info_type(InfoType::AdoptMmcInstance {
        name: instance.name.clone(),
    });
    Some(instance)
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

    /// 获取实例 GUI 设置文件 gui_setting.json，与 game.json 并列
    ///
    /// **归属 GUI**：实例 GUI 设置，含日志窗口、模组分组、方块背景、自开日志窗与
    /// 排列顺序，是界面状态，内核不读也不写它，读写实现在
    /// `mml-gui/src-tauri/src/gui_setting.rs`。这里只提供路径：
    /// "实例目录在哪"是内核的知识，"目录里放什么 GUI 文件"不是。
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
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
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_icon_file_or_default(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(self.icon.as_deref().unwrap_or(names::ICON_FILE))
    }

    /// 获取存档备份信息文件
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_backup_file(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::SAVE_BACKUP_FILE)
    }

    /// 获取在线文件信息文件
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_online_info_file(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::MOD_INFO_FILE)
    }

    /// 获取服务器实例文件
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_server_pack_file(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::SERVER_FILE)
    }

    /// 获取旧服务器实例文件
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_server_pack_old_file(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::SERVER_OLD_FILE)
    }

    /// 获取模组更新记录文件
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_mod_update_file(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::MOD_UPDATE_FILE)
    }

    /// 获取安全Log4j文件
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_log4j_file(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::LOG4J_FILE)
    }

    /// 获取自定义加载器文件
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_loader_file(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::LOADER_FILE)
    }

    /// 获取游戏配置文件
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
    pub fn get_option_file(&self) -> PathBuf {
        BASE_DIR
            .get()
            .unwrap()
            .join(&self.dir)
            .join(names::GAME_DIR)
            .join(names::OPTION_FILE)
    }

    /// 获取服务器储存文件
    ///
    /// # 返回值
    ///
    /// 实例目录下的绝对路径，不检查是否存在
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

    /// 同步保存在线文件信息
    ///
    /// 整合包安装 / 升级紧接着要读它来准备资源映射，异步队列可能还没落盘。
    ///
    /// - `info`: 在线文件信息表
    ///
    /// # 返回值
    ///
    /// 写盘失败时返回对应错误
    pub fn save_online_info_sync(&self, info: &OnlineInfoList) -> CoreResult<()> {
        serialize_tools::json_to_file(info, self.get_online_info_file())
    }

    /// 删除实例内文件对应的在线信息
    ///
    /// 资源窗口删除模组 / 材质包时调用，否则启动前检查会按在线信息把文件补回来。
    ///
    /// - `target`: 已删除的实例内文件，正常文件名
    ///
    /// # 返回值
    ///
    /// 写盘失败时返回对应错误
    pub fn remove_online_info(&self, target: &Path) -> CoreResult<()> {
        let game_path = self.get_game_path();
        let Ok(relative) = target.strip_prefix(&game_path) else {
            return Ok(());
        };
        let Some(file) = relative.file_name().and_then(|name| name.to_str()) else {
            return Ok(());
        };
        let dir = relative
            .parent()
            .map(|parent| parent.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();

        let mut info = self.read_online_info();
        info.retain(|_, item| !(item.path == dir && item.file == file));
        self.save_online_info_sync(&info)
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

    /// 读取模组更新记录
    ///
    /// # 返回值
    ///
    /// 项目编号到该项目更新历史的表；文件不存在或解析失败返回空表
    ///
    /// 早先根是数组，改成按项目编号分表之后旧文件解析失败即当空表，
    /// 下一次追加会把整个文件按新格式重写；日志级别的东西不值当写迁移
    pub fn read_mod_update(&self) -> ModUpdateList {
        let file = self.get_mod_update_file();
        if file.exists() && file.is_file() {
            if let Ok(list) = serialize_tools::json_from_file::<ModUpdateList>(&file) {
                return list;
            }
        }

        HashMap::new()
    }

    /// 追加一条模组更新记录
    ///
    /// **同步写盘**，不走 `config_save` 队列：这里是"读旧表 + 追加 + 写回"，
    /// 排队时连着换两个模组，后一次可能读到还没落盘的旧表，把前一条冲掉。
    ///
    /// - `project_id`: 项目编号，即表键；第一次见到的项目会新建一项
    /// - `name`: 模组名字，非空时就更新表里记的那个（名字可能随版本变）
    /// - `record`: 记录内容；时间由调用方打好
    ///
    /// # 返回值
    ///
    /// 写盘失败时返回对应错误
    pub fn append_mod_update(
        &self,
        project_id: &str,
        name: &str,
        record: ModUpdateRecordObj,
    ) -> CoreResult<()> {
        let mut list = self.read_mod_update();
        let entry = list.entry(project_id.to_string()).or_default();
        if !name.is_empty() {
            entry.name = name.to_string();
        }
        entry.update.push(record);

        self.save_mod_update(&list)
    }

    /// 覆盖写入模组更新记录
    ///
    /// **同步写盘**，理由同 [`Self::append_mod_update`]：读旧表再整体写回，
    /// 走队列会与并发的读改写抢。
    ///
    /// - `list`: 整份记录表
    ///
    /// # 返回值
    ///
    /// 写盘失败时返回对应错误
    pub fn save_mod_update(&self, list: &ModUpdateList) -> CoreResult<()> {
        serialize_tools::json_to_file(&list, self.get_mod_update_file())
    }
}
