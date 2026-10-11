//! Authlib-Injector 外置登录模块
//!
//! Authlib-Injector 是一种允许使用自定义认证服务器的 Minecraft 登录方式。
//! 启动器通过向指定的 Yggdrasil 兼容 API 服务器进行认证，
//! 配合游戏端的 authlib-injector 模组实现外置登录。
//!
//! # 认证流程
//!
//! 1. 向用户指定的服务器地址发起 Yggdrasil 认证
//! 2. 处理多角色选择，通过 GUI 回调或自动选择第一个
//! 3. 刷新令牌并保存账户

use mml_names::i18_items::error_type::{CoreResult, ErrorType};
use tokio_util::sync::CancellationToken;

use crate::{
    AuthType, LoginObj,
    legacy::{self, GuiSelectHandel},
};

/// 外置登录认证
///
/// `gui` 为 `None` 时自动选择第一个角色
///
/// - `client_token`: 客户端标识令牌
/// - `user`: 用户名
/// - `password`: 密码
/// - `server`: 认证服务器地址，完整 URL
/// - `gui`: 可选的角色选择回调
///
/// # 返回值
///
/// 已认证并刷新令牌的 [`LoginObj`]，服务器地址存入 `text1`
pub async fn authenticate(
    client_token: String,
    user: String,
    password: String,
    server: String,
    gui: Option<Box<dyn GuiSelectHandel>>,
) -> CoreResult<LoginObj> {
    let obj = legacy::authenticate(&server, client_token, user, password, true).await?;

    let mut auth = obj.auth;
    auth.auth_type = AuthType::AuthlibInjector;
    auth.text1 = Some(server.clone());

    let need_select = false;

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

    legacy::refresh(&server, &mut auth, need_select).await?;
    Ok(auth)
}

impl LoginObj {
    /// 刷新 Authlib-Injector 登录令牌：先验证有效性，无效则返回超时错误
    ///
    /// - `cancel`: 取消令牌
    ///
    /// # 返回值
    ///
    /// 刷新成功返回 `Ok(())`，此时账户凭据已被更新；令牌失效返回 `ErrorType::AuthTokenTimeout`，被取消时返回取消错误
    pub async fn refresh_authlib(&mut self, cancel: CancellationToken) -> CoreResult<()> {
        let server = self
            .text1
            .clone()
            .filter(|s| !s.is_empty())
            .ok_or(ErrorType::AuthServerNull)?;
        if legacy::validate(&server, self).await? {
            if cancel.is_cancelled() {
                return Err(ErrorType::TaskCancel);
            }

            Ok(legacy::refresh(&server, self, false).await?)
        } else {
            Err(ErrorType::AuthTokenTimeout)
        }
    }

    /// 获取启动参数所需的 Yggdrasil 服务器元数据
    ///
    /// 访问认证服务器根路径获取 JSON 元信息，用于设置游戏启动参数。
    ///
    /// # 返回值
    ///
    /// 认证服务器返回的元数据 JSON 文本
    pub async fn get_authlib_key(&self) -> CoreResult<String> {
        let server = self
            .text1
            .clone()
            .filter(|s| !s.is_empty())
            .ok_or(ErrorType::AuthServerNull)?;

        Ok(mml_net::get_login_client().get_text(&server).await?)
    }
}
