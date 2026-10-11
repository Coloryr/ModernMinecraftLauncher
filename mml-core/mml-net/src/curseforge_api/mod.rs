//! CurseForge API
//!
//! 提供与 CurseForge REST API 交互的功能，支持搜索和下载：
//!
//! # 支持的内容类型
//!
//! | 常量 | 值 | 用途 |
//! |------|---|------|
//! | `CLASS_MOD` | 6 | 模组 |
//! | `CLASS_MODPACK` | 4471 | 整合包 |
//! | `CLASS_SAVES` | 17 | 存档 |
//! | `CLASS_RESOURCEPACKS` | 12 | 资源包 |
//! | `CLASS_SHADERPACKS` | 6552 | 光影包 |
//!
//! # API Key
//!
//! CurseForge API 需要 API Key，通过 [`set_key()`] 在初始化时设置。

use std::sync::OnceLock;

use mml_names::i18_items::error_type::{CoreResult, ErrorData, ErrorType};
use reqwest::{
    Method, Url,
    header::{CONTENT_TYPE, HeaderValue},
};
use serde::{Serialize, de::DeserializeOwned};

use crate::{
    curseforge_api::{
        categories_obj::CurseForgeCategoriesObj,
        file_obj::{CurseFogreFilePageObj, CurseForgeFileDataObj, CurseForgeFileObj},
        list_obj::{CurseForgeListObj, CurseForgeListPageObj},
        version_obj::{CurseForgeVersionObj, CurseForgeVersionTypeObj},
    },
    urls,
};

pub mod categories_obj;
pub mod file_obj;
pub mod list_obj;
pub mod version_obj;

/// CurseForge 游戏 ID，Minecraft 为 432
pub const GAME_ID: u32 = 432;
/// 模组加载器类型：Fabric，用于 `modLoaderType` 查询参数
pub const MODLOADER_FABRIC: u32 = 4;
/// 模组加载器类型：NeoForge，用于 `modLoaderType` 查询参数
pub const MODLOADER_NEOFORGE: u32 = 6;
/// 分类 ID：整合包
pub const CLASS_MODPACK: u32 = 4471;
/// 分类 ID：模组
pub const CLASS_MOD: u32 = 6;
/// 分类 ID：存档
pub const CLASS_SAVES: u32 = 17;
/// 分类 ID：资源包
pub const CLASS_RESOURCEPACKS: u32 = 12;
/// 分类 ID：光影包
pub const CLASS_SHADERPACKS: u32 = 6552;
/// 分类 ID：数据包，OpenLoader 用
pub const CLASS_OPENLOADER_DATAPACK: u32 = 6945;
/// 分类 ID：数据包，配 `CLASS_RESOURCEPACKS` 使用
pub const CATEGORYID_DATAPACKS: u32 = 5193;

/// 全局 CurseForge API Key，由 `set_key` 首次写入
static API_KEY: OnceLock<String> = OnceLock::new();

/// 搜索排序方式，对应 API 的 `sortField` 取值
#[derive(Debug, Clone, Copy)]
pub enum CurseForgeSortType {
    /// 流行度，`sortField` = 2
    Popularity,
    /// 特性，`sortField` = 1
    Featured,
    /// 上次更新，`sortField` = 3
    LastUpdated,
    /// 名字，`sortField` = 4
    Name,
    /// 下载次数，`sortField` = 6
    TotalDownloads,
}

impl CurseForgeSortType {
    /// 获取排序方式对应编号
    ///
    /// # 返回值
    ///
    /// 返回 API 的 `sortField` 取值
    fn to_id(&self) -> u32 {
        match self {
            CurseForgeSortType::Featured => 1,
            CurseForgeSortType::Popularity => 2,
            CurseForgeSortType::LastUpdated => 3,
            CurseForgeSortType::Name => 4,
            CurseForgeSortType::TotalDownloads => 6,
        }
    }

