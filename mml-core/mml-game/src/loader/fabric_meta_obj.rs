//! Fabric Meta 元数据 DTO

use serde::{Deserialize, Serialize};

/// Fabric Loader 版本项
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct FabricMetaLoaderObj {
    /// 版本号
    pub version: String,
    /// 是否为稳定版，`false` 表示快照或测试版
    pub stable: bool,
}

/// Fabric Meta 元数据
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct FabricMetaObj {
    /// 可用的 Loader 版本列表
    pub loader: Vec<FabricMetaLoaderObj>,
    /// 支持的游戏版本列表，能装哪些版本由它决定
    pub game: Vec<FabricMetaLoaderObj>,
}
