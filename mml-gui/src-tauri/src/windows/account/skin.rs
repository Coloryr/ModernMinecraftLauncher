//! 账户皮肤与披风：贴图列表 / 应用 / 上传 / 去官网
//!
//! 从 `account/mod.rs` 拆出来的。贴图来自账户档案的 Mojang skin / cape 两项，
//! 装备走 Mojang services 的换肤 / 显示披风接口，需要该账户自己的 OAuth 令牌。
//! 命令带分组标记，值填原始组名 `account`，免得组键随文件名变掉，见 AGENTS.md §4。

use std::path::Path;

use mml_auth::{AuthType, auths};
use mml_net::mojang_api::{self, SkinObj};
use mml_net::urls;
use mml_sys::open_helper;
use tauri::AppHandle;

use crate::dtos::account_dto::auth_type_from_str;
use crate::dtos::skin_dto::{TextureItemDto, TexturesDto};
use crate::err_keys;

use super::emit_account_change;
use super::oauth::get_oauth_profile;

/// 查询账户的皮肤 / 披风纹理列表
///
/// OAuth 账户走 minecraft services 档案，多皮肤 + 多披风，`active` 是服务端当前选中项；
/// 其余在线账户走会话服务器，只有单皮肤 + 单披风且恒为 `active`；离线账户返回空列表，
/// 前端走占位图。取到的纹理同时按内容 SHA1 缓存并登记源 URL，前端拿到 sha1 后经
/// `mml-image` 的 sha1 型 URI 取图。
///
/// - `account_type`: 前端账户类型串，见 `auth_type_from_str`
/// - `uuid`: 账户 UUID
///
/// # 返回值
///
/// 皮肤与披风两个列表，离线账户两者都为空；账户不存在返回 `err.accountMissing`
#[gui_macros::ipc_group("account")]
#[gui_macros::command]
pub async fn account_get_textures(
    account_type: String,
    uuid: String,
) -> Result<TexturesDto, String> {
    let auth_type = auth_type_from_str(&account_type);
    if auth_type == AuthType::Offline {
        return Ok(TexturesDto::default());
    }

    let Some(mut auth) = auths::get(&uuid, auth_type) else {
        return Err(String::from(err_keys::ACCOUNT_MISSING));
    };

    if auth_type.is_oauth() {
        let profile = get_oauth_profile(&mut auth)
            .await
            .map_err(|err| err.to_string())?;
        let skins = textures_from(&profile.skins).await;
        let capes = textures_from(&profile.capes).await;
        return Ok(TexturesDto { skins, capes });
    }

    // 非 OAuth：会话服务器只有单皮肤 + 单披风
    let res = mml_game::player_skin::download_skin(&auth).await;
    let skins = match &res.skin {
        Some(tex) => vec![TextureItemDto {
            name: auth.uuid.clone(),
            sha1: tex.sha1.clone(),
            model: if res.is_new_slim {
                String::from("slim")
            } else {
                String::new()
            },
            active: true,
        }],
        None => Vec::new(),
    };
    let capes = match &res.cape {
        Some(tex) => vec![TextureItemDto {
            name: auth.uuid.clone(),
            sha1: tex.sha1.clone(),
            model: String::new(),
            active: true,
        }],
        None => Vec::new(),
    };

    if let Some(tex) = &res.skin {
        crate::image_manager::push_texture_url(&tex.sha1, &tex.url);
    }
    if let Some(tex) = &res.cape {
        crate::image_manager::push_texture_url(&tex.sha1, &tex.url);
    }
    Ok(TexturesDto { skins, capes })
}

