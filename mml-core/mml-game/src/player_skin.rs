//! 获取玩家皮肤
//!
//! 按账户认证类型向对应的会话服务器查询档案，解析 profile 属性里的
//! 皮肤 / 披风地址，并缓存到本地皮肤目录。

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{LazyLock, RwLock},
};

use mml_auth::{AuthType, LoginObj};
use mml_base::hash_helper;
use mml_names::i18_items::error_type::{CoreResult, ErrorType, SkinBlockErrorData};
use mml_net::{mojang_api, urls};
use serde::Deserialize;

use crate::launcher_path::assets_path;

/// 皮肤数据，来自 profile 属性 base64 解码后的 JSON
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct MinecraftTexturesObj {
    /// 档案所属玩家名，皮肤方块的显示名来源；可缺省
    #[serde(rename = "profileName")]
    profile_name: Option<String>,
    /// 皮肤与披风纹理，两者都可缺省
    textures: TexturesObj,
}

/// 皮肤与披风地址
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct TexturesObj {
    /// 皮肤纹理；没有皮肤时缺省
    #[serde(rename = "SKIN")]
    skin: Option<TextureObj>,
    /// 披风纹理；没有披风时缺省
    #[serde(rename = "CAPE")]
    cape: Option<TextureObj>,
}

/// 单个纹理条目
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct TextureObj {
    /// 纹理下载地址
    url: String,
    /// 纹理元数据；可缺省
    metadata: Option<TextureMetadataObj>,
}

/// 皮肤元数据，`model` 为 `slim` 表示纤细皮肤
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct TextureMetadataObj {
    /// 皮肤模型，`slim` 表示纤细皮肤，`classic` 表示经典皮肤
    model: String,
}

/// 单个纹理的本地缓存，按内容 SHA1 寻址，皮肤与披风共用一套存储
#[derive(Debug, Clone)]
pub struct TextureFile {
    /// 本地缓存文件位置
    pub path: PathBuf,
    /// 内容 SHA1，十六进制小写，与游戏 assets objects 命名一致
    pub sha1: String,
    /// 纹理源地址，供上层登记；本地文件被清后按此回源
    pub url: String,
}

/// 皮肤下载结果
#[derive(Debug, Default)]
pub struct DownloadSkinRes {
    /// 皮肤文件；没有皮肤时为 `None`
    pub skin: Option<TextureFile>,
    /// 披风文件；没有披风时为 `None`
    pub cape: Option<TextureFile>,
    /// 皮肤模型是否为纤细 slim
    pub is_new_slim: bool,
}

/// URL → 内容 SHA1 映射：同进程内同一 URL 只下载一次
static URL_HASH: LazyLock<RwLock<HashMap<String, String>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 下载皮肤与披风
///
/// - `obj`: 账户信息
///
/// 按认证类型查询对应的会话服务器，取回皮肤 / 披风文件并缓存到本地皮肤目录。
///
/// # 返回值
///
/// 下载结果；查询失败返回全空的默认值
pub async fn download_skin(obj: &LoginObj) -> DownloadSkinRes {
    let url = match obj.auth_type {
        AuthType::Offline | AuthType::OAuth => None,
        AuthType::Nide8 => Some(format!(
            "{}{}/sessionserver/session/minecraft/profile/{}",
            urls::NIDE8_URL,
            obj.text1.clone().unwrap_or_default(),
            obj.uuid
        )),
        AuthType::AuthlibInjector => Some(format!(
            "{}/sessionserver/session/minecraft/profile/{}",
            obj.text1.clone().unwrap_or_default(),
            obj.uuid
        )),
        AuthType::LittleSkin => Some(format!(
            "{}api/yggdrasil/sessionserver/session/minecraft/profile/{}",
            urls::LITTLE_SKIN_URL,
            obj.uuid
        )),
        AuthType::SelfLittleSkin => Some(format!(
            "{}/api/yggdrasil/sessionserver/session/minecraft/profile/{}",
            obj.text1.clone().unwrap_or_default(),
            obj.uuid
        )),
    };

    let Some(textures) = load_textures(obj, url.as_deref()).await else {
        return DownloadSkinRes::default();
    };

    let is_new_slim = textures
        .textures
        .skin
        .as_ref()
        .and_then(|skin| skin.metadata.as_ref())
        .map(|metadata| metadata.model == "slim")
        .unwrap_or(false);

    let skin = match &textures.textures.skin {
        Some(data) => load_texture(&data.url).await,
        None => None,
    };
    let cape = match &textures.textures.cape {
        Some(data) => load_texture(&data.url).await,
        None => None,
    };

    DownloadSkinRes {
        skin,
        cape,
        is_new_slim,
    }
}

