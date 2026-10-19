//! CurseForge 版本类型 DTO

use serde::{Deserialize, Serialize};

/// 游戏版本列表接口 `/games/{id}/versions` 的响应
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct CurseForgeVersionObj {
    /// 版本类型数据列表
    pub data: Vec<CurseForgeVersionDataObj>,
}

/// 单个版本类型分组
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct CurseForgeVersionDataObj {
    /// 版本类型 ID，与版本类型接口返回的 `id` 对应
    #[serde(rename = "type")]
    pub verion_type: u32,
    /// 该类型下的游戏版本号列表，如 1.20.4
    pub versions: Vec<String>,
}

/// 版本类型列表接口 `/games/{id}/version-types` 的响应
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct CurseForgeVersionTypeObj {
    /// 版本类型列表
    pub data: Vec<CurseForgeVersionTypeDataObj>,
}

/// 单个版本类型，`id` 与 `name` 一一对应
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct CurseForgeVersionTypeDataObj {
    /// 类型 ID，用于按类型筛选游戏版本
    pub id: u32,
    /// 类型名称，如 Minecraft 1.20.4
    pub name: String,
}
