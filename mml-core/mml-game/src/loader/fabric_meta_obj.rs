//! Fabric Meta 元数据 DTO

use serde::{Deserialize, Serialize};

/// Fabric Loader 版本项
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct FabricMetaLoaderObj {
    /// 版本号
    pub version: String,
    /// 是否为稳定版，`false` 表示快照或测试版
    pub stable: bool,
}

impl Default for FabricMetaLoaderObj {
    fn default() -> Self {
        Self {
            version: Default::default(),
            stable: Default::default(),
        }
    }
}

/// Fabric Meta 元数据
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct FabricMetaObj {
    /// 可用的 Loader 版本列表
    pub loader: Vec<FabricMetaLoaderObj>,
}

impl Default for FabricMetaObj {
    fn default() -> Self {
        Self {
            loader: Default::default(),
        }
    }
}
