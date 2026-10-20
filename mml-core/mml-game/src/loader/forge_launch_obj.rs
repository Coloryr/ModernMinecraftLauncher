//! Forge 启动版本 JSON DTO

use serde::{Deserialize, Serialize};

use crate::mojang::game_arg_obj::ArtifactObj;

/// Forge 运行库下载信息
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct ForgeDownloadsObj {
    /// 构件下载信息
    pub artifact: ArtifactObj,
}

/// Forge 运行库
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct ForgeLibrariesObj {
    /// Maven 坐标名
    pub name: String,
    /// 下载信息
    pub downloads: ForgeDownloadsObj,
}

/// Forge 启动参数
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct ForgeArgumentsObj {
    /// 游戏参数
    pub game: Vec<String>,
    /// JVM 参数
    pub jvm: Vec<String>,
}

/// 加载器要求的 Java 版本
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct ForgeJavaVersionObj {
    /// 主版本号，如 25
    #[serde(rename = "majorVersion")]
    pub major_version: i32,
}

/// Forge 启动版本信息
#[derive(Serialize, Deserialize, Debug, Default)]
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
    /// 清单声明的 Java 版本；Cleanroom 的 version.json 带 `javaVersion`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub java_version: Option<ForgeJavaVersionObj>,
}