/// 把 OAuth 档案的纹理列表转成 DTO，并行下载后按内容算 SHA1 并登记源 URL
///
/// 各项并行下载：都是等网络的 IO，串行会随皮肤 / 披风数量线性变慢；
/// 某个纹理下载失败时跳过该项而不整体失败
///
/// - `items`: 档案里的纹理条目，皮肤或披风
///
/// # 返回值
///
/// 只含下载成功条目的 DTO，相对顺序与 `items` 一致
pub(super) async fn textures_from(items: &[SkinObj]) -> Vec<TextureItemDto> {
    let handles: Vec<_> = items
        .iter()
        .map(|item| {
            // 披风项用服务器给的披风名 alias，皮肤项没有 alias，退回条目 UUID
            let name = item.alias.clone().unwrap_or_else(|| item.id.clone());
            let state = item.state.clone();
            let url = item.url.clone();
            let variant = item.variant.clone();
            tokio::spawn(async move {
                let tex = mml_game::player_skin::load_texture(&url).await?;
                crate::image_manager::push_texture_url(&tex.sha1, &url);
                Some(TextureItemDto {
                    name,
                    sha1: tex.sha1,
                    model: variant.to_lowercase(),
                    active: state == "ACTIVE",
                })
            })
        })
        .collect();

    // 按提交顺序收集，保持档案原有排序
    let mut out = Vec::new();
    for handle in handles {
        if let Ok(Some(dto)) = handle.await {
            out.push(dto);
        }
    }
    out
}

/// 装备账户的皮肤 / 披风，正版专用；第三方规范无多纹理接口
///
/// 先重查档案，按内容 SHA1 找到对应条目再调服务端装备接口；
/// 成功后清纹理渲染缓存并广播账户变更，列表预览跟随刷新。
///
/// - `app`: 应用句柄，装备成功后广播账户变更
/// - `account_type`: 前端账户类型串，非正版类型返回 `err.notOauth`
/// - `uuid`: 账户 UUID
/// - `kind`: 纹理种类，`skin` 走换肤接口、`cape` 走显示披风接口
/// - `sha1`: 要装备的纹理内容 SHA1
///
/// # 返回值
///
/// 账户不存在返回 `err.accountMissing`；档案里没有该 SHA1 返回 `err.textureMissing`
pub(super) async fn equip_texture(
    app: &AppHandle,
    account_type: &str,
    uuid: &str,
    kind: &str,
    sha1: &str,
) -> Result<(), String> {
    let auth_type = auth_type_from_str(account_type);
    if !auth_type.is_oauth() {
        return Err(String::from(err_keys::NOT_OAUTH));
    }
    let Some(mut auth) = auths::get(uuid, auth_type) else {
        return Err(String::from(err_keys::ACCOUNT_MISSING));
    };

    let profile = get_oauth_profile(&mut auth)
        .await
        .map_err(|err| err.to_string())?;

    // 按内容 SHA1 匹配条目；load_texture 有进程内 URL→SHA1 缓存，重复查询开销小
    let mut target = None;
    let items = if kind == "skin" {
        &profile.skins
    } else {
        &profile.capes
    };
    for item in items {
        let Some(tex) = mml_game::player_skin::load_texture(&item.url).await else {
            continue;
        };
        if tex.sha1 == sha1 {
            target = Some(item);
            break;
        }
    }
    let Some(item) = target else {
        return Err(String::from(err_keys::TEXTURE_MISSING));
    };

    if kind == "skin" {
        mojang_api::set_minecraft_skin(&auth.access_token, &item.variant.to_lowercase(), &item.url)
            .await
    } else {
        mojang_api::set_minecraft_cape(&auth.access_token, &item.id).await
    }
    .map_err(|err| err.to_string())?;

    crate::image_manager::clear_texture_images();
    emit_account_change(app);
    Ok(())
}

/// 装备账户选中的皮肤，仅正版
///
/// - `app`: 应用句柄，装备成功后广播账户变更
/// - `account_type`: 前端账户类型串，非正版类型返回 `err.notOauth`
/// - `uuid`: 账户 UUID
/// - `sha1`: 要装备的皮肤内容 SHA1
///
/// # 返回值
///
/// 与 [`equip_texture`] 相同：账户不存在 / 纹理不存在时返回对应错误键
#[gui_macros::ipc_group("account")]
#[gui_macros::command]
pub async fn account_set_active_skin(
    app: AppHandle,
    account_type: String,
    uuid: String,
    sha1: String,
) -> Result<(), String> {
    equip_texture(&app, &account_type, &uuid, "skin", &sha1).await
}

/// 装备账户选中的披风，仅正版
///
/// - `app`: 应用句柄，装备成功后广播账户变更
/// - `account_type`: 前端账户类型串，非正版类型返回 `err.notOauth`
/// - `uuid`: 账户 UUID
/// - `sha1`: 要装备的披风内容 SHA1
///
/// # 返回值
///
/// 与 [`equip_texture`] 相同：账户不存在 / 纹理不存在时返回对应错误键
#[gui_macros::ipc_group("account")]
#[gui_macros::command]
pub async fn account_set_active_cape(
    app: AppHandle,
    account_type: String,
    uuid: String,
    sha1: String,
) -> Result<(), String> {
    equip_texture(&app, &account_type, &uuid, "cape", &sha1).await
}

