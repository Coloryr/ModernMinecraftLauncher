//! Quilt Loader 版本 JSON DTO

use serde::{Deserialize, Serialize};

use crate::loader::LibrariesObj;

/// Quilt 启动参数
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct QuiltArgumentsObj {
    /// 游戏参数
    pub game: Vec<String>,
}

/// Quilt Loader 版本信息
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct QuiltLoaderObj {
    /// 加载器 ID
    pub id: String,
    /// 主类
    #[serde(rename = "mainClass")]
    pub main_class: String,
    /// 启动参数
    pub arguments: QuiltArgumentsObj,
    /// 运行库列表
    pub libraries: Vec<LibrariesObj>,
}
