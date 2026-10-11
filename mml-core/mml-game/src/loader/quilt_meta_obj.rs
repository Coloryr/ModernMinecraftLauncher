//! Quilt Meta 元数据 DTO

use serde::{Deserialize, Serialize};

/// Quilt Loader 版本项
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct QuiltMetaLoaderObj {
    /// 版本号
    pub version: String,
}

impl Default for QuiltMetaLoaderObj {
    /// 全字段取默认值：字符串为空串
    fn default() -> Self {
        Self {
            version: Default::default(),
        }
    }
}

/// Quilt Meta 元数据
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct QuiltMetaObj {
    /// 可用的 Loader 版本列表
    pub loader: Vec<QuiltMetaLoaderObj>,
}

impl Default for QuiltMetaObj {
    /// 全字段取默认值：版本列表为空
    fn default() -> Self {
        Self {
            loader: Default::default(),
        }
    }
}
