//! Mojang API
//!
//! 提供与 Mojang 官方 API 交互的功能，包括：
//!
//! - 获取 Minecraft 版本列表
//! - 获取玩家档案和皮肤信息
//! - Xbox Live → Minecraft Token 认证
//! - Minecraft 官方新闻
//!
//! # 认证相关
//!
//! 本模块中的 [`get_minecraft_profile`] 和 [`get_minecraft_token`]
//! 是 Microsoft OAuth 认证流程的最后两步。

use mml_config::config_obj::SourceLocal;
use mml_names::i18_items::error_type::{CoreResult, ErrorData, ErrorType, HttpErrorData};

use reqwest::{Method, Request, Url, header::HeaderValue};
use serde::{Deserialize, Serialize};

use crate::{url_helper, urls};

/// 直接下载资源
///
/// - `url`: 资源下载地址
///
/// # 返回值
///
/// 返回资源内容字节
pub async fn get_assets(url: &String) -> CoreResult<Vec<u8>> {
    crate::get_work_client().get_bytes(url).await
}

/// 获取主版本列表
///
/// - `source`: 下载源，`None` 时取当前配置
///
/// # 返回值
///
/// 返回版本清单 JSON 原文字节
pub async fn get_versions(source: Option<SourceLocal>) -> CoreResult<Vec<u8>> {
    let url = url_helper::game_version(source);
    crate::get_work_client().get_bytes(&url).await
}

/// 档案里的单件皮肤 / 披风
///
/// 皮肤项只有 `variant`；披风项多一个 `alias`，即披风名，如 `Migrator`
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct SkinObj {
    /// 纹理条目 UUID，对应档案 `skins[]` / `capes[]` 的 `id`
    pub id: String,
    /// 选中状态，取值 `ACTIVE` 或空串，对应 `state`
    pub state: String,
    /// 贴图下载地址，对应 `url`
    pub url: String,
    /// 皮肤型号，取值 `CLASSIC` / `SLIM`，披风项为空串，对应 `variant`
    pub variant: String,
    /// 披风名，仅披风项有，对应 `alias`
    pub alias: Option<String>,
}

impl Default for SkinObj {
    fn default() -> Self {
        Self {
            id: Default::default(),
            state: Default::default(),
            url: Default::default(),
            variant: Default::default(),
            alias: Default::default(),
        }
    }
}

/// Minecraft 玩家档案，对应 `MINECRAFT_SERVICES` 的响应体
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct MinecraftProfileObj {
    /// 玩家 UUID，对应 `id`
    pub id: String,
    /// 玩家名，对应 `name`
    pub name: String,
    /// 拥有的皮肤列表，对应 `skins`
    pub skins: Vec<SkinObj>,
    /// 拥有的披风列表，对应 `capes`
    pub capes: Vec<SkinObj>,
}

impl Default for MinecraftProfileObj {
    fn default() -> Self {
        Self {
            id: Default::default(),
            name: Default::default(),
            skins: Default::default(),
            capes: Default::default(),
        }
    }
}

/// 用户档案属性项，对应会话服务器 `properties[]` 的元素
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct UserProfilePropertiesObj {
    /// 属性值，对应 `value`，是 Base64 编码的皮肤 / 披风纹理信息
    pub value: String,
}

impl Default for UserProfilePropertiesObj {
    fn default() -> Self {
        Self {
            value: Default::default(),
        }
    }
}

/// 用户档案，含纹理属性，对应会话服务器的响应体
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct UserProfileObj {
    /// 档案属性列表，对应 `properties`
    pub properties: Vec<UserProfilePropertiesObj>,
}

impl Default for UserProfileObj {
    fn default() -> Self {
        Self {
            properties: Default::default(),
        }
    }
}

/// 获取账户信息
///
/// - `token`: 登陆 Token
///
/// # 返回值
///
/// 返回当前账号的 Minecraft 档案，含 UUID 与玩家名
pub async fn get_minecraft_profile(token: &str) -> CoreResult<MinecraftProfileObj> {
    let client = crate::get_login_client();
    let mut req = Request::new(Method::GET, Url::parse(urls::MINECRAFT_SERVICES).unwrap());
    req.headers_mut().insert(
        "Authorization",
        HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
    );
    let res = client.send(req).await?;
    let data = crate::handle_response::<MinecraftProfileObj>(res).await?;

    Ok(data)
}

