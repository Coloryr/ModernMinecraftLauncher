//! MultiMC 实例组件配置 DTO，即 MMC

use serde::{Deserialize, Serialize};

/// MultiMC 实例组件配置
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct MMCObj {
    /// 组件列表
    pub components: Vec<ComponentsObj>,
}

impl Default for MMCObj {
    /// 全字段取默认值：组件列表为空
    fn default() -> Self {
        Self {
            components: Default::default(),
        }
    }
}

/// 单个组件
#[derive(Serialize, Deserialize, Debug)]
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

impl Default for ComponentsObj {
    /// 全字段取默认值：字符串为空串
    fn default() -> Self {
        Self {
            cached_version: Default::default(),
            uid: Default::default(),
            version: Default::default(),
        }
    }
}
