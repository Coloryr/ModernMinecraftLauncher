//! Modrinth 搜索结果 DTO

use serde::{Deserialize, Serialize};

/// Modrinth 搜索返回，对应 `search` 接口
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct ModrinthSearchObj {
    /// 命中的项目总数，对应响应键 `total_hits`，用于算总页数
    pub total_hits: u64,
    /// 命中条目列表，对应响应键 `hits`，只含分页后的当前页
    pub hits: Vec<HitObj>,
}

/// 单条搜索结果，对应 `hits` 数组里的一项
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct HitObj {
    /// 项目 ID，形如 `1KVo5zza`
    pub project_id: String,
    /// 作者名，可能为空串
    pub author: String,
    /// 作者的用户 ID；搜索结果不给头像，头像要按它去 `users` 批量查
    pub author_id: String,
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
