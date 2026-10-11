//! Modrinth 项目详情 DTO

use serde::{Deserialize, Serialize};

/// Modrinth 项目详情，对应项目详情接口的响应体
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct ModrinthProjectObj {
    /// 项目 ID，对应 `id`
    pub id: String,
    /// 项目类型，对应 `project_type`，取值 `mod` / `modpack` 等
    pub project_type: String,
    /// 项目名称，对应 `title`
    pub title: String,
    /// 简介，对应 `description`
    pub description: String,
    /// 正文描述，对应 `body`，是 Markdown 原文
    pub body: String,
    /// 最后更新时间，对应 `updated`，ISO 8601 字符串
    pub updated: String,
    /// 下载量，对应 `downloads`
    pub downloads: u64,
    /// 支持的加载器列表，对应 `loaders`
    pub loaders: Vec<String>,
    /// 图标 URL，对应 `icon_url`
    pub icon_url: String,
    /// 分类标签，对应 `categories`
    pub categories: Vec<String>,
    /// 图库列表，对应 `gallery`
    pub gallery: Vec<GalleryObj>,
}

impl Default for ModrinthProjectObj {
    fn default() -> Self {
        Self {
            id: Default::default(),
            project_type: Default::default(),
            title: Default::default(),
            description: Default::default(),
            body: Default::default(),
            updated: Default::default(),
            downloads: Default::default(),
            loaders: Default::default(),
            icon_url: Default::default(),
            categories: Default::default(),
            gallery: Default::default(),
        }
    }
}

/// 项目图库图片，对应 `gallery[]` 的元素
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct GalleryObj {
    /// 图片标题，对应 `title`，可缺省为 `null`
    pub title: Option<String>,
    /// 原始图片 URL，对应 `raw_url`
    pub raw_url: String,
    /// 图片说明，对应 `description`，可缺省为 `null`
    pub description: Option<String>,
    /// 排序序号，对应 `ordering`，越小越靠前
    pub ordering: i32,
}

impl Default for GalleryObj {
    fn default() -> Self {
        Self {
            title: Default::default(),
            raw_url: Default::default(),
            description: Default::default(),
            ordering: Default::default(),
        }
    }
}
