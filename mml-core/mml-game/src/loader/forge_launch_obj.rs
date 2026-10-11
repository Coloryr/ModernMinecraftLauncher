//! Forge 启动版本 JSON DTO

use serde::{Deserialize, Serialize};

use crate::mojang::game_arg_obj::ArtifactObj;

/// Forge 运行库下载信息
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct ForgeDownloadsObj {
    /// 构件下载信息
    pub artifact: ArtifactObj,
}

impl Default for ForgeDownloadsObj {
    /// 全字段取默认值：嵌套结构取默认值
    fn default() -> Self {
        Self {
            artifact: Default::default(),
        }
    }
}

/// Forge 运行库
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct ForgeLibrariesObj {
    /// Maven 坐标名
    pub name: String,
    /// 下载信息
    pub downloads: ForgeDownloadsObj,
}

impl Default for ForgeLibrariesObj {
    /// 全字段取默认值：字符串为空串
    fn default() -> Self {
        Self {
            name: Default::default(),
            downloads: Default::default(),
        }
    }
}

/// Forge 启动参数
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct ForgeArgumentsObj {
    /// 游戏参数
    pub game: Vec<String>,
    /// JVM 参数
    pub jvm: Vec<String>,
}

impl Default for ForgeArgumentsObj {
    /// 全字段取默认值：参数列表为空
    fn default() -> Self {
        Self {
            game: Default::default(),
            jvm: Default::default(),
        }
    }
}

/// Forge 启动版本信息
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct ForgeLaunchObj {
    /// 主类
    #[serde(rename = "mainClass")]
    pub main_class: String,
    /// 游戏启动参数，旧版格式
    #[serde(rename = "minecraftArguments")]
    pub minecraft_arguments: Option<String>,
    /// 启动参数，新版格式
    pub arguments: Option<ForgeArgumentsObj>,
    /// 运行库列表
    pub libraries: Vec<ForgeLibrariesObj>,
}

impl Default for ForgeLaunchObj {
    /// 全字段取默认值：可选字段为 `None`、列表为空
    fn default() -> Self {
        Self {
            main_class: Default::default(),
            minecraft_arguments: Default::default(),
            arguments: Default::default(),
            libraries: Default::default(),
        }
    }
}