/// 查询档案并解析皮肤数据
///
/// - `obj`: 账户信息
/// - `url`: 会话服务器地址，`None` 时用官方服务器
///
/// # 返回值
///
/// 解析出的皮肤数据；查询或解析失败返回 `None`
async fn load_textures(obj: &LoginObj, url: Option<&str>) -> Option<MinecraftTexturesObj> {
    let res = mojang_api::get_user_profile(&obj.uuid, url).await.ok()?;
    let value = res.properties.first()?.value.clone();

    parse_textures(&value)
}

/// 输入是否为 UUID，带不带横线都认
///
/// - `input`: 待检查的字符串
///
/// # 返回值
///
/// 是否为合法 UUID 格式
fn is_uuid(input: &str) -> bool {
    let plain = input.replace('-', "");
    plain.len() == 32 && plain.chars().all(|c| c.is_ascii_hexdigit())
}

/// 按玩家名或 UUID 取到的皮肤
pub struct SkinFetch {
    /// 玩家名，以档案里的 `profileName` 为准；档案没给名字时是传入的 UUID
    pub name: String,
    /// 皮肤图片的本地缓存路径，由 [`load_texture`] 按内容 SHA1 落盘
    pub path: PathBuf,
}

/// 按用户名或 UUID 取玩家皮肤，走官方会话服务器
///
/// - `input`: 玩家名或 UUID
///
/// 输入是 UUID 时直接查档案，否则先经 Mojang API 按名查 UUID。
///
/// # 返回值
///
/// 见 [`SkinFetch`]；
/// 玩家不存在 / 档案没有皮肤 / 皮肤下载失败都报 DataNotFound
pub async fn fetch_skin_by_input(input: &str) -> CoreResult<SkinFetch> {
    let input = input.trim();
    let not_found = || ErrorType::SkinBlockError(SkinBlockErrorData::PlayerNotFound);

    // UUID 直接用；玩家名先查 UUID，档案里的 profileName 才是权威名字
    let (uuid, name) = if is_uuid(input) {
        (input.to_string(), None)
    } else {
        let profile = mojang_api::get_profile_by_name(input)
            .await
            .map_err(|_| not_found())?;
        (profile.id, Some(profile.name))
    };

    let res = mojang_api::get_user_profile(&uuid, None)
        .await
        .map_err(|_| not_found())?;
    let value = res.properties.first().ok_or_else(not_found)?.value.clone();
    let textures = parse_textures(&value).ok_or_else(not_found)?;
    let skin = textures.textures.skin.ok_or_else(not_found)?;
    let file = load_texture(&skin.url).await.ok_or_else(not_found)?;

    let name = textures
        .profile_name
        .or(name)
        .unwrap_or_else(|| uuid.clone());
    Ok(SkinFetch {
        name,
        path: file.path,
    })
}

/// 解析 profile 属性里的皮肤数据，属性值是 base64 后的 JSON
///
/// - `value`: base64 编码的属性值
///
/// # 返回值
///
/// 解析出的皮肤数据；解码或解析失败返回 `None`
fn parse_textures(value: &str) -> Option<MinecraftTexturesObj> {
    let data = hash_helper::de_base64(value).ok()?;

    serde_json::from_str::<MinecraftTexturesObj>(&data).ok()
}

