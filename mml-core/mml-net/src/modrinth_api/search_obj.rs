//! Modrinth 搜索结果 DTO

use serde::{Deserialize, Serialize};

/// Modrinth 搜索返回，对应 `search` 接口
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct ModrinthSearchObj {
    /// 命中的项目总数，对应响应键 `total_hits`，用于算总页数
    pub total_hits: u64,
    /// 命中条目列表，对应响应键 `hits`，只含分页后的当前页
    pub hits: Vec<HitObj>,
}

impl Default for ModrinthSearchObj {
    /// 全字段取默认值，配合 `#[serde(default)]` 兜底缺失字段
    fn default() -> Self {
        Self {
            total_hits: Default::default(),
            hits: Default::default(),
        }
    }
}

/// 单条搜索结果，对应 `hits` 数组里的一项
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct HitObj {
    /// 项目 ID，形如 `1KVo5zza`
    pub project_id: String,
    /// 作者名，可能为空串
    pub author: String,
    /// 项目名称
    pub title: String,
    /// 项目简介
    pub description: String,
    /// 分类标签列表，可能为空
    pub categories: Vec<String>,
    /// 图库图片 URL 列表，可能为空
    pub gallery: Vec<String>,
    /// 下载量，单位次
    pub downloads: u64,
    /// 图标 URL；项目没设图标时缺省
    pub icon_url: Option<String>,
    /// 最后更新时间，RFC 3339 字符串
    pub date_modified: String,
}

impl Default for HitObj {
    /// 全字段取默认值，配合 `#[serde(default)]` 兜底缺失字段
    fn default() -> Self {
        Self {
            project_id: Default::default(),
            author: Default::default(),
            title: Default::default(),
            description: Default::default(),
            categories: Default::default(),
            gallery: Default::default(),
            downloads: Default::default(),
            icon_url: Default::default(),
            date_modified: Default::default(),
        }
    }
}
