//! MultiMC 实例组件配置 DTO，即 MMC

use serde::{Deserialize, Serialize};

/// MultiMC 实例组件配置
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct MMCObj {
    /// 清单格式版本，Prism / MultiMC 只认 1；旧文件没写时按 1 处理
    #[serde(rename = "formatVersion", default = "default_format_version")]
    pub format_version: u32,
    /// 组件列表
    pub components: Vec<ComponentsObj>,
}

/// 缺省清单格式版本
///
/// # 返回值
///
/// 固定的 1，对应 Prism / MultiMC 的组件清单格式
fn default_format_version() -> u32 {
    1
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
