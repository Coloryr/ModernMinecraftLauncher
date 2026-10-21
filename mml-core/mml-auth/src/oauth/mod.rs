//! Microsoft OAuth 2.0 认证模块
//!
//! 本模块实现了 Microsoft 正版 Minecraft 的完整 OAuth 2.0 认证流程。
//! 由于 Minecraft Java 版已迁移至微软账户体系，登录需要经过以下认证链：
//!
//! # 认证流程
//!
//! ```text
//! Microsoft OAuth 设备码授权
//!     │
//!     ▼
//! 获取 Microsoft Token ───► refresh_token 可用于续期
//!     │
//!     ▼
//! Xbox Live 认证，换取 XBL Token
//!     │
//!     ▼
//! XSTS 认证，换取 XSTS Token
//!     │
//!     ▼
//! Minecraft 服务认证，换取 Minecraft Token
//!     │
//!     ▼
//! 获取 Minecraft 玩家 Profile，即用户名 + UUID
//! ```
//!
//! # 子模块
//!
//! - [`oauth_obj`] — OAuth 请求/响应的数据结构
//! - [`xbox_obj`] — Xbox Live/XSTS 认证的数据结构
//!
//! # 认证状态
//!
//! [`AuthState`] 枚举表示了认证流程中的各个阶段：
//! `OAuth` → `XBox` → `XSTS` → `Token` → `Profile`

use std::{sync::OnceLock, time::Duration};

use chrono::Local;
use mml_names::i18_items::error_type::{CoreResult, ErrorData, ErrorType, HttpErrorData};
use mml_net::{FormField, mojang_api, urls};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

use crate::{
    LoginObj,
    oauth::{
        oauth_obj::{OAuthGetCodeObj, OAuthGetCodeResObj, OAuthObj},
        xbox_obj::{
            XBoxLiveResObj, XBoxLoginObj, XBoxLoginPropertiesObj, XBoxLoginResObj, XSTSLoginObj,
            XSTSLoginPropertiesObj,
        },
    },
};

pub mod oauth_obj;
pub mod xbox_obj;

/// OAuth 客户端密钥，即 Azure 应用程序 ID，启动时通过 `set_key()` 设置
static KEY: OnceLock<String> = OnceLock::new();

/// 微软认证流程中的当前阶段
///
/// 用于在 UI 中展示认证进度。
pub enum AuthState {
    /// 正在进行 Microsoft OAuth 设备码授权
    OAuth,
    /// 正在进行 Xbox Live 认证
    XBox,
    /// 正在进行 XSTS 认证，XSTS 即 Xbox Secure Token Service
    XSTS,
    /// 正在获取 Minecraft 服务令牌
    Token,
    /// 正在获取 Minecraft 玩家档案
    Profile,
}

/// 设置 OAuth 客户端密钥
///
/// 应在程序启动时调用，设置 Azure 应用程序的客户端 ID。
///
/// - `key`: Azure 应用程序注册 ID
pub fn set_key(key: &str) {
    KEY.get_or_init(|| key.to_string());
}

/// 获取已设置的 OAuth 客户端密钥
///
/// # 返回值
///
/// 成功时返回密钥字符串，未设置时返回 `ErrorType::KeyIsNull`
fn have_key() -> CoreResult<String> {
    match KEY.get() {
        None => Err(ErrorType::KeyIsNull),
        Some(key) => Ok(key.clone()),
    }
}

/// 发起 OAuth 设备码授权——第一步：获取设备码与验证网址，
/// 用户在浏览器中打开网址并输入设备码完成授权
///
/// # 返回值
///
/// 返回 `OAuthGetCodeResObj`，包含：
/// - `code`: 用户需要输入的设备码
/// - `url`: 用户需要访问的验证网址
/// - `device_code`: 后续轮询用的设备码
/// - `expires_in`: 设备码的有效期，单位秒
pub async fn get_code() -> CoreResult<OAuthGetCodeResObj> {
    let key = have_key()?;

    let obj = &[
        FormField {
            key: "client_id",
            value: key.as_str(),
        },
        FormField {
            key: "scope",
            value: "XboxLive.signin offline_access",
        },
    ];

    let data = mml_net::get_login_client()
        .post_form_get_json::<OAuthObj>(urls::OAUTH_CODE, obj)
        .await?;

    match data.error {
        Some(err) => Err(ErrorType::OAuthGetTokenError(ErrorData { error: err })),
        None => Ok(OAuthGetCodeResObj {
            code: data.user_code,
            url: data.verification_uri,
            device_code: data.device_code,
            expires_in: data.expires_in,
        }),
    }
}

