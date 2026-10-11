//! Modrinth API
//!
//! 提供从 Modrinth 搜索与获取内容的功能：整合包 / 模组 / 资源包 / 光影包 /
//! 数据包的项目列表、项目详情、版本与文件、团队成员、分类标签，
//! 以及按文件哈希反查项目版本。
//!
//! # 子模块
//!
//! | 模块 | 用途 |
//! |------|------|
//! | [`project_obj`] | 项目详情 DTO |
//! | [`search_obj`] | 搜索结果 DTO |
//! | [`team_obj`] | 团队成员 DTO |
//! | [`version_obj`] | 版本与文件 DTO |

use std::collections::HashMap;

use mml_names::{
    i18_items::error_type::{CoreResult, ErrorData, ErrorType},
    names,
};
use serde::{Deserialize, Serialize};

use crate::{
    modrinth_api::{
        project_obj::ModrinthProjectObj, search_obj::ModrinthSearchObj, team_obj::ModrinthTeamObj,
        version_obj::ModrinthVersionObj,
    },
    urls,
};

pub mod project_obj;
pub mod search_obj;
pub mod team_obj;
pub mod version_obj;

/// 项目类型：整合包
pub const CLASS_MODPACK: &str = "modpack";
/// 项目类型：模组
pub const CLASS_MOD: &str = "mod";
/// 项目类型：资源包
pub const CLASS_RESOURCEPACK: &str = "resourcepack";
/// 项目类型：光影包
pub const CLASS_SHADERPACK: &str = "shader";
/// 数据包分类标签
pub const CATEGORIES_DATA_PACK: &str = "datapack";

/// Modrinth API 每分钟请求上限
const LIMITE_PER_MIN: u32 = 300;

/// 搜索 facets 过滤条件：同组内为 OR，组间为 AND
pub struct FacetsObj {
    /// 过滤字段名：`categories` / `versions` / `project_type`
    pub data: String,
    /// 该字段允许的取值列表，为空时这一组整体跳过
    pub values: Vec<String>,
}

/// Modrinth 搜索排序方式
pub enum ModrinthSortType {
    /// 推荐，对应 API 的 `relevance`
    Relevance,
    /// 下载次数，对应 API 的 `downloads`
    Downloads,
    /// 订阅数量，对应 API 的 `follows`
    Follows,
    /// 最新发布，对应 API 的 `newest`
    Newest,
    /// 最后更新，对应 API 的 `updated`
    Updated,
}

impl Default for ModrinthSortType {
    /// 默认排序方式：推荐
    fn default() -> Self {
        ModrinthSortType::Relevance
    }
}

impl ModrinthSortType {
    /// 获取排序方式名称
    ///
    /// # 返回值
    ///
    /// 返回 API 的 `index` 参数取值
    pub fn to_string(&self) -> String {
        String::from(match self {
            ModrinthSortType::Relevance => "relevance",
            ModrinthSortType::Downloads => "downloads",
            ModrinthSortType::Follows => "follows",
            ModrinthSortType::Newest => "newest",
            ModrinthSortType::Updated => "updated",
        })
    }

    /// 按名称解析排序方式
    ///
    /// - `id`: 排序方式名称
    ///
    /// # 返回值
    ///
    /// 返回对应的排序方式；未知名称返回 `None`
    pub fn from_string(id: &str) -> Option<ModrinthSortType> {
        match id {
            "relevance" => Some(ModrinthSortType::Relevance),
            "downloads" => Some(ModrinthSortType::Downloads),
            "follows" => Some(ModrinthSortType::Follows),
            "newest" => Some(ModrinthSortType::Newest),
            "updated" => Some(ModrinthSortType::Updated),
            _ => None,
        }
    }
}

/// Modrinth 搜索参数
pub struct ModrinthSearchArg {
    /// 游戏版本号，形如 `1.20.4`；`None` 表示不限
    pub version: Option<String>,
    /// 搜索词；`None` 或空串表示不带关键词
    pub query: Option<String>,
    /// 搜索排序方式，缺省按推荐
    pub sort: ModrinthSortType,
    /// 页数，从 0 起；`None` 按 0 处理
    pub page: Option<u32>,
    /// 每页条数；`None` 按 20 处理
    pub page_size: Option<u32>,
    /// 分类筛选，如 `adventure`；`None` 表示不限
    pub category: Option<String>,
    /// 加载器筛选，如 `fabric` / `forge`；`None` 表示不限
    pub loader: Option<String>,
}