    /// 根据排序方式获取排序方向编号
    ///
    /// # 返回值
    ///
    /// 返回 API 的 `sortOrder` 取值：1 为降序，0 为字母序
    fn to_order_index(&self) -> u32 {
        match self {
            CurseForgeSortType::Featured
            | CurseForgeSortType::Popularity
            | CurseForgeSortType::LastUpdated
            | CurseForgeSortType::TotalDownloads => 1,
            CurseForgeSortType::Name => 0,
        }
    }

    /// 获取排序方式名称
    ///
    /// # 返回值
    ///
    /// 返回排序方式名称
    pub fn to_string(&self) -> String {
        String::from(match self {
            CurseForgeSortType::Popularity => "popularity",
            CurseForgeSortType::Featured => "featured",
            CurseForgeSortType::LastUpdated => "last_updated",
            CurseForgeSortType::Name => "name",
            CurseForgeSortType::TotalDownloads => "total_downloads",
        })
    }

    /// 按名称解析排序方式
    ///
    /// - `id`: 排序方式名称
    ///
    /// # 返回值
    ///
    /// 返回对应的排序方式；未知名称返回 `None`
    pub fn from_string(id: &str) -> Option<CurseForgeSortType> {
        match id {
            "popularity" => Some(CurseForgeSortType::Popularity),
            "featured" => Some(CurseForgeSortType::Featured),
            "last_updated" => Some(CurseForgeSortType::LastUpdated),
            "name" => Some(CurseForgeSortType::Name),
            "total_downloads" => Some(CurseForgeSortType::TotalDownloads),
            _ => None,
        }
    }
}

impl Default for CurseForgeSortType {
    /// 默认排序方式：流行度
    fn default() -> Self {
        CurseForgeSortType::Popularity
    }
}

/// CurseForge 搜索/列表请求参数
pub struct CurseFogreArg {
    /// 项目编号；`None` 或空串表示不带
    pub id: Option<String>,
    /// 游戏版本，形如 `1.20.4`；`None` 或空串表示不限
    pub version: Option<String>,
    /// 页数，从 0 起；`None` 按 0 处理
    pub page: Option<u32>,
    /// 排序方式；`None` 按流行度
    pub sort: Option<CurseForgeSortType>,
    /// 搜索过滤词；`None` 或空串表示不带
    pub filter: Option<String>,
    /// 每页条数；`None` 时列表类按 20、文件列表按 50
    pub page_size: Option<u32>,
    /// 分类 ID；`None` 或空串表示不限
    pub category: Option<String>,
    /// 模组加载器类型，取值见 `MODLOADER_*` 常量；`None` 或 0 表示不限
    pub loader: Option<u32>,
}

impl Default for CurseFogreArg {
    /// 所有筛选条件留空，`sort` 取流行度
    fn default() -> Self {
        Self {
            id: Default::default(),
            version: Default::default(),
            page: Default::default(),
            sort: Default::default(),
            filter: Default::default(),
            page_size: Default::default(),
            category: Default::default(),
            loader: Default::default(),
        }
    }
}

/// 设置 API Key，只在首次调用时生效
///
/// - `key`: CurseForge API Key
pub fn set_key(key: &str) {
    API_KEY.get_or_init(|| key.to_string());
}

/// 获取 API Key
///
/// # 返回值
///
/// 返回已设置的 Key；没设置过返回 `KeyIsNull`
pub fn get_key() -> CoreResult<String> {
    match API_KEY.get() {
        Some(key) => Ok(key.clone()),
        None => Err(ErrorType::KeyIsNull),
    }
}

/// 发送请求，带上 API Key 并解析响应
///
/// - `req`: 请求内容，方法、URL、请求体都已装好
///
/// # 返回值
///
/// 返回响应体反序列化后的 `T`
async fn send<T: DeserializeOwned>(mut req: reqwest::Request) -> CoreResult<T> {
    req.headers_mut()
        .insert("x-api-key", get_key()?.parse().unwrap());

    let res = crate::get_work_client().send(req).await?;
    crate::handle_response(res).await
}

