//! CurseForge 项目列表 DTO

use serde::{Deserialize, Serialize};

/// 项目详情接口 `/mods/{id}` 的响应
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct CurseForgeListObj {
    /// 项目列表
    pub data: CurseForgeListDataObj,
}

/// 项目分页列表，搜索接口 `/mods/search` 与批量查询 `/mods` 共用
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct CurseForgeListPageObj {
    /// 项目列表
    pub data: Vec<CurseForgeListDataObj>,
    /// 分页信息
    pub pagination: CurseForgeListPaginationObj,
}

/// 单个项目信息
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct CurseForgeListDataObj {
    /// 项目 ID
    pub id: u64,
    /// 所属大类 ID，如模组 / 整合包
    #[serde(rename = "classId")]
    pub class_id: u32,
    /// 项目名称
    pub name: String,
    /// 相关链接
    pub links: LinksObj,
    /// 项目简介
    pub summary: String,
    /// 下载量
    #[serde(rename = "downloadCount")]
    pub download_count: u64,
    /// 分类标签
    pub categories: Vec<CategoriesObj>,
    /// 作者列表
    pub authors: Vec<AuthorsObj>,
    /// 项目 Logo
    pub logo: LogoObj,
    /// 截图列表
    pub screenshots: Vec<ScreenshotsObj>,
    /// 最后更新时间，ISO 8601 UTC 字符串
    #[serde(rename = "dateModified")]
    pub date_modified: String,
}

/// 项目截图
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct ScreenshotsObj {
    /// 截图标题
    pub title: String,
    /// 截图说明
    pub description: String,
    /// 截图 URL
    pub url: String,
}

/// 项目 Logo
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct LogoObj {
    /// Logo 图片地址，接口未提供时为 `None`
    pub url: Option<String>,
}

/// 项目作者
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct AuthorsObj {
    /// 作者名称
    pub name: String,
    /// 头像图片地址，接口未提供时为 `None`
    #[serde(rename = "avatarUrl")]
    pub avatar_url: Option<String>,
}

/// 项目分类标签
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct CategoriesObj {
    /// 分类名称
    pub name: String,
    /// 分类图标 URL
    #[serde(rename = "iconUrl")]
    pub icon_url: String,
    /// 所属大类 ID
    #[serde(rename = "classId")]
    pub class_id: u32,
}

/// 项目相关链接
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct LinksObj {
    /// 官网地址
    #[serde(rename = "websiteUrl")]
    pub website_url: String,
}

/// 分页信息
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct CurseForgeListPaginationObj {
    /// 结果总数，用于翻页
    #[serde(rename = "totalCount")]
    pub total_count: u64,
}