impl Default for ModrinthSearchArg {
    /// 所有筛选条件留空，`sort` 取推荐
    fn default() -> Self {
        Self {
            version: Default::default(),
            query: Default::default(),
            sort: Default::default(),
            page: Default::default(),
            page_size: Default::default(),
            category: Default::default(),
            loader: Default::default(),
        }
    }
}

/// 构建 facets 查询参数 JSON
///
/// - `list`: 过滤条件列表
///
/// # 返回值
///
/// 返回形如 `[["versions:1.20.4"],["categories:forge"]]` 的 JSON 字符串
fn build_facets(list: Vec<FacetsObj>) -> String {
    let mut str = String::new();

    str.push('[');

    for item in list.iter() {
        if item.values.is_empty() {
            continue;
        }

        for item1 in item.values.iter() {
            str.push_str(&format!("[\"{}:{}\"],", &item.data, item1));
        }
    }

    str.remove(str.len() - 1);
    str.push(']');

    str
}

/// 构建分类过滤条件
///
/// - `values`: 分类名列表
///
/// # 返回值
///
/// 返回 categories 过滤条件
fn build_categories(values: Vec<String>) -> FacetsObj {
    FacetsObj {
        data: "categories".to_string(),
        values,
    }
}

/// 构建游戏版本过滤条件
///
/// - `values`: 游戏版本号列表
///
/// # 返回值
///
/// 返回 versions 过滤条件
fn build_versions(values: Vec<String>) -> FacetsObj {
    FacetsObj {
        data: "versions".to_string(),
        values,
    }
}

/// 构建项目类型过滤条件
///
/// - `values`: 项目类型列表
///
/// # 返回值
///
/// 返回 project_type 过滤条件
fn build_project_type(values: Vec<String>) -> FacetsObj {
    FacetsObj {
        data: "project_type".to_string(),
        values,
    }
}

/// 搜索内容
///
/// - `query`: 搜索词
/// - `index`: 排序方式
/// - `offset`: 结果偏移量
/// - `limit`: 单页数量
/// - `facets`: 过滤条件列表
///
/// # 返回值
///
/// 返回搜索结果，请求走速率限制
async fn search(
    query: &str,
    index: ModrinthSortType,
    offset: u32,
    limit: u32,
    facets: Vec<FacetsObj>,
) -> CoreResult<ModrinthSearchObj> {
    // 查询词与 facets 含空格、引号、方括号等特殊字符，
    // 必须经 URL 编码，否则服务端会静默返回空结果
    let mut url = reqwest::Url::parse(&format!("{}search", urls::MODRINTH_API)).unwrap();
    url.query_pairs_mut()
        .append_pair("query", query)
        .append_pair("index", &index.to_string())
        .append_pair("offset", &offset.to_string())
        .append_pair("limit", &limit.to_string())
        .append_pair("facets", &build_facets(facets));

    crate::get_work_client()
        .get_json_limited(url.as_str(), LIMITE_PER_MIN)
        .await
}

/// 获取整合包列表
///
/// - `arg`: 搜索参数，`loader` 字段在这里不生效
///
/// # 返回值
///
/// 返回整合包搜索结果
pub async fn get_modpack_list(arg: ModrinthSearchArg) -> CoreResult<ModrinthSearchObj> {
    let page_size = arg.page_size.unwrap_or(20);
    let mut facets = Vec::new();

    facets.push(build_project_type(vec![CLASS_MODPACK.to_string()]));

    if let Some(version) = arg.version {
        facets.push(build_versions(vec![version.clone()]));
    }

    if let Some(category) = arg.category {
        facets.push(build_categories(vec![category.clone()]));
    }

    search(
        &arg.query.unwrap_or_default(),
        arg.sort,
        arg.page.unwrap_or(0) * page_size,
        page_size,
        facets,
    )
    .await
}