/// 上传本地皮肤文件并装备，正版专用；第三方规范无上传接口
///
/// 文件路径来自前端系统文件对话框，仅支持 PNG；上传成功后服务端
/// 自动装备新皮肤，这里清纹理渲染缓存并广播账户变更。
///
/// - `app`: 应用句柄，上传成功后广播账户变更
/// - `account_type`: 前端账户类型串，非正版类型返回 `err.notOauth`
/// - `uuid`: 账户 UUID
/// - `variant`: 皮肤型号，`classic` 是经典、`slim` 是纤细
/// - `path`: 本地皮肤 PNG 文件路径
///
/// # 返回值
///
/// 型号非法返回 `err.variantInvalid`；文件不是 PNG 返回 `err.notPng`；
/// 账户不存在返回 `err.accountMissing`；读取文件失败返回 IO 错误信息
#[gui_macros::ipc_group("account")]
#[gui_macros::command]
pub async fn account_upload_skin(
    app: AppHandle,
    account_type: String,
    uuid: String,
    variant: String,
    path: String,
) -> Result<(), String> {
    let auth_type = auth_type_from_str(&account_type);
    if !auth_type.is_oauth() {
        return Err(String::from(err_keys::NOT_OAUTH));
    }
    if variant != "classic" && variant != "slim" {
        return Err(String::from(err_keys::VARIANT_INVALID));
    }

    let data = mml_sys::path_helper::read_byte(&path).map_err(|err| err.to_string())?;
    // PNG 文件签名 89 50 4E 47 0D 0A 1A 0A；服务端只收 PNG，提前拦下明显不对的文件
    if !data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Err(String::from(err_keys::NOT_PNG));
    }

    let auth = auths::get(&uuid, auth_type).ok_or(err_keys::ACCOUNT_MISSING)?;
    let file_name = Path::new(&path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("skin")
        .to_string();

    mojang_api::upload_minecraft_skin(&auth.access_token, &variant, &file_name, data)
        .await
        .map_err(|err| err.to_string())?;

    crate::image_manager::clear_texture_images();
    emit_account_change(&app);
    Ok(())
}

/// 打开第三方账户对应皮肤站的网页；皮肤上传与装备在皮肤站的网页面板上完成
///
/// LittleSkin 开官网，统一通行证开对应服务器页，自建皮肤站 / 外置登录开
/// 服务器根地址；外置登录存的是 Yggdrasil API 根，剥掉 `/api/yggdrasil` 后缀
/// 得到站点根。
///
/// - `account_type`: 前端账户类型串，离线与正版返回 `err.notOauth`
/// - `uuid`: 账户 UUID
///
/// # 返回值
///
/// 账户不存在返回 `err.accountMissing`；服务器地址为空 / 不是 http 开头返回 `err.serverEmpty`
#[gui_macros::ipc_group("account")]
#[gui_macros::command]
pub fn account_open_skin_site(account_type: String, uuid: String) -> Result<(), String> {
    let auth_type = auth_type_from_str(&account_type);
    if auth_type == AuthType::Offline || auth_type.is_oauth() {
        return Err(String::from(err_keys::NOT_OAUTH));
    }
    let auth = auths::get(&uuid, auth_type).ok_or(err_keys::ACCOUNT_MISSING)?;
    let server = auth.text1.clone().unwrap_or_default();

    let url = match auth_type {
        AuthType::LittleSkin => urls::LITTLE_SKIN_URL.to_string(),
        AuthType::Nide8 => format!("{}{}", urls::NIDE8_URL, server),
        AuthType::AuthlibInjector => server
            .trim_end_matches('/')
            .trim_end_matches("/api/yggdrasil")
            .to_string(),
        AuthType::SelfLittleSkin => server.trim_end_matches('/').to_string(),
        _ => return Err(String::from(err_keys::NOT_OAUTH)),
    };
    if !url.starts_with("http") {
        return Err(String::from(err_keys::SERVER_EMPTY));
    }
    open_helper::open_url(&url);
    Ok(())
}
