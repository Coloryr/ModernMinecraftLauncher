//! MultiMC 实例组件配置 DTO，即 MMC

use serde::{Deserialize, Serialize};

/// MultiMC 实例组件配置
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct MMCObj {
    /// 组件列表
    pub components: Vec<ComponentsObj>,
}

/// 单个组件
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct ComponentsObj {
    /// 缓存的版本号
    #[serde(rename = "cachedVersion")]
    pub cached_version: String,
    /// 组件 UID，如 `net.minecraft`、`org.lwjgl`
    pub uid: String,
    /// 组件版本号
    pub version: String,
}
