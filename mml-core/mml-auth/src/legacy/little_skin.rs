//! LittleSkin 皮肤站登录模块
//!
//! LittleSkin 是一个国内的 Minecraft 皮肤站服务，提供基于 Yggdrasil 协议的
//! 账户认证和外置皮肤功能。
//!
//! # 支持的站点类型
//!
//! - **官方 LittleSkin** — 使用 `urls::LITTLE_SKIN_URL` 作为认证服务器
//! - **自建皮肤站** — 用户提供自定义服务器地址，兼容 LittleSkin 的 API 布局

use mml_names::i18_items::error_type::{CoreResult, ErrorType};
use mml_net::urls;
use tokio_util::sync::CancellationToken;

use crate::{
    AuthType, LoginObj,
    legacy::{self, GuiSelectHandel},
};

/// LittleSkin 皮肤站登录认证，支持官方站和自建站两种模式。
/// `server` 为 `None` 时使用官方 LittleSkin
///
/// - `client_token`: 客户端标识令牌
/// - `user`: 用户名
/// - `password`: 密码
/// - `server`: 自建皮肤站地址，`None` 表示使用官方 LittleSkin
/// - `gui`: 可选的角色选择回调
///
/// # 返回值
///
/// 已认证并刷新令牌的 [`LoginObj`]，`auth_type` 为 `LittleSkin` 或 `SelfLittleSkin`
pub async fn authenticate(
    client_token: String,
    user: String,
    password: String,
    server: Option<String>,
    gui: Option<Box<dyn GuiSelectHandel>>,
) -> CoreResult<LoginObj> {
    let mut auth_type = AuthType::LittleSkin;
    let server = match server {
        None => String::from(urls::LITTLE_SKIN_URL),
        Some(server) => {
            auth_type = AuthType::SelfLittleSkin;
            let mut server = server.clone();
            // 规范化服务器地址：移除常见的子路径
            if server.ends_with("/api/yggdrasil") {
                server = server.replace("/api/yggdrasil", "/");
            }
            if server.ends_with("/user") {
                server = server.replace("/user", "/");
            }
            if !server.ends_with('/') {
                server.push('/');
            }

            server
        }
    };

    let server1 = server.clone() + "api/yggdrasil";

    let obj = legacy::authenticate(&server1, client_token, user, password, true).await?;

    let mut auth = obj.auth;

    if let Some(list) = obj.logins {
        match gui {
            Some(gui) => {
                let auths: Vec<String> = list.iter().map(|x| x.user_name.clone()).collect();
                let index = gui.select_auth(auths);
                if let Some(item) = list.get(index as usize) {
                    auth.uuid = item.uuid.clone();
                    auth.user_name = item.user_name.clone();
                }
            }
            None => {
                let item = &list[0];
                auth.uuid = item.uuid.clone();
                auth.user_name = item.user_name.clone();
            }
        };
    }

    auth.auth_type = auth_type;
    if auth_type == AuthType::SelfLittleSkin {
        auth.text1 = Some(server.clone());
    }

    legacy::refresh(&server1, &mut auth, true).await?;
    Ok(auth)
}

impl LoginObj {
    /// 刷新 LittleSkin 皮肤站登录令牌
    ///
    /// - `cancel`: 取消令牌
    ///
    /// # 返回值
    ///
    /// 刷新成功返回 `Ok(())`，此时账户凭据已被更新；令牌失效返回 `ErrorType::AuthTokenTimeout`，被取消时返回取消错误
    pub async fn refresh_littleskin(&mut self, cancel: CancellationToken) -> CoreResult<()> {
        let mut server = if self.auth_type == AuthType::LittleSkin {
            String::from(urls::LITTLE_SKIN_URL)
        } else {
            self.text1
                .clone()
                .filter(|s| !s.is_empty())
                .ok_or(ErrorType::AuthServerNull)?
        };

        server.push_str("api/yggdrasil");

        if legacy::validate(&server, self).await? {
            if cancel.is_cancelled() {
                return Err(ErrorType::TaskCancel);
            }
            Ok(legacy::refresh(&server, self, false).await?)
        } else {
            Err(ErrorType::AuthTokenTimeout)
        }
    }

    /// 获取启动参数所需的 Yggdrasil 元数据
    ///
    /// # 返回值
    ///
    /// 认证服务器返回的元数据 JSON 文本
    pub async fn get_littleskin_key(&self) -> CoreResult<String> {
        let mut server = if self.auth_type == AuthType::LittleSkin {
            String::from(urls::LITTLE_SKIN_URL)
        } else {
            self.text1
                .clone()
                .filter(|s| !s.is_empty())
                .ok_or(ErrorType::AuthServerNull)?
        };

        server.push_str("api/yggdrasil");

        Ok(mml_net::get_login_client().get_text(&server).await?)
    }
}