/// 发送项目搜索请求，各类列表的公共实现
///
/// - `classid`: 分类 ID，取值见 `CLASS_*` 常量
/// - `version`: 游戏版本，空串表示不限
/// - `page`: 页码，从 0 起
/// - `sort`: 排序字段编号
/// - `filter`: 搜索过滤词
/// - `page_size`: 页大小
/// - `sort_order`: 排序方向编号
/// - `category`: 分类 ID，空串表示不限
/// - `mod_loader_type`: 加载器类型，0 表示不限
///
/// # 返回值
///
/// 返回分页的项目列表
async fn get_list(
    classid: u32,
    version: &str,
    page: u32,
    sort: u32,
    filter: &str,
    page_size: u32,
    sort_order: u32,
    category: &str,
    mod_loader_type: u32,
) -> CoreResult<CurseForgeListPageObj> {
    let mut url = format!(
        "{}mods/search?gameId={}&classId={classid}&gameVersion={version}&index={}&sortField={sort}&searchFilter={filter}&pageSize={page_size}&sortOrder={sort_order}&categoryId={category}",
        urls::CURSEFORGE,
        GAME_ID,
        page * page_size
    );

    if mod_loader_type != 0 {
        url.push_str(&format!("&modLoaderType={mod_loader_type}"));
    }

    let req = reqwest::Request::new(Method::GET, Url::parse(&url).unwrap());

    send(req).await
}

/// 获取整合包列表
///
/// - `arg`: 搜索参数，`loader` 字段在这里不生效
///
/// # 返回值
///
/// 返回整合包分页列表
pub async fn get_modpack_list(arg: CurseFogreArg) -> CoreResult<CurseForgeListPageObj> {
    get_list(
        CLASS_MODPACK,
        &arg.version.unwrap_or_default(),
        arg.page.unwrap_or(0),
        arg.sort.unwrap_or_default().to_id(),
        &arg.filter.unwrap_or_default(),
        arg.page_size.unwrap_or(20),
        arg.sort.unwrap_or_default().to_order_index(),
        &arg.category.unwrap_or_default(),
        0,
    )
    .await
}

/// 获取模组列表
///
/// - `arg`: 搜索参数
///
/// # 返回值
///
/// 返回模组分页列表
pub async fn get_mod_list(arg: CurseFogreArg) -> CoreResult<CurseForgeListPageObj> {
    get_list(
        CLASS_MOD,
        &arg.version.unwrap_or_default(),
        arg.page.unwrap_or(0),
        arg.sort.unwrap_or_default().to_id(),
        &arg.filter.unwrap_or_default(),
        arg.page_size.unwrap_or(20),
        arg.sort.unwrap_or_default().to_order_index(),
        &arg.category.unwrap_or_default(),
        arg.loader.unwrap_or(0),
    )
    .await
}

/// 获取存档列表
///
/// - `arg`: 搜索参数
///
/// # 返回值
///
/// 返回存档分页列表
pub async fn get_save_list(arg: CurseFogreArg) -> CoreResult<CurseForgeListPageObj> {
    get_list(
        CLASS_SAVES,
        &arg.version.unwrap_or_default(),
        arg.page.unwrap_or(0),
        arg.sort.unwrap_or_default().to_id(),
        &arg.filter.unwrap_or_default(),
        arg.page_size.unwrap_or(20),
        arg.sort.unwrap_or_default().to_order_index(),
        &arg.category.unwrap_or_default(),
        0,
    )
    .await
}

/// 获取资源包列表
///
/// - `arg`: 搜索参数
///
/// # 返回值
///
/// 返回资源包分页列表
pub async fn get_resourcepack_list(arg: CurseFogreArg) -> CoreResult<CurseForgeListPageObj> {
    get_list(
        CLASS_RESOURCEPACKS,
        &arg.version.unwrap_or_default(),
        arg.page.unwrap_or(0),
        arg.sort.unwrap_or_default().to_id(),
        &arg.filter.unwrap_or_default(),
        arg.page_size.unwrap_or(20),
        arg.sort.unwrap_or_default().to_order_index(),
        &arg.category.unwrap_or_default(),
        0,
    )
    .await
}

