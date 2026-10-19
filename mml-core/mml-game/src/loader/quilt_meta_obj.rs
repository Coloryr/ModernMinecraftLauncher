//! Quilt Meta 元数据 DTO

use serde::{Deserialize, Serialize};

/// Quilt Loader 版本项
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct QuiltMetaLoaderObj {
    /// 版本号
    pub version: String,
}

/// Quilt Meta 元数据
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct QuiltMetaObj {
    /// 可用的 Loader 版本列表
    pub loader: Vec<QuiltMetaLoaderObj>,
}
