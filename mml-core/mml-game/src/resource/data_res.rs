//! 实例内资源下载项，含下载位置与在线文件记录的联动

use std::path::PathBuf;

use mml_base::file_item::FileItemObj;
use mml_names::names;

use crate::{
    launcher::{FileType, instance_setting_obj::InstanceSettingObj},
    launcher_path::instance_path::OnlineInfoList,
};

/// 下载项的存放位置信息
pub struct ItemPathRes {
    /// 文件要写入的目录，实例内绝对路径
    pub file_path: PathBuf,
    /// 实例内相对路径，写进在线文件记录用
    pub path: String,
    /// 下载项类型，与 `path` 指向的目录对应
    pub file_type: FileType,
}

/// 一批下载项及其在线文件记录
pub struct DownloadItemRes {
    /// 待下载的文件列表
    pub list: Vec<FileItemObj>,
    /// 在线文件信息，要保存到实例
    pub online: OnlineInfoList,
}

impl ItemPathRes {
    /// 切换为资源包目录
    ///
    /// - `game`: 目标实例
    pub fn change_to_resourcepacks(&mut self, game: &InstanceSettingObj) {
        self.file_path = game.get_resourcepacks_path();
        self.path = names::GAME_RESOURCEPACKS_DIR.to_string();
        self.file_type = FileType::Resourcepack;
    }

    /// 切换为光影包目录
    ///
    /// - `game`: 目标实例
    pub fn change_to_shaderpacks(&mut self, game: &InstanceSettingObj) {
        self.file_path = game.get_shaderpacks_path();
        self.path = names::GAME_SHADERPACKS_DIR.to_string();
        self.file_type = FileType::Shaderpack;
    }

    /// 切换为存档目录
    ///
    /// - `game`: 目标实例
    pub fn change_to_saves(&mut self, game: &InstanceSettingObj) {
        self.file_path = game.get_saves_path();
        self.path = names::GAME_SAVES_DIR.to_string();
        self.file_type = FileType::Save;
    }

    /// 切换为 OpenLoader 数据包目录
    ///
    /// - `game`: 目标实例
    pub fn change_to_openloader_datapack(&mut self, game: &InstanceSettingObj) {
        self.file_path = game
            .get_config_path()
            .join(names::OPEN_LOADER_DIR)
            .join(names::DATA_DIR);
        self.path = format!(
            "{}/{}/{}",
            names::GAME_CONFIG_DIR,
            names::OPEN_LOADER_DIR,
            names::DATA_DIR
        );
        self.file_type = FileType::OpenLoaderDataPack;
    }
}
