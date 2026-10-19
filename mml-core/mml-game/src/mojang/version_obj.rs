//! Mojang 版本清单 DTO

use serde::{Deserialize, Serialize};

/// 版本清单里的 `latest` 字段
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct LastestObj {
    /// 最新正式版版本号，如 `1.21.4`
    pub release: String,
}

/// 版本清单里的单个版本条目
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct VersionsObj {
    /// 版本 ID，如 `1.21.4` / `24w14a`
    pub id: String,
    /// 版本类型：`release` / `snapshot` / `old_beta` / `old_alpha`
    #[serde(rename = "type")]
    pub version_type: String,
    /// 该版本 JSON 的下载地址
    pub url: String,
    /// 该版本 JSON 的 SHA1 校验值
    pub sha1: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
/// 版本清单根对象，对应 `version_manifest_v2.json`
pub struct VersionObj {
    /// 最新版本号，正式版与快照各一条
    pub latest: LastestObj,
    /// 全部版本条目，按发布时间从新到旧
    pub versions: Vec<VersionsObj>,
}