/// 获取模组列表
///
/// - `arg`: 搜索参数
///
/// # 返回值
///
/// 返回模组搜索结果
pub async fn get_mod_list(arg: ModrinthSearchArg) -> CoreResult<ModrinthSearchObj> {
    let page_size = arg.page_size.unwrap_or(20);
    let mut facets = Vec::new();

    facets.push(build_project_type(vec![CLASS_MOD.to_string()]));

    if let Some(version) = arg.version {
        facets.push(build_versions(vec![version.clone()]));
    }

    let mut cate = build_categories(Vec::new());

    if let Some(category) = arg.category {
        cate.values.push(category.clone());
    }

    if let Some(loader) = arg.loader {
        cate.values.push(loader.clone());
    }

    facets.push(cate);

    search(
        &arg.query.unwrap_or_default(),
        arg.sort,
        arg.page.unwrap_or(0) * page_size,
        page_size,
        facets,
    )
    .await
}

/// 获取资源包列表
///
/// - `arg`: 搜索参数
///
/// # 返回值
///
/// 返回资源包搜索结果
pub async fn get_resourcepack_list(arg: ModrinthSearchArg) -> CoreResult<ModrinthSearchObj> {
    let page_size = arg.page_size.unwrap_or(20);
    let mut facets = Vec::new();

    facets.push(build_project_type(vec![CLASS_RESOURCEPACK.to_string()]));

    if let Some(version) = arg.version {
        facets.push(build_versions(vec![version.clone()]));
    }

    if let Some(category) = arg.category {
        facets.push(build_categories(vec![category.clone()]));
    }

    search(
        &arg.query.unwrap_or_default(),
        arg.sort,
        arg.page.unwrap_or(0) * page_size,
        page_size,
        facets,
    )
    .await
}

/// 获取光影包列表
///
/// - `arg`: 搜索参数
///
/// # 返回值
///
/// 返回光影包搜索结果
pub async fn get_shaderpack_list(arg: ModrinthSearchArg) -> CoreResult<ModrinthSearchObj> {
    let page_size = arg.page_size.unwrap_or(20);
    let mut facets = Vec::new();

    facets.push(build_project_type(vec![CLASS_SHADERPACK.to_string()]));

    if let Some(version) = arg.version {
        facets.push(build_versions(vec![version.clone()]));
    }

    if let Some(category) = arg.category {
        facets.push(build_categories(vec![category.clone()]));
    }

    search(
        &arg.query.unwrap_or_default(),
        arg.sort,
        arg.page.unwrap_or(0) * page_size,
        page_size,
        facets,
    )
    .await
}

/// 获取数据包列表
///
/// - `arg`: 搜索参数
///
/// # 返回值
///
/// 返回数据包搜索结果
pub async fn get_datapack_list(arg: ModrinthSearchArg) -> CoreResult<ModrinthSearchObj> {
    let page_size = arg.page_size.unwrap_or(20);
    let mut facets = Vec::new();

    facets.push(build_project_type(vec![CLASS_MOD.to_string()]));

    if let Some(version) = arg.version {
        facets.push(build_versions(vec![version.clone()]));
    }

    // datapack 是独立的一组 facets，组间 AND、组内 OR：
    // 无论是否选分类都必须带上，否则会搜出全部模组
    facets.push(build_categories(vec![CATEGORIES_DATA_PACK.to_string()]));

    if let Some(category) = arg.category {
        facets.push(build_categories(vec![category.clone()]));
    }

    search(
        &arg.query.unwrap_or_default(),
        arg.sort,
        arg.page.unwrap_or(0) * page_size,
        page_size,
        facets,
    )
    .await
}

/// 获取指定版本号的内容
///
/// - `id`: 项目编号
/// - `version`: 版本号
///
/// # 返回值
///
/// 返回该版本的信息，含文件列表
pub async fn get_version(id: &str, version: &str) -> CoreResult<ModrinthVersionObj> {
    let url = format!("{}project/{id}/version/{version}", urls::MODRINTH_API);

    crate::get_work_client()
        .get_json_limited(&url, LIMITE_PER_MIN)
        .await
}