/// 向玩家配置接口发送带 Bearer 的 JSON 请求，成功时不解析响应体
///
/// 换肤 / 换披风接口成功时返回空负载 204 或无需使用的 profile，
/// 这里只关心是否 2xx；失败时把服务器的错误 JSON 原文带进错误里。
///
/// - `method`: 请求方法，`POST` 或 `PUT`
/// - `path`: 相对 `MINECRAFT_SERVICES` 的路径，如 `/skins`
/// - `token`: Minecraft 访问令牌
/// - `body`: JSON 请求体
async fn send_profile_json<T: Serialize>(
    method: Method,
    path: &str,
    token: &str,
    body: &T,
) -> CoreResult<()> {
    let client = crate::get_login_client();
    let mut req = Request::new(
        method,
        Url::parse(&format!("{}{path}", urls::MINECRAFT_SERVICES)).unwrap(),
    );
    req.headers_mut().insert(
        "Authorization",
        HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
    );
    let body = serde_json::to_vec(body)
        .map_err(|err| {
            ErrorType::SerializerError(ErrorData {
                error: err.to_string(),
            })
        })?
        .into();
    *req.body_mut() = Some(body);
    let res = client.send(req).await?;

    let status = res.status();
    if !status.is_success() {
        let url = res.url().to_string();
        let error = res.text().await.unwrap_or_default();
        return Err(ErrorType::HttpError(HttpErrorData {
            error,
            url,
            status: Some(status.as_u16()),
        }));
    }

    Ok(())
}

/// 更换玩家皮肤，装备档案里已有的皮肤，传该皮肤条目的 URL 与型号
///
/// - `token`: Minecraft 访问令牌
/// - `variant`: 皮肤型号，取值 `classic` / `slim`
/// - `url`: 皮肤纹理 URL，取档案 `skins[].url`
///
/// # 返回值
///
/// 成功返回 `Ok(())`；令牌无效 / 不拥有该皮肤等返回对应错误
pub async fn set_minecraft_skin(token: &str, variant: &str, url: &str) -> CoreResult<()> {
    send_profile_json(
        Method::POST,
        "/skins",
        token,
        &serde_json::json!({ "variant": variant, "url": url }),
    )
    .await
}

/// 显示指定披风，装备档案里已有的披风
///
/// - `token`: Minecraft 访问令牌
/// - `cape_id`: 披风 UUID，取档案 `capes[].id`
///
/// # 返回值
///
/// 成功返回 `Ok(())`；不拥有该披风，即 400，返回对应错误
pub async fn set_minecraft_cape(token: &str, cape_id: &str) -> CoreResult<()> {
    send_profile_json(
        Method::PUT,
        "/capes/active",
        token,
        &serde_json::json!({ "capeId": cape_id }),
    )
    .await
}

/// 上传本地皮肤文件，仅正版账号可用
///
/// 皮肤文件是 multipart 表单直传，表单字段为 `variant` 与 `file`，
/// 与按 URL 换肤 [`set_minecraft_skin`] 走同一接口的不同形态，成功返回空负载。
///
/// - `token`: Minecraft 访问令牌
/// - `variant`: 皮肤型号，取值 `classic` 经典或 `slim` 纤细
/// - `file_name`: 文件名，服务端记录来源用，取本地文件名即可
/// - `data`: PNG 文件字节
pub async fn upload_minecraft_skin(
    token: &str,
    variant: &str,
    file_name: &str,
    data: Vec<u8>,
) -> CoreResult<()> {
    let form = reqwest::multipart::Form::new()
        .text("variant", variant.to_string())
        .part(
            "file",
            reqwest::multipart::Part::bytes(data)
                .file_name(file_name.to_string())
                .mime_str("image/png")
                .map_err(|err| {
                    ErrorType::SerializerError(ErrorData {
                        error: err.to_string(),
                    })
                })?,
        );

    let res = crate::get_login_client()
        .post_multipart(&format!("{}/skins", urls::MINECRAFT_SERVICES), form, token)
        .await?;

    let status = res.status();
    if !status.is_success() {
        let url = res.url().to_string();
        let error = res.text().await.unwrap_or_default();
        return Err(ErrorType::HttpError(HttpErrorData {
            error,
            url,
            status: Some(status.as_u16()),
        }));
    }
    Ok(())
}

/// 获取皮肤信息
///
/// - `uuid`: 玩家 UUID
/// - `url`: 查询地址，`None` 时用官方会话服务器
///
/// # 返回值
///
/// 返回用户档案，属性中含皮肤 / 披风纹理
pub async fn get_user_profile(uuid: &str, url: Option<&str>) -> CoreResult<UserProfileObj> {
    let url = match url {
        Some(data) => data.to_string(),
        // 官方会话服务器只认不带连字符的 UUID，带连字符会返回 400，
        // 而账户数据里存的是常规带连字符格式，这里统一去掉
        None => format!(
            "{}/{}",
            urls::MINECRAFT_SESSION_SERVER,
            uuid.replace('-', "")
        ),
    };
    crate::get_login_client()
        .get_json::<UserProfileObj>(&url)
        .await
}

