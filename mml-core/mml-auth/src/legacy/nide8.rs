//! 统一通行证 Nide8 登录模块
//!
//! 统一通行证是国内流行的 Minecraft 第三方认证服务，
//! 使用 Yggdrasil 兼容 API，通过服务器 UUID 区分不同的认证服务器节点。

use mml_names::i18_items::error_type::{CoreResult, ErrorType};
use mml_net::urls;
use tokio_util::sync::CancellationToken;

use crate::{
    AuthType, LoginObj,
    legacy::{self},
};

/// 统一通行证登录认证
///
/// - `client_token`: 客户端标识令牌
/// - `user`: 用户名
/// - `password`: 密码
/// - `server`: Nide8 服务器 UUID，用于构建认证 URL `{NIDE8_URL}{server}`
///
/// # 返回值
///
/// 已认证的 [`LoginObj`]，服务器 UUID 存入 `text1`
pub async fn authenticate(
    client_token: String,
    user: String,
    password: String,
    server: String,
) -> CoreResult<LoginObj> {
    let url = String::from(urls::NIDE8_URL) + &server;

    let obj = legacy::authenticate(&url, client_token, user, password, false).await?;

    let mut auth = obj.auth;
    auth.auth_type = AuthType::Nide8;
    auth.text1 = Some(server.clone());

    Ok(auth)
}

impl LoginObj {
    /// 刷新统一通行证登录令牌：先验证有效性，无效则返回超时错误
    ///
    /// - `cancel`: 取消令牌
    ///
    /// # 返回值
    ///
    /// 刷新成功返回 `Ok(())`，此时账户凭据已被更新；令牌失效返回 `ErrorType::AuthTokenTimeout`，被取消时返回取消错误
    pub async fn refresh_nide8(&mut self, cancel: CancellationToken) -> CoreResult<()> {
        let Some(server_id) = self.text1.clone().filter(|s| !s.is_empty()) else {
            return Err(ErrorType::AuthServerNull);
        };
        let server = String::from(urls::NIDE8_URL) + &server_id;

        if legacy::validate(&server, self).await? {
            if cancel.is_cancelled() {
                return Err(ErrorType::TaskCancel);
            }

            Ok(legacy::refresh(&server, self, false).await?)
        } else {
            Err(ErrorType::AuthTokenTimeout)
        }
    }
}