/// 获取数据包列表
///
/// CurseForge 没有独立的数据包分类，这里走资源包分类加 datapack 分类 ID。
///
/// - `arg`: 搜索参数，`category` 字段在这里被固定的 datapack 分类覆盖
///
/// # 返回值
///
/// 返回数据包分页列表
pub async fn get_datapacks_list(arg: CurseFogreArg) -> CoreResult<CurseForgeListPageObj> {
    get_list(
        CLASS_RESOURCEPACKS,
        &arg.version.unwrap_or_default(),
        arg.page.unwrap_or(0),
        arg.sort.unwrap_or_default().to_id(),
        &arg.filter.unwrap_or_default(),
        arg.page_size.unwrap_or(20),
        arg.sort.unwrap_or_default().to_order_index(),
        &CATEGORYID_DATAPACKS.to_string(),
        0,
    )
    .await
}

/// 获取光影包列表
///
/// - `arg`: 搜索参数
///
/// # 返回值
///
/// 返回光影包分页列表
pub async fn get_shaders_list(arg: CurseFogreArg) -> CoreResult<CurseForgeListPageObj> {
    get_list(
        CLASS_SHADERPACKS,
        &arg.version.unwrap_or_default(),
        arg.page.unwrap_or(0),
        arg.sort.unwrap_or_default().to_id(),
        &arg.filter.unwrap_or_default(),
        arg.page_size.unwrap_or(20),
        arg.sort.unwrap_or_default().to_order_index(),
        "",
        0,
    )
    .await
}

/// 获取模组信息
///
/// - `pid`: 项目编号
/// - `fid`: 文件编号
///
/// # 返回值
///
/// 返回该文件的详细信息
pub async fn get_mod(pid: &str, fid: &str) -> CoreResult<CurseForgeFileObj> {
    let url = format!("{}mods/{pid}/files/{fid}", urls::CURSEFORGE);

    let req = reqwest::Request::new(Method::GET, Url::parse(&url).unwrap());

    send(req).await
}

/// 批量查询文件的请求体，对应 `mods/files` 接口
#[derive(Serialize, Debug)]
#[serde(default)]
struct CurseForgeGetFilesObj {
    /// 文件 ID 列表，对应请求键 `fileIds`
    #[serde(rename = "fileIds")]
    pub file_ids: Vec<u64>,
}

/// 把序列化后的 JSON 写进请求体
///
/// - `req`: 目标请求
/// - `json`: 请求体对象
///
/// # 返回值
///
/// 成功返回 `Ok(())`；序列化失败返回 `SerializerError`
pub fn json<T: Serialize>(req: &mut reqwest::Request, json: &T) -> CoreResult<()> {
    match serde_json::to_vec(json) {
        Ok(body) => {
            req.headers_mut()
                .entry(CONTENT_TYPE)
                .or_insert_with(|| HeaderValue::from_static("application/json"));
            *req.body_mut() = Some(body.into());

            Ok(())
        }
        Err(err) => Err(ErrorType::SerializerError(ErrorData {
            error: err.to_string(),
        })),
    }
}

/// 获取文件列表
///
/// - `ids`: 文件 ID 列表
///
/// # 返回值
///
/// 返回各文件的详细信息
pub async fn get_files(ids: Vec<u64>) -> CoreResult<Vec<CurseForgeFileDataObj>> {
    let obj = CurseForgeGetFilesObj { file_ids: ids };

    let url = format!("{}mods/files", urls::CURSEFORGE);

    let mut req = reqwest::Request::new(Method::POST, Url::parse(&url).unwrap());

    json(&mut req, &obj)?;

    // 该接口返回包装对象 `{"data": [...]}` 而非裸数组，按 Vec 反序列化会报
    // "invalid type: map, expected a sequence"；复用分页结构，pagination 由 serde(default) 兜底
    let res: CurseFogreFilePageObj = send(req).await?;

    Ok(res.data)
}