/// 按玩家名查 UUID 档案，查不到玩家时接口返回 204，这里同样报 `DataNotFound`
///
/// - `name`: 玩家名
///
/// # 返回值
///
/// 返回玩家档案，含 UUID 与玩家名
pub async fn get_profile_by_name(name: &str) -> CoreResult<MinecraftProfileObj> {
    let url = format!("{}/{name}", urls::MINECRAFT_PROFILE_API);
    crate::get_login_client()
        .get_json::<MinecraftProfileObj>(&url)
        .await
}

/// Minecraft Token 请求体，对应 `MINECRAFT_SERVICES_XBOX` 的请求体
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct MinecraftTokenObj {
    /// Xbox 身份令牌，对应 `identityToken`，形如 `XBL3.0 x=<uhs>;<token>`
    #[serde(rename = "identityToken")]
    pub identity_token: String,
}

impl Default for MinecraftTokenObj {
    fn default() -> Self {
        Self {
            identity_token: Default::default(),
        }
    }
}

/// Minecraft Token 响应，对应 `MINECRAFT_SERVICES_XBOX` 的响应体
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct MinecraftTokenResObj {
    /// 访问令牌，对应 `access_token`
    pub access_token: String,
    /// 令牌有效期，单位秒，对应 `expires_in`
    pub expires_in: i64,
}

impl Default for MinecraftTokenResObj {
    fn default() -> Self {
        Self {
            access_token: Default::default(),
            expires_in: Default::default(),
        }
    }
}

/// 从 XBOX 登陆获取账户认证
///
/// - `uhs`: Xbox 用户哈希
/// - `token`: Xbox Live 令牌
///
/// # 返回值
///
/// 返回 Minecraft 访问令牌及其有效期，单位秒，本地判过期用；
/// 令牌无效时返回 `AuthTokenTimeout`
pub async fn get_minecraft_token(uhs: &str, token: &str) -> CoreResult<(String, i64)> {
    let obj = MinecraftTokenObj {
        identity_token: format!("XBL3.0 x={uhs};{token}"),
    };

    let res = crate::get_login_client()
        .post_json_get_json::<_, MinecraftTokenResObj>(urls::MINECRAFT_SERVICES_XBOX, &obj)
        .await?;

    if res.expires_in <= 0 || res.access_token.is_empty() {
        Err(ErrorType::AuthTokenTimeout)
    } else {
        Ok((res.access_token, res.expires_in))
    }
}

/// 新闻配图，对应新闻接口的 `default_tile.image`
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct ImageObj {
    /// 图片类型，对应 `content_type`
    pub content_type: String,
    /// 图片 URL，对应 `imageURL`
    #[serde(rename = "imageURL")]
    pub image_url: String,
    /// 图片说明，对应 `alt`
    pub alt: String,
}

impl Default for ImageObj {
    fn default() -> Self {
        Self {
            content_type: Default::default(),
            image_url: Default::default(),
            alt: Default::default(),
        }
    }
}

/// 新闻卡片，对应 `article_grid[]` 的 `default_tile`
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct DefaultTileObj {
    /// 新闻标题，对应 `title`
    pub title: String,
    /// 副标题，对应 `sub_header`
    pub sub_header: String,
    /// 卡片尺寸标识，对应 `tile_size`，决定前端占几格
    pub tile_size: String,
    /// 卡片配图，对应 `image`
    pub image: ImageObj,
}

impl Default for DefaultTileObj {
    fn default() -> Self {
        Self {
            title: Default::default(),
            sub_header: Default::default(),
            tile_size: Default::default(),
            image: Default::default(),
        }
    }
}

/// 单条新闻，对应 `article_grid[]` 的元素
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct ArticleGridObj {
    /// 新闻卡片内容，对应 `default_tile`
    pub default_tile: DefaultTileObj,
    /// 主分类，对应 `primary_category`
    pub primary_category: String,
    /// 新闻页 URL，对应 `article_url`
    pub article_url: String,
}

impl Default for ArticleGridObj {
    fn default() -> Self {
        Self {
            default_tile: Default::default(),
            primary_category: Default::default(),
            article_url: Default::default(),
        }
    }
}

/// Minecraft 官方新闻，对应 `MINECRAFT_NEWS` 的响应体
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct MinecraftNewsObj {
    /// 新闻列表，对应 `article_grid`
    pub article_grid: Vec<ArticleGridObj>,
}

impl Default for MinecraftNewsObj {
    fn default() -> Self {
        Self {
            article_grid: Default::default(),
        }
    }
}

/// 获取 Minecraft 新闻
///
/// - `page`: 新闻页码，从 1 开始
///
/// # 返回值
///
/// 返回该页新闻列表
pub async fn get_minecraft_news(page: u32) -> CoreResult<MinecraftNewsObj> {
    let url = format!("{}{page}.json", urls::MINECRAFT_NEWS);

    crate::get_work_client()
        .get_json::<MinecraftNewsObj>(&url)
        .await
}