/// 根据版本号获取项目信息
///
/// - `ids`: 版本号
///
/// # 返回值
///
/// 返回各版本的信息列表
pub async fn get_versions(ids: Vec<String>) -> CoreResult<Vec<ModrinthVersionObj>> {
    // 用紧凑 JSON + URL 编码：`json_to_string` 输出 pretty 多行格式，
    // 直接拼进 URL 会带换行与缩进，服务端解析失败或必须依赖 reqwest 兜底清理
    let ids_json = serde_json::to_string(&ids).map_err(|err| {
        ErrorType::SerializerError(ErrorData {
            error: err.to_string(),
        })
    })?;

    let mut url = reqwest::Url::parse(&format!("{}versions", urls::MODRINTH_API)).unwrap();
    url.query_pairs_mut().append_pair("ids", &ids_json);

    crate::get_work_client()
        .get_json_limited(url.as_str(), LIMITE_PER_MIN)
        .await
}

/// 获取团队列表
///
/// - `id`: 项目编号
///
/// # 返回值
///
/// 返回项目团队成员列表
pub async fn get_team(id: &str) -> CoreResult<Vec<ModrinthTeamObj>> {
    let url = format!("{}project/{id}/members", urls::MODRINTH_API);

    crate::get_work_client()
        .get_json_limited(&url, LIMITE_PER_MIN)
        .await
}

/// 获取指定项目的内容
///
/// - `id`: 项目编号
///
/// # 返回值
///
/// 返回项目详情
pub async fn get_project(id: &str) -> CoreResult<ModrinthProjectObj> {
    let url = format!("{}project/{id}", urls::MODRINTH_API);

    crate::get_work_client()
        .get_json_limited(&url, LIMITE_PER_MIN)
        .await
}

/// 获取文件列表
///
/// - `id`: 项目编号
/// - `version`: 游戏版本
/// - `loader`: 加载器版本
///
/// # 返回值
///
/// 返回符合条件的版本文件列表
pub async fn get_file_versions(
    id: &str,
    version: Option<&str>,
    loader: Option<&str>,
) -> CoreResult<Vec<ModrinthVersionObj>> {
    let url = match version {
        Some(version) => {
            let mut url = format!(
                "{}project/{id}/version?game_versions=[\"{version}\"]",
                urls::MODRINTH_API
            );

            if let Some(loader) = loader {
                url.push_str(&format!("&loaders=[\"{}\"]", loader.to_lowercase()));
            }

            url
        }
        None => {
            let mut url = format!("{}project/{id}/version?", urls::MODRINTH_API);

            if let Some(loader) = loader {
                url.push_str(&format!("loaders=[\"{}\"]", loader.to_lowercase()));
            }

            url
        }
    };

    crate::get_work_client()
        .get_json_limited(&url, LIMITE_PER_MIN)
        .await
}

/// 游戏版本标签，对应 `tag/game_version` 接口
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct ModrinthGameVersionObj {
    /// 游戏版本号，形如 `1.20.4`
    pub version: String,
}

impl Default for ModrinthGameVersionObj {
    /// 全字段取默认值，配合 `#[serde(default)]` 兜底缺失字段
    fn default() -> Self {
        Self {
            version: Default::default(),
        }
    }
}

/// 获取所有游戏版本
///
/// # 返回值
///
/// 返回 Modrinth 支持的所有游戏版本
pub async fn get_game_versions() -> CoreResult<Vec<ModrinthGameVersionObj>> {
    let url = format!("{}tag/game_version", urls::MODRINTH_API);

    crate::get_work_client()
        .get_json_limited(&url, LIMITE_PER_MIN)
        .await
}

/// 分类标签，对应 `tag/category` 接口
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct ModrinthCategoriesObj {
    /// 分类图标 URL，可能为空串
    pub icon: String,
    /// 分类名称，如 `adventure`
    pub name: String,
    /// 所属项目类型：`mod` / `modpack` / `resourcepack` / `shader` / `datapack`
    pub project_type: String,
    /// 所属分组标头：`categories` / `loaders` / `versions` 等
    pub header: String,
}

impl Default for ModrinthCategoriesObj {
    /// 全字段取默认值，配合 `#[serde(default)]` 兜底缺失字段
    fn default() -> Self {
        Self {
            icon: Default::default(),
            name: Default::default(),
            project_type: Default::default(),
            header: Default::default(),
        }
    }
}

