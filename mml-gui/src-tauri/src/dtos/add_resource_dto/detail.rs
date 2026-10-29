//! 项目详情 DTO：简介 + 正文 + 作者 + 标签 + 截图
//!
//! 从 `add_resource_dto/mod.rs` 拆出来的，`new_modrinth` / `new_curseforge` 两套构造。

use serde::{Deserialize, Serialize};

use mml_game::modrinth;
use mml_names::i18_items::error_type::CoreResult;
use mml_net::curseforge_api::list_obj::CurseForgeListDataObj;
use mml_net::modrinth_api;
use mml_net::modrinth_api::project_obj::ModrinthProjectObj;

use crate::image_manager;

use super::common::{DecPicDto, PicDto, TagDto};

/// 项目详情，双击列表项弹出：简介 + 正文 + 作者 + 标签 + 截图
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetailDto {
    /// 简介
    pub summary: String,
    /// 正文，即 Modrinth 的 body，markdown 文本；CurseForge 没有正文，只有简介
    pub body: Option<String>,
    /// 作者
    pub authors: Vec<PicDto>,
    /// 标签
    pub tag: Vec<TagDto>,
    /// 截图
    pub screenshots: Vec<DecPicDto>,
}

impl ProjectDetailDto {
    /// Modrinth 详情：project 拿正文 / 截图，team 拿作者头像
    ///
    /// - `data`: Modrinth 项目数据
    ///
    /// # 返回值
    ///
    /// 返回项目详情，team 请求失败返回对应错误
    pub async fn new_modrinth(data: &ModrinthProjectObj) -> CoreResult<Self> {
        let mut authors = Vec::new();

        let team = modrinth_api::get_team(&data.id).await?;

        for item in team {
            authors.push(PicDto {
                name: item.user.username,
                logo: item
                    .user
                    .avatar_url
                    .map(|item| image_manager::push_image_url(&item)),
            });
        }

        // Modrinth 把分类拆成主 / 次两份，少一份就会比列表卡片少一堆标签；
        // 合并后排序，顺序与搜索接口返回的 categories 一致
        let mut categories: Vec<String> = data
            .categories
            .iter()
            .chain(data.additional_categories.iter())
            .cloned()
            .collect();
        categories.sort();
        categories.dedup();

        let mut tag = Vec::new();

        for item in data.loaders.iter().chain(categories.iter()) {
            tag.push(TagDto {
                name: item.clone(),
                logo: None,
                svg: modrinth::get_categories_icon(item).await,
            });
        }

        let mut gallery: Vec<_> = data.gallery.iter().collect();
        gallery.sort_by_key(|a| a.ordering);

        let mut screenshots = Vec::new();
        for item in gallery {
            screenshots.push(DecPicDto {
                name: item.title.clone().unwrap_or_default(),
                logo: image_manager::push_image_url(&item.raw_url),
                description: item.description.clone().unwrap_or_default(),
            });
        }

        Ok(Self {
            summary: data.description.clone(),
            body: Some(data.body.clone()),
            authors,
            tag,
            screenshots,
        })
    }

    /// CurseForge 详情：只有简介，数据来自列表缓存或 mod_info，没有正文
    ///
    /// - `data`: CurseForge 项目数据
    ///
    /// # 返回值
    ///
    /// 返回项目详情，body 为 None
    pub fn new_curseforge(data: &CurseForgeListDataObj) -> Self {
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
            summary: data.summary.clone(),
            body: None,
            authors,
            tag,
            screenshots,
        }
    }
}
