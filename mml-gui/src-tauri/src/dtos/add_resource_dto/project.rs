//! 项目搜索列表 DTO：搜索结果条目与分页信息
//!
//! 从 `add_resource_dto/mod.rs` 拆出来的。两个源的搜索命中统一成同一份前端结构，
//! 标签图标走内核的 Modrinth 分类接口，带缓存。

use serde::{Deserialize, Serialize};

use mml_game::launcher::{FileType, ModPackType};
use mml_game::modrinth;
use mml_net::curseforge_api::list_obj::CurseForgeListDataObj;
use mml_net::modrinth_api::search_obj::HitObj;

use crate::image_manager;

use super::common::{DecPicDto, McmodDto, PicDto, SourceTypeDto, TagDto, get_modrinth_url};

/// 项目搜索结果，分页
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDto {
    /// 项目列表
    pub items: Vec<ProjectItemDto>,
    /// 项目总数
    pub count: u64,
}

/// 项目搜索结果条目，即列表卡片
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectItemDto {
    /// 名字
    pub name: String,
    /// 描述
    pub summary: String,
    /// 图片地址，经 image_manager 转发，无则 None
    pub image: Option<String>,
    /// 作者
    pub authors: Vec<PicDto>,
    /// 标签
    pub tag: Vec<TagDto>,
    /// 截图
    pub screenshots: Vec<DecPicDto>,
    /// 下载次数
    pub download_count: u64,
    /// 更新时间
    pub date: String,
    /// 是否已经下载
    pub download: bool,
    /// 能否收藏
    pub can_star: bool,
    /// 是否已经收藏
    pub is_star: bool,
    /// 是否正在下载
    pub download_now: bool,
    /// 跳转网址
    pub url: String,
    /// 百科翻译条目，没有则 None
    pub mcmod: Option<McmodDto>,
    /// 下载源信息
    pub source: SourceTypeDto,
}

impl ProjectItemDto {
    /// 由 CurseForge 项目数据构造
    ///
    /// - `data`: CurseForge 项目数据
    /// - `file_type`: 资源类型
    /// - `download`: 是否已经下载
    /// - `can_star`: 能否收藏
    /// - `is_star`: 是否已经收藏
    /// - `download_now`: 是否正在下载
    /// - `mcmod`: 百科翻译，无则 None
    ///
    /// # 返回值
    ///
    /// 返回列表条目，图标 / 作者头像 / 标签 / 截图都经 image_manager 转发地址
    pub fn new_curseforge(
        data: &CurseForgeListDataObj,
        file_type: FileType,
        download: bool,
        can_star: bool,
        is_star: bool,
        download_now: bool,
        mcmod: Option<McmodDto>,
    ) -> Self {
        let mut authors = Vec::new();
        for item in data.authors.iter() {
            authors.push(PicDto {
                name: item.name.clone(),
                logo: item
                    .avatar_url
                    .as_ref()
                    .map(|item| image_manager::push_image_url(item)),
            });
        }

        let mut tag = Vec::new();
        for item in data.categories.iter() {
            tag.push(TagDto {
                name: item.name.clone(),
                logo: Some(image_manager::push_image_url(&item.icon_url)),
                svg: None,
            });
        }

        let mut screenshots = Vec::new();
        for item in data.screenshots.iter() {
            screenshots.push(DecPicDto {
                name: item.title.clone(),
                logo: image_manager::push_image_url(&item.url),
                description: item.description.clone(),
            });
        }

        Self {
            name: data.name.clone(),
            summary: data.summary.clone(),
            image: data
                .logo
                .url
                .as_ref()
                .map(|item| image_manager::push_image_url(item)),
            authors,
            tag,
            screenshots,
            download_count: data.download_count,
            date: data.date_modified.clone(),
            download,
            can_star,
            is_star,
            download_now,
            url: data.links.website_url.clone(),
            mcmod,
            source: SourceTypeDto {
                file_type: file_type.to_string(),
                source: ModPackType::CurseForge.to_string(),
                pid: data.id.to_string(),
                fid: Default::default(),
            },
        }
    }

    /// 由 Modrinth 搜索命中项构造，对齐旧 C# GetModPackListAsync：只从 HitObj 构造，
    /// 不发 project / team 请求，作者头像与截图留到项目详情 GetFileItemAsync 再补；
    /// 分类 svg 走 OnceLock 缓存的 /tag/category，逐条取没有额外网络请求
    ///
    /// - `data`: Modrinth 搜索命中项
    /// - `file_type`: 资源类型
    /// - `download`: 是否已经下载
    /// - `can_star`: 能否收藏
    /// - `is_star`: 是否已经收藏
    /// - `download_now`: 是否正在下载
    /// - `mcmod`: 百科翻译，无则 None
    ///
    /// # 返回值
    ///
    /// 返回列表条目
    pub async fn new_modrinth(
        data: &HitObj,
        file_type: FileType,
        download: bool,
        can_star: bool,
        is_star: bool,
        download_now: bool,
        mcmod: Option<McmodDto>,
    ) -> Self {
        let authors = vec![PicDto {
            name: data.author.clone(),
            logo: None,
        }];

        let mut tag = Vec::new();

        // 命中项的 categories 把加载器与分类混在一起，先排成"加载器在前、分类在后"，
        // 与项目详情页的标签顺序一致
        for item in modrinth::order_tags(&data.categories).await {
            let svg = modrinth::get_categories_icon(&item).await;
            tag.push(TagDto {
                name: item,
                logo: None,
                svg,
            });
        }

        Self {
            name: data.title.clone(),
            summary: data.description.clone(),
            image: data
                .icon_url
                .as_ref()
                .map(|item| image_manager::push_image_url(item)),
            authors,
            tag,
            screenshots: Vec::new(),
            download_count: data.downloads,
            date: data.date_modified.clone(),
            download,
            can_star,
            is_star,
            download_now,
            url: get_modrinth_url(&file_type, &data.project_id),
            mcmod,
            source: SourceTypeDto {
                file_type: file_type.to_string(),
                source: ModPackType::Modrinth.to_string(),
                pid: data.project_id.clone(),
                fid: Default::default(),
            },
        }
    }
}