/// 获取所有类型
///
/// # 返回值
///
/// 返回 Modrinth 的所有分类标签
pub async fn get_categories() -> CoreResult<Vec<ModrinthCategoriesObj>> {
    let url = format!("{}tag/category", urls::MODRINTH_API);

    crate::get_work_client()
        .get_json_limited(&url, LIMITE_PER_MIN)
        .await
}

/// 按哈希反查版本的请求体，对应 `version_files` 接口
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
struct VersionHashObj {
    /// 文件哈希列表，算法由 `algorithm` 指定
    pub hashes: Vec<String>,
    /// 哈希算法：`sha1` / `sha512`
    pub algorithm: String,
}

impl Default for VersionHashObj {
    /// 全字段取默认值，配合 `#[serde(default)]` 兜底缺失字段
    fn default() -> Self {
        Self {
            hashes: Default::default(),
            algorithm: Default::default(),
        }
    }
}

/// 从文件 SHA1 获取项目
///
/// - `sha1`: 文件 SHA1 列表，一次可查多个
///
/// # 返回值
///
/// 返回哈希到版本信息的映射，查不到的哈希不会出现在结果里
pub async fn get_version_from_sha1(
    sha1: Vec<String>,
) -> CoreResult<HashMap<String, ModrinthVersionObj>> {
    let url = format!("{}version_files", urls::MODRINTH_API);

    crate::get_work_client()
        .post_json_get_json_limited(
            &url,
            &VersionHashObj {
                hashes: sha1,
                algorithm: names::SHA1_EXT.to_string(),
            },
            LIMITE_PER_MIN,
        )
        .await
}

/// 从文件 SHA512 获取项目
///
/// - `sha512`: 文件 SHA512 列表，一次可查多个
///
/// # 返回值
///
/// 返回哈希到版本信息的映射，查不到的哈希不会出现在结果里
pub async fn get_version_from_sha512(
    sha512: Vec<String>,
) -> CoreResult<HashMap<String, ModrinthVersionObj>> {
    let url = format!("{}version_files", urls::MODRINTH_API);

    crate::get_work_client()
        .post_json_get_json_limited(
            &url,
            &VersionHashObj {
                hashes: sha512,
                algorithm: names::SHA512_EXT.to_string(),
            },
            LIMITE_PER_MIN,
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 单个值的 facets 应输出 `[["data:value"]]` 形式
    #[test]
    fn build_facets_single_value() {
        let facets = vec![build_versions(vec![String::from("1.20.4")])];
        assert_eq!(build_facets(facets), r#"[["versions:1.20.4"]]"#);
    }

    /// 同一 data 下的多个值各自成组，对应 Modrinth 的 OR 语义
    #[test]
    fn build_facets_multiple_values() {
        let facets = vec![build_categories(vec![
            String::from("adventure"),
            String::from("optimization"),
        ])];
        assert_eq!(
            build_facets(facets),
            r#"[["categories:adventure"],["categories:optimization"]]"#
        );
    }

    /// 多个 data 的 facets 依次拼接
    #[test]
    fn build_facets_multiple_groups() {
        let facets = vec![
            build_project_type(vec![CLASS_MODPACK.to_string()]),
            build_versions(vec![String::from("1.21.6")]),
        ];
        assert_eq!(
            build_facets(facets),
            r#"[["project_type:modpack"],["versions:1.21.6"]]"#
        );
    }

    /// values 为空的组应被跳过
    #[test]
    fn build_facets_skips_empty_group() {
        let facets = vec![
            build_categories(Vec::new()),
            build_project_type(vec![CLASS_MOD.to_string()]),
        ];
        assert_eq!(build_facets(facets), r#"[["project_type:mod"]]"#);
    }

    /// ModrinthSortType 的排序参数名
    #[test]
    fn sort_type_index() {
        assert_eq!(ModrinthSortType::Relevance.to_string(), "relevance");
        assert_eq!(ModrinthSortType::Downloads.to_string(), "downloads");
        assert_eq!(ModrinthSortType::Follows.to_string(), "follows");
        assert_eq!(ModrinthSortType::Newest.to_string(), "newest");
        assert_eq!(ModrinthSortType::Updated.to_string(), "updated");
    }
}
