//! Quilt Loader 版本 JSON DTO

use serde::{Deserialize, Serialize};

use crate::loader::LibrariesObj;

/// Quilt 启动参数
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct QuiltArgumentsObj {
    /// 游戏参数
    pub game: Vec<String>,
}

impl Default for QuiltArgumentsObj {
    /// 全字段取默认值：参数列表为空
    fn default() -> Self {
        Self {
            game: Default::default(),
        }
    }
}

/// Quilt Loader 版本信息
#[derive(Serialize, Deserialize, Debug)]
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

impl Default for QuiltLoaderObj {
    /// 全字段取默认值：字符串为空串、列表为空
    fn default() -> Self {
        Self {
            id: Default::default(),
            main_class: Default::default(),
            arguments: Default::default(),
            libraries: Default::default(),
        }
    }
}