/// 获取分类信息
///
/// # 返回值
///
/// 返回 Minecraft 下所有分类
pub async fn get_categories() -> CoreResult<CurseForgeCategoriesObj> {
    let url = format!("{}categories?gameId={}", urls::CURSEFORGE, GAME_ID);

    let req = reqwest::Request::new(Method::GET, Url::parse(&url).unwrap());

    send(req).await
}

/// 获取游戏版本列表
///
/// # 返回值
///
/// 返回 Minecraft 的游戏版本分组列表
pub async fn get_version() -> CoreResult<CurseForgeVersionObj> {
    let url = format!("{}games/{}/versions", urls::CURSEFORGE, GAME_ID);

    let req = reqwest::Request::new(Method::GET, Url::parse(&url).unwrap());

    send(req).await
}

/// 获取版本类型列表
///
/// # 返回值
///
/// 返回 Minecraft 的版本类型列表，如 release / beta
pub async fn get_version_type() -> CoreResult<CurseForgeVersionTypeObj> {
    let url = format!("{}games/{}/version-types", urls::CURSEFORGE, GAME_ID);

    let req = reqwest::Request::new(Method::GET, Url::parse(&url).unwrap());

    send(req).await
}

/// 获取模组项目信息
///
/// - `id`: 项目编号
///
/// # 返回值
///
/// 返回项目详情
pub async fn get_mod_info(id: &str) -> CoreResult<CurseForgeListObj> {
    let url = format!("{}mods/{id}", urls::CURSEFORGE);

    let req = reqwest::Request::new(Method::GET, Url::parse(&url).unwrap());

    send(req).await
}

/// 批量查询项目的请求体，对应 `mods` 接口
#[derive(Serialize, Debug)]
#[serde(default)]
struct CurseForgeModsInfoObj {
    /// 项目 ID 列表，对应请求键 `modIds`
    #[serde(rename = "modIds")]
    pub mod_ids: Vec<u64>,
    /// 是否仅保留 PC 可用项目，对应请求键 `filterPcOnly`
    #[serde(rename = "filterPcOnly")]
    pub filter: bool,
}

/// 批量获取项目信息
///
/// - `ids`: 项目 ID 列表
///
/// # 返回值
///
/// 返回各项目的详情分页结果
pub async fn get_mods_info(ids: Vec<u64>) -> CoreResult<CurseForgeListPageObj> {
    let obj = CurseForgeModsInfoObj {
        mod_ids: ids,
        filter: true,
    };

    let url = format!("{}mods", urls::CURSEFORGE);

    let mut req = reqwest::Request::new(Method::POST, Url::parse(&url).unwrap());

    json(&mut req, &obj)?;

    send(req).await
}

/// 获取指定项目的文件列表
///
/// - `arg`: 查询参数，用其中的 `id` / `page` / `page_size` / `version` / `loader`
///
/// # 返回值
///
/// 返回该项目的文件分页列表
pub async fn get_files_page(arg: CurseFogreArg) -> CoreResult<CurseFogreFilePageObj> {
    let page_size = arg.page_size.unwrap_or(50);

    let mut url = format!(
        "{}mods/{}/files?index={}&pageSize={page_size}&gameVersion={}",
        urls::CURSEFORGE,
        arg.id.unwrap_or_default(),
        arg.page.unwrap_or(0) * page_size,
        arg.version.unwrap_or_default()
    );

    if let Some(loader) = arg.loader {
        url.push_str(&format!("&modLoaderType={loader}"));
    }

    let req = reqwest::Request::new(Method::GET, Url::parse(&url).unwrap());

    send(req).await
}

impl CurseForgeFileDataObj {
    /// 修正下载地址
    ///
    /// `download_url` 缺失时就地按 CurseForge 的 CDN 规则补齐
    pub fn fix_download_url(&mut self) {
        if self.download_url.is_none() {
            self.download_url = Some(format!(
                "{}files/{}/{}/{}",
                urls::CURSEFORGE_DOWNLOAD,
                self.id / 1000,
                self.id % 1000,
                self.file_name
            ))
        }
    }