/// 取回单个皮肤 / 披风纹理，按内容 SHA1 缓存
///
/// - `url`: 纹理下载地址
///
/// 下载后按内容定键，布局与游戏 assets objects 相同；同一份贴图不论来自哪个
/// URL / 账户都只存一份，同进程内同 URL 只下载一次。
///
/// # 返回值
///
/// 本地文件与内容 SHA1；下载或写入失败返回 `None`
pub async fn load_texture(url: &str) -> Option<TextureFile> {
    let url = url.trim();
    if url.is_empty() {
        return None;
    }

    // 先查 URL→SHA1 映射：命中且文件还在就直接用，不发请求
    if let Some(sha1) = URL_HASH.read().unwrap().get(url) {
        let file = assets_path::get_skin_object(sha1);
        if file.exists() {
            return Some(TextureFile {
                path: file,
                sha1: sha1.clone(),
                url: url.to_string(),
            });
        }
    }

    let data = mojang_api::get_assets(&url.to_string()).await.ok()?;
    let sha1 = assets_path::save_skin_object(&data).ok()?;
    URL_HASH
        .write()
        .unwrap()
        .insert(url.to_string(), sha1.clone());

    Some(TextureFile {
        path: assets_path::get_skin_object(&sha1),
        sha1,
        url: url.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 带纤细皮肤与披风的 textures 属性
    const SLIM: &str = "eyJ0aW1lc3RhbXAiOjE2MDAwMDAwMDAwMDAsInByb2ZpbGVJZCI6IjA2OWE3OWY0NDRlOTQ3MjZhNWJlZmNhOTBlMzhhYWY1IiwicHJvZmlsZU5hbWUiOiJOb3RjaCIsInRleHR1cmVzIjp7IlNLSU4iOnsidXJsIjoiaHR0cDovL3RleHR1cmVzLm1pbmVjcmFmdC5uZXQvdGV4dHVyZS9hYmMiLCJtZXRhZGF0YSI6eyJtb2RlbCI6InNsaW0ifX0sIkNBUEUiOnsidXJsIjoiaHR0cDovL3RleHR1cmVzLm1pbmVjcmFmdC5uZXQvdGV4dHVyZS9jYXBlIn19fQ==";
    /// 只有 timestamp 等字段、没有任何纹理的 textures 属性
    const EMPTY: &str = "eyJ0aW1lc3RhbXAiOjE2MDAwMDAwMDAwMDAsInByb2ZpbGVJZCI6IjA2OWE3OWY0NDRlOTQ3MjZhNWJlZmNhOTBlMzhhYWY1IiwicHJvZmlsZU5hbWUiOiJOb3RjaCIsInRleHR1cmVzIjp7fX0=";

    /// 应解析出皮肤与披风地址，皮肤带 slim 标记
    #[test]
    fn test_parse_textures_slim() {
        let obj = parse_textures(SLIM).expect("应能解析");

        let skin = obj.textures.skin.expect("应有皮肤");
        assert_eq!(skin.url, "http://textures.minecraft.net/texture/abc");
        assert_eq!(skin.metadata.expect("应有元数据").model, "slim");

        let cape = obj.textures.cape.expect("应有披风");
        assert_eq!(cape.url, "http://textures.minecraft.net/texture/cape");
    }

    /// 没有纹理时皮肤与披风都应为 `None`
    #[test]
    fn test_parse_textures_empty() {
        let obj = parse_textures(EMPTY).expect("应能解析");

        assert!(obj.textures.skin.is_none());
        assert!(obj.textures.cape.is_none());
    }

    /// 非法 base64 / 非法 JSON 应返回 `None`
    #[test]
    fn test_parse_textures_invalid() {
        assert!(parse_textures("!!!").is_none());
        assert!(parse_textures("aGVsbG8=").is_none());
    }
}
