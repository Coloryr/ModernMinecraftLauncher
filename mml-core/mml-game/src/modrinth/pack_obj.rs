//! Modrinth 整合包索引 DTO，对应 .mrpack 包

use std::collections::HashMap;

use mml_net::modrinth_api::version_obj::HasheObj;
use serde::{Deserialize, Serialize};

use crate::launcher::project_save_obj::MmlProjectSaveObj;

/// Modrinth 整合包索引，对应 `.mrpack` 内的 `modrinth.index.json`
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct ModrinthPackObj {
    /// 索引格式版本，目前固定为 1
    #[serde(rename = "formatVersion")]
    pub format_version: i32,
    /// 整合包版本 ID，由整合包作者给出
    #[serde(rename = "versionId")]
    pub version_id: String,
    /// 整合包名称
    pub name: String,
    /// 整合包简介
    pub summary: String,
    /// 需要下载的文件列表，含路径、哈希与下载地址
    pub files: Vec<ModrinthPackFileObj>,
    /// 依赖项：`minecraft` / `fabric-loader` 等键到版本号的映射
    pub dependencies: HashMap<String, String>,
}

/// 整合包索引里的单个待下载文件
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct ModrinthPackFileObj {
    /// 相对游戏目录的文件路径，用 `/` 分隔
    pub path: String,
    /// 文件哈希，比对时以 `sha1` 为准
    pub hashes: HasheObj,
    /// 下载地址列表，按顺序尝试
    pub downloads: Vec<String>,
    /// 文件大小，单位字节
    #[serde(rename = "fileSize")]
    pub file_size: u64,
    /// 来源项目信息，私有扩展字段，仅本启动器写入；可缺省
    #[serde(rename = "_private_data", alias = "_colormc")]
    pub project: Option<MmlProjectSaveObj>,
}