    /// 提取 SHA1 哈希值
    ///
    /// # 返回值
    ///
    /// 返回 SHA1 哈希；没有时返回空字符串
    #[inline]
    pub fn sha1_hash(&self) -> String {
        self.hashes
            .iter()
            .find(|h| h.algo == 1)
            .map(|h| h.value.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curseforge_api::file_obj::HashesObj;

    /// 排序方式编号与 CurseForge API 的 sortField 取值一致：
    /// Featured=1，Popularity=2，LastUpdated=3，Name=4，TotalDownloads=6
    #[test]
    fn sort_type_index() {
        assert_eq!(CurseForgeSortType::Featured.to_id(), 1);
        assert_eq!(CurseForgeSortType::Popularity.to_id(), 2);
        assert_eq!(CurseForgeSortType::LastUpdated.to_id(), 3);
        assert_eq!(CurseForgeSortType::Name.to_id(), 4);
        assert_eq!(CurseForgeSortType::TotalDownloads.to_id(), 6);
    }

    /// 排序方向编号：Name 为 0，其余为 1
    #[test]
    fn sort_type_order_index() {
        assert_eq!(CurseForgeSortType::Featured.to_order_index(), 1);
        assert_eq!(CurseForgeSortType::Popularity.to_order_index(), 1);
        assert_eq!(CurseForgeSortType::LastUpdated.to_order_index(), 1);
        assert_eq!(CurseForgeSortType::TotalDownloads.to_order_index(), 1);
        assert_eq!(CurseForgeSortType::Name.to_order_index(), 0);
    }

    /// 默认排序方式应为流行度
    #[test]
    fn sort_type_default() {
        assert!(matches!(
            CurseForgeSortType::default(),
            CurseForgeSortType::Popularity
        ));
    }

    /// 构造一个测试用的文件数据对象
    fn make_file(id: u64, file_name: &str, sha1: Option<&str>) -> CurseForgeFileDataObj {
        CurseForgeFileDataObj {
            id,
            file_name: file_name.to_string(),
            hashes: match sha1 {
                Some(value) => vec![HashesObj {
                    value: value.to_string(),
                    algo: 1,
                }],
                None => Vec::new(),
            },
            ..Default::default()
        }
    }

    /// fix_download_url：downloadUrl 缺失时按 CF 的 CDN 规则补齐，
    /// 形如 `{CDN}files/{id/1000}/{id%1000}/{文件名}`
    #[test]
    fn fix_download_url_when_missing() {
        let mut obj = make_file(5152805, "jei-1.20.4-forge-15.3.0.4.jar", Some("aaa"));
        assert!(obj.download_url.is_none());

        obj.fix_download_url();

        assert_eq!(
            obj.download_url.as_deref(),
            Some("https://edge.forgecdn.net/files/5152/805/jei-1.20.4-forge-15.3.0.4.jar")
        );
    }

    /// fix_download_url：已有下载地址时不应覆盖
    #[test]
    fn fix_download_url_keeps_existing() {
        let mut obj = make_file(1000, "a.jar", None);
        obj.download_url = Some(String::from("https://example.com/a.jar"));

        obj.fix_download_url();

        assert_eq!(
            obj.download_url.as_deref(),
            Some("https://example.com/a.jar")
        );
    }

    /// sha1_hash：algo == 1 的哈希即 SHA1
    #[test]
    fn sha1_hash_extraction() {
        let mut obj = make_file(1, "a.jar", None);
        obj.hashes = vec![
            HashesObj {
                value: String::from("md5-value"),
                algo: 2,
            },
            HashesObj {
                value: String::from("sha1-value"),
                algo: 1,
            },
        ];
        assert_eq!(obj.sha1_hash(), "sha1-value");

        // 无哈希时返回空字符串
        let obj = make_file(1, "a.jar", None);
        assert_eq!(obj.sha1_hash(), "");
    }
}