/// 轮询等待用户完成设备码授权——第二步：获取 Microsoft Token，
/// 直到用户完成授权、超时或被取消，初始间隔 2 秒，收到 slow_down 加 5 秒
///
/// - `res`: 第一步返回的设备码信息
/// - `cancel`: 取消令牌，用于用户主动终止等待
///
/// # 返回值
///
/// 成功时返回包含 `access_token` 和 `refresh_token` 的 `OAuthGetCodeObj`
pub async fn run_get_code(
    res: &OAuthGetCodeResObj,
    cancel: &CancellationToken,
) -> CoreResult<OAuthGetCodeObj> {
    let key = have_key()?;

    let obj = &[
        FormField {
            key: "client_id",
            value: key.as_str(),
        },
        FormField {
            key: "grant_type",
            value: "urn:ietf:params:oauth:grant-type:device_code",
        },
        FormField {
            key: "device_code",
            value: res.device_code.as_str(),
        },
    ];

    let start_time = Local::now().timestamp();
    let mut delay = 2;

    loop {
        sleep(Duration::from_secs(delay)).await;
        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        let estimated_time = Local::now().timestamp() - start_time;
        if estimated_time > res.expires_in {
            return Err(ErrorType::TaskTimeout);
        }

        // 令牌端点在用户尚未授权时返回 400 + `authorization_pending`，属正常中间态，
        // 不能用 post_form_get_json，非 2xx 直接报错，须拿原始响应自行解析 body
        let resp = mml_net::get_login_client()
            .post_form_get_req(urls::OAUTH_TOKEN, obj)
            .await?;
        let text = resp.text().await.map_err(|err| {
            ErrorType::HttpError(HttpErrorData {
                error: err.to_string(),
                url: urls::OAUTH_TOKEN.to_string(),
                status: None,
            })
        })?;
        let data: OAuthGetCodeObj = serde_json::from_str(&text)
            .map_err(|err| ErrorType::OAuthGetTokenError(ErrorData::from_error(err)))?;

        if let Some(error) = data.error {
            if error == "authorization_pending" {
                continue;
            } else if error == "slow_down" {
                delay += 5;
            } else {
                // expired_token / invalid_client 等其余错误均终止登录
                return Err(ErrorType::OAuthGetTokenError(ErrorData { error }));
            }
        } else {
            return Ok(data);
        }
    }
}

/// 用保存的 refresh_token 换取新令牌，无需用户重新授权
///
/// - `token`: 之前保存的 refresh_token
///
/// # 返回值
///
/// 成功时返回新的 `OAuthGetCodeObj`，其中含新的 access_token 和 refresh_token
pub async fn refresh_oauth_token(token: &str) -> CoreResult<OAuthGetCodeObj> {
    let key = have_key()?;

    let obj = &[
        FormField {
            key: "client_id",
            value: key.as_str(),
        },
        FormField {
            key: "grant_type",
            value: "refresh_token",
        },
        FormField {
            key: "refresh_token",
            value: token,
        },
    ];

    let data = mml_net::get_login_client()
        .post_form_get_json::<OAuthGetCodeObj>(urls::OAUTH_TOKEN, obj)
        .await?;

    match data.error {
        Some(err) => Err(ErrorType::AuthFail(err)),
        None => Ok(data),
    }
}

/// Xbox Live 认证——第三步：用 Microsoft Token 换取 XBL Token 与用户哈希 UHS
///
/// - `token`: Microsoft OAuth access_token
///
/// # 返回值
///
/// 成功时返回 XBL Token 与用户哈希 UHS，令牌为空时返回 `ErrorType::OAuthGetTokenEmpty`
pub async fn get_xbox(token: &str) -> CoreResult<XBoxLiveResObj> {
    let obj = XBoxLoginObj {
        properties: XBoxLoginPropertiesObj {
            auth_method: "RPS".to_string(),
            site_name: "user.auth.xboxlive.com".to_string(),
            rps_ticket: format!("d={}", token),
        },
        relying_party: "http://auth.xboxlive.com".to_string(),
        token_type: "JWT".to_string(),
    };

    let data = mml_net::get_login_client()
        .post_json_get_json::<_, XBoxLoginResObj>(urls::XBOX_LIVE, &obj)
        .await?;
    let item = &data.display_claims.xui[0];
    let token = data.token;
    let uhs = item.uhs.clone();

    if token.is_empty() || uhs.is_empty() {
        Err(ErrorType::OAuthGetTokenEmpty)
    } else {
        Ok(XBoxLiveResObj {
            xbl_token: token,
            xbl_uhs: uhs,
        })
    }
}

