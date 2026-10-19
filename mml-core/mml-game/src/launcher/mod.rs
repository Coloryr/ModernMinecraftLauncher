//! 实例运行设置与公共枚举
//!
//! 子模块:
//!
//! | 模块 | 职责 |
//! | --- | --- |
//! | `custom_game_arg_obj` | 自定义游戏参数 |
//! | `custom_loader_obj` | 自定义加载器 |
//! | `file_online_info_obj` | 在线文件信息 |
//! | `game_time_obj` | 游戏运行时长记录 |
//! | `instance_setting_obj` | 实例设置 |
//! | `mod_update_obj` | 模组更新记录 |
//! | `project_save_obj` | 项目保存信息 |

use mml_base::tools;
use serde_repr::{Deserialize_repr, Serialize_repr};

pub mod custom_game_arg_obj;
pub mod custom_loader_obj;
pub mod file_online_info_obj;
pub mod game_time_obj;
pub mod instance_setting_obj;
pub mod mod_update_obj;
pub mod project_save_obj;

/// 整合包类型
#[derive(Serialize_repr, Deserialize_repr, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ModPackType {
    /// CurseForge 整合包
    CurseForge,
    /// Modrinth 整合包
    Modrinth,
    /// McMod 整合包
    McMod,
    /// 服务器整合包
    ServerPack,
    /// 未识别
    None,
}

impl Default for ModPackType {
    /// 默认为未识别
    fn default() -> Self {
        ModPackType::None
    }
}

impl ModPackType {
    /// 独立 ID，跨进程传输用，显示名由前端 i18n 翻译
    ///
    /// # 返回值
    ///
    /// 该类型的独立 ID
    pub fn to_string(&self) -> String {
        String::from(match self {
            ModPackType::CurseForge => "curseforge",
            ModPackType::Modrinth => "modrinth",
            ModPackType::McMod => "mcmod",
            ModPackType::ServerPack => "serverpack",
            ModPackType::None => "none",
        })
    }

    /// 按 ID 解析整合包类型
    ///
    /// - `id`: 独立 ID，形如 `curseforge`
    ///
    /// # 返回值
    ///
    /// 解析出的整合包类型
    pub fn from_string(id: &str) -> Self {
        match id {
            "curseforge" => ModPackType::CurseForge,
            "modrinth" => ModPackType::Modrinth,
            "mcmod" => ModPackType::McMod,
            "serverpack" => ModPackType::ServerPack,
            _ => ModPackType::None,
        }
    }
}

/// 编码模式
#[derive(Serialize_repr, Deserialize_repr, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LogEncoding {
    /// UTF-8
    UTF8,
    /// GBK
    GBK,
}

impl Default for LogEncoding {
    /// 默认为 UTF-8
    fn default() -> Self {
        LogEncoding::UTF8
    }
}

/// 文件类型
#[derive(Serialize_repr, Deserialize_repr, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FileType {
    /// 整合包
    Modpack,
    /// 模组
    Mod,
    /// 存档
    Save,
    /// 光影包
    Shaderpack,
    /// 资源包
    Resourcepack,
    /// 数据包
    DataPacks,
    /// 建筑原理图
    Schematic,
    /// Java 运行时
    Java,
    /// 游戏
    Game,
    /// 配置
    Config,
    /// 登陆配置
    AuthConfig,
    /// 图片
    Pic,
    /// 高清修复
    Optifine,
    /// 皮肤
    Skin,
    /// 音乐
    Music,
    /// 文本
    Text,
    /// 实例图标
    GameIcon,
    /// 玩家头像
    Head,
    /// Java 压缩包
    JavaZip,
    /// 加载器
    Loader,
    /// 输入配置
    InputConfig,
    /// 用户
    User,
    /// 脚本
    Cmd,
    /// 图标
    Icon,
    /// 启动图标
    StartIcon,
    /// 普通文件
    File,
    /// OpenLoader 数据包
    OpenLoaderDataPack,
    /// 语言文件
    Lang,
    /// Json文件
    Json,
    /// Nbt文件
    Nbt,
    /// 区块文件
    Mca,
    /// Toml文件
    Toml,
    /// Yaml文件
    Yaml,
}

impl Default for FileType {
    /// 默认为整合包
    fn default() -> Self {
        FileType::Modpack
    }
}

impl FileType {
    /// 按独立 ID 解析文件类型
    ///
    /// - `id`: 文件类型 ID
    ///
    /// # 返回值
    ///
    /// 返回对应的文件类型；无法识别返回 `None`
    pub fn from_string(id: &str) -> Option<FileType> {
        match id {
            "modpack" => Some(FileType::Modpack),
            "mod" => Some(FileType::Mod),
            "save" => Some(FileType::Save),
            "shaderpack" => Some(FileType::Shaderpack),
            "resourcepack" => Some(FileType::Resourcepack),
            "dataPacks" => Some(FileType::DataPacks),
            "schematic" => Some(FileType::Schematic),
            _ => None,
        }
    }

    /// 独立 ID，跨进程传输用，显示名由前端 i18n 翻译
    ///
    /// # 返回值
    ///
    /// 返回文件类型 ID；未参与转换的类型返回空串
    pub fn to_string(&self) -> String {
        String::from(match self {
            FileType::Modpack => "modpack",
            FileType::Mod => "mod",
            FileType::Save => "save",
            FileType::Shaderpack => "shaderpack",
            FileType::Resourcepack => "resourcepack",
            FileType::DataPacks => "dataPacks",
            FileType::Schematic => "schematic",
            _ => "",
        })
    }
}

/// 检测下载源
///
/// - `pid`: 项目号
/// - `fid`: 文件号
///
/// # 返回值
///
/// 返回下载源类型：非纯数字 ID 判定为 Modrinth，否则为 CurseForge
pub fn get_source_type(pid: &str, fid: &str) -> ModPackType {
    if tools::check_is_not_number(pid) || tools::check_is_not_number(fid) {
        ModPackType::Modrinth
    } else {
        ModPackType::CurseForge
    }
}
