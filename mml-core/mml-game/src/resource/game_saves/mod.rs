//! 游戏实例存档相关
//! 包括存档里面的数据包

pub(crate) use std::{
    collections::HashMap,
    io::{Read, Seek},
    path::{Path, PathBuf},
    sync::Mutex,
};

pub(crate) use chrono::{Datelike, Local, Timelike};
pub(crate) use mml_base::{
    archives::{ArchiveType, BaseArchive, BaseArchiveGui},
    serialize_tools::{self, MiniJsonObj},
};
pub(crate) use mml_config::config_save;
pub(crate) use mml_names::{
    i18_items::error_type::{CoreResult, DataNotFoundData, ErrorType, FileSystemErrorData},
    names, uuids,
};
pub(crate) use mml_nbt::{
    nbt_file::NbtFile,
    nbt_types::{NbtByte, NbtCompound, NbtInt, NbtList, NbtLong, NbtString},
};
pub(crate) use mml_net::chunkbase_api;
pub(crate) use mml_sys::path_helper;
pub(crate) use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
pub(crate) use serde::{Deserialize, Serialize};
pub(crate) use zip::ZipArchive;

pub(crate) use crate::error::{archive_read_err, serializer_err};
pub(crate) use crate::launcher::instance_setting_obj::InstanceSettingObj;

mod datapack;
mod instance;
mod read;
mod save;

pub(crate) use self::read::{read_pack, read_save};

/// 存档备份信息
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default, rename_all = "PascalCase")]
pub struct SaveBackupObj {
    /// 存档目录名，相对实例的 saves 目录
    pub dir: String,
    /// 该存档的备份文件名列表，按加入顺序排列
    pub back: Vec<String>,
}

/// 游戏存档，由 level.dat 解析而来
#[derive(Default)]
pub struct SaveObj {
    /// 地图种子
    pub random_seed: i64,
    /// 上次游玩时间，单位毫秒的 Unix 时间戳
    pub last_played: i64,
    /// 游戏模式：0 生存 / 1 创造 / 2 冒险 / 3 旁观
    pub game_type: i32,
    /// 是否极限模式，非 0 即开启
    pub hard_core: u8,
    /// 存档名字，界面上显示它
    pub level_name: String,
    /// 难度：0 和平 / 1 简单 / 2 普通 / 3 困难
    pub difficulty: u8,
    /// 生成器名字，如 `minecraft:large_biomes`
    pub generator_name: String,
    /// 存档目录的完整路径
    pub path: PathBuf,
    /// 存档图标文件路径，没有 icon.png 时为 `None`
    pub icon: Option<PathBuf>,
    /// level.dat 缺少必要字段时为 `true`，此时其余字段全是缺省值
    pub broken: bool,
    /// 解析出的原始 NBT，写回时以它为准
    pub nbt: NbtFile,
}

/// 存档内的数据包信息
pub struct SaveDataPackObj {
    /// 数据包名，形如 `file/<包名>`
    pub name: String,
    /// 数据包文件或目录的完整路径
    pub path: PathBuf,
    /// 来自 pack.mcmeta 的描述文本
    pub description: String,
    /// 数据包格式版本号，元数据缺失时为 -1
    pub pack_format: i64,
    /// 是否启用；`None` 表示 level.dat 的启用与禁用列表里都没有它
    pub enable: Option<bool>,
}

/// 存档可编辑的那几项，由资源管理窗口的「编辑存档」提交
pub struct SaveEditObj {
    /// 游戏模式：0 生存 / 1 创造 / 2 冒险 / 3 旁观
    pub game_type: i32,
    /// 难度：0 和平 / 1 简单 / 2 普通 / 3 困难
    pub difficulty: u8,
    /// 是否极限模式
    pub hard_core: bool,
    /// 存档名字，界面上显示它
    pub level_name: String,
    /// 地图种子
    pub seed: i64,
}

#[cfg(test)]
#[path = "../../../tests/unit/game_saves_tests.rs"]
mod tests;