/// XSTS 认证——第四步：用 XBL Token 换取 XSTS Token 与用户哈希 UHS
///
/// - `token`: XBL token
///
/// # 返回值
///
/// 成功时返回 XSTS Token 与用户哈希 UHS，令牌为空时返回 `ErrorType::OAuthGetTokenEmpty`
pub async fn get_xsts(token: &str) -> CoreResult<XBoxLiveResObj> {
    let obj = XSTSLoginObj {
        properties: XSTSLoginPropertiesObj {
            sandbox_id: "RETAIL".to_string(),
            user_tokens: vec![token.to_string()],
        },
        relying_party: "rp://api.minecraftservices.com/".to_string(),
        token_type: "JWT".to_string(),
    };

    let data = mml_net::get_login_client()
        .post_json_get_json::<_, XBoxLoginResObj>(urls::XSTS, &obj)
        .await?;
    let item = &data.display_claims.xui[0];
    let token = data.token;
    let uhs = item.uhs.clone();

    if token.is_empty() || uhs.is_empty() {
        Err(ErrorType::OAuthGetTokenEmpty)
    } else {
        Ok(XBoxLiveResObj {
            xbl_token: token,
            xbl_uhs: uhs,
        })
    }
}

impl LoginObj {
    /// 微软正版账户的刷新流程：先用现有 Minecraft Token 快速验证，
    /// 失败则走完整刷新链：refresh_token → Xbox → XSTS → Minecraft Token → Profile
    ///
    /// 快速验证成功时不更新凭据，token 仍有效，`expire_at` 保持原值；
    /// 完整刷新成功后按新令牌的 expires_in 重算 `expire_at`
    ///
    /// - `cancel`: 取消令牌，用于中断异步操作
    ///
    /// # 返回值
    ///
    /// 刷新成功返回 `Ok(())`，此时账户凭据已被更新；认证链任一步失败或被取消时返回相应错误
    pub async fn refresh_oauth(&mut self, cancel: CancellationToken) -> CoreResult<()> {
        let profile = mojang_api::get_minecraft_profile(&self.access_token).await;
        if profile.is_ok() {
            return Ok(());
        }

        // 旧数据可能没有 refresh_token，无法走完整刷新链，提示重新登录
        let Some(refresh_token) = self.text1.clone().filter(|s| !s.is_empty()) else {
            return Err(ErrorType::AuthTokenTimeout);
        };
        let oauth = refresh_oauth_token(&refresh_token).await?;
        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }
        let xbox = get_xbox(&oauth.access_token).await?;
        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }
        let xsts = get_xsts(&xbox.xbl_token).await?;
        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }
        let mc_token = mojang_api::get_minecraft_token(&xsts.xbl_uhs, &xsts.xbl_token).await?;
        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }
        let profile = mojang_api::get_minecraft_profile(&mc_token.access_token).await?;

        self.user_name = profile.name;
        self.uuid = profile.id;
        self.text1 = Some(oauth.refresh_token);
        self.access_token = mc_token.access_token;
        self.set_expire_in(mc_token.expires_in);
        self.last_login = Local::now().fixed_offset();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 注意：以下测试依赖 KEY 全局未被 `set_key()` 初始化，
    // 本测试二进制内不要新增会调用 `set_key()` 的非 ignore 测试。

    /// 未设置 KEY 时，设备码流程第一步应在发起网络请求前失败
    #[tokio::test]
    async fn test_get_code_without_key() {
        let result = get_code().await;
        assert!(matches!(result, Err(ErrorType::KeyIsNull)));
    }

    /// 未设置 KEY 时，refresh_token 刷新也应在发起网络请求前失败
    #[tokio::test]
    async fn test_refresh_token_without_key() {
        let result = refresh_oauth_token("fake-refresh-token").await;
        assert!(matches!(result, Err(ErrorType::KeyIsNull)));
    }
}
