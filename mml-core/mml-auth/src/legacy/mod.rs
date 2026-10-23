//! 旧版 Yggdrasil 认证协议模块
//!
//! 本模块实现了 Minecraft 旧版认证协议 Yggdrasil API，
//! 这是 Mojang 在迁移到 Microsoft OAuth 之前使用的认证系统。
//! 目前该协议仍被以下第三方认证服务广泛使用：
//!
//! - **外置登录 Authlib-Injector** — 自定义认证服务器
//! - **统一通行证 Nide8** — 国内流行的第三方认证
//! - **LittleSkin** — 皮肤站认证服务
//!
//! # API 端点
//!
//! 所有认证服务器均遵循统一的 Yggdrasil API 规范：
//! - `POST /authserver/authenticate` — 登录认证
//! - `POST /authserver/refresh` — 刷新令牌
//! - `POST /authserver/validate` — 验证令牌有效性
//!
//! # 子模块
//!
//! - [`authenticate_obj`] — 认证请求/响应的数据结构
//! - [`authlib_injector`] — Authlib-Injector 外置登录实现
//! - [`nide8`] — 统一通行证登录实现
//! - [`little_skin`] — LittleSkin 皮肤站登录实现

use chrono::Local;
use mml_names::i18_items::error_type::{CoreResult, ErrorType};
use reqwest::StatusCode;

use crate::{
    LoginObj,
    legacy::authenticate_obj::{
        AgentObj, AuthenticateObj, AuthenticateResObj, RefreshObj, SelectedProfileObj,
    },
};

pub mod authenticate_obj;
pub mod authlib_injector;
pub mod little_skin;
pub mod nide8;

/// GUI 账户选择回调接口
///
/// 当认证服务器返回多个可选角色时，通过此 trait 弹窗让用户选择
/// 要登录的账户。
/// `Send` 约束：登录在异步命令里执行，Tauri 要求 future 为 Send。
pub trait GuiSelectHandel: Send {
    /// 让用户从多个账户中选择一个
    ///
    /// - `auths`: 可选账户的用户名列表
    ///
    /// # 返回值
    ///
    /// 返回被选中账户在列表中的索引，从 0 开始
    fn select_auth(&self, auths: Vec<String>) -> i32;
}

/// 旧版认证方式的登录结果
///
/// 包含已认证的账户信息，以及可能存在的多角色选择列表。
pub struct LegacyLoginRes {
    /// 选中的账户；仅有令牌的临时账户表示还在等用户选角色
    pub auth: LoginObj,
    /// 可选的账户列表，服务器返回多个角色时为 `Some`
    pub logins: Option<Vec<LoginObj>>,
    /// 服务器尚未绑定角色，刷新时需提交本地选中的角色。
    pub needs_selection: bool,
}

/// 把 Yggdrasil 服务器的 HTTP 错误响应转成可读的登录失败原因
///
/// 服务器拒绝时返回 403/400 + JSON body，含一般已本地化的 errorMessage，
/// 直接透原文对用户没用；body 不是合法 Yggdrasil 错误结构时保持原错误
fn map_auth_error(err: ErrorType) -> ErrorType {
    let ErrorType::HttpError(data) = &err else {
        return err;
    };
    if let Ok(res) = serde_json::from_str::<AuthenticateResObj>(&data.error)
        && let Some(msg) = res.error_message
    {
        return ErrorType::AuthFail(msg);
    }
    err
}

/// 向 Yggdrasil 认证服务器发起登录请求
///
/// 旧版认证协议的通用登录实现，被外置登录、皮肤站和统一通行证共用。
///
/// - `server`: 认证服务器地址，完整 URL
/// - `client_token`: 客户端标识令牌
/// - `user`: 用户名，通常是邮箱
/// - `password`: 密码
/// - `use_minecraft`: 是否使用 Minecraft 官方启动器的 Agent 标识
///
/// # 返回值
///
/// 成功时返回 [`LegacyLoginRes`]，多角色时带可选列表；服务器未给出任何角色时返回错误
pub async fn authenticate(
    server: &String,
    client_token: String,
    user: String,
    password: String,
    use_minecraft: bool,
) -> CoreResult<LegacyLoginRes> {
    let obj = AuthenticateObj {
        agent: AgentObj::new(use_minecraft),
        username: user.clone(),
        password,
        client_token,
    };

    let mut server = server.clone();

    if !server.ends_with('/') {
        server.push('/');
    }

    server.push_str("authserver/authenticate");

    let obj = mml_net::get_login_client()
        .post_json_get_json::<_, AuthenticateResObj>(&server, &obj)
        .await
        .map_err(map_auth_error)?;

    if let Some(data) = obj.error_message {
        Err(ErrorType::AuthFail(data))
    } else if obj.selected_profile.is_none() && obj.available_profiles.is_none() {
        Err(ErrorType::AuthNoProfile)
    } else if let Some(data) = obj.selected_profile {
        // 服务器明确选中了某个角色
        Ok(LegacyLoginRes {
            auth: LoginObj::new(data.name, data.id, obj.access_token, obj.client_token),
            logins: None,
            needs_selection: false,
        })
    } else if let Some(list) = obj.available_profiles {
        if list.len() == 0 {
            Err(ErrorType::AuthNoProfile)
        } else if list.len() == 1 {
            // 仅有一个角色，直接选中
            let temp = &list[0];

            Ok(LegacyLoginRes {
                auth: LoginObj::new(
                    temp.name.clone(),
                    temp.id.clone(),
                    obj.access_token,
                    obj.client_token,
                ),
                logins: None,
                needs_selection: true,
            })
        } else {
            // 多个角色，优先按用户名匹配
            if let Some(item) = list
                .iter()
                .find(|item| item.name.eq_ignore_ascii_case(&user))
            {
                Ok(LegacyLoginRes {
                    auth: LoginObj::new(
                        item.name.clone(),
                        item.id.clone(),
                        obj.access_token,
                        obj.client_token,
                    ),
                    logins: None,
                    needs_selection: true,
                })
            } else {
                // 无一匹配，返回可选列表让用户自行选择
                let mut logins: Vec<LoginObj> = Vec::new();
                for item in list.iter() {
                    logins.push(LoginObj::new_empty(item.name.clone(), item.id.clone()));
                }

                Ok(LegacyLoginRes {
                    auth: LoginObj::new_token(obj.access_token, obj.client_token),
                    logins: Some(logins),
                    needs_selection: true,
                })
            }
        }
    } else {
        Err(ErrorType::AuthNoProfile)
    }
}

/// 刷新 Yggdrasil 认证令牌，提交现有 access token 换取新令牌。
/// `select` 为 `true` 时附带选定角色；成功后更新 `login` 的账户字段
///
/// - `server`: 认证服务器地址
/// - `login`: 待刷新的账户，可变引用，成功后字段会被更新
/// - `select`: 是否需要在刷新时选定角色，`true` 时附带 `selected_profile`
///
/// # 返回值
///
/// 刷新成功返回 `Ok(())`，此时 `login` 的字段已被更新；服务器返回错误或无角色时返回相应错误
pub async fn refresh(server: &String, login: &mut LoginObj, select: bool) -> CoreResult<()> {
    let obj = if select {
        RefreshObj {
            access_token: login.access_token.clone(),
            client_token: login.client_token.clone(),
            selected_profile: Some(SelectedProfileObj {
                name: login.user_name.clone(),
                id: login.uuid.clone(),
            }),
        }
    } else {
        RefreshObj {
            access_token: login.access_token.clone(),
            client_token: login.client_token.clone(),
            selected_profile: None,
        }
    };

    let mut server = server.clone();

    if !server.ends_with('/') {
        server.push('/');
    }

    server.push_str("authserver/refresh");

    let obj = mml_net::get_login_client()
        .post_json_get_json::<_, AuthenticateResObj>(&server, &obj)
        .await
        .map_err(map_auth_error)?;

    if let Some(data) = obj.error_message {
        Err(ErrorType::AuthFail(data))
    } else if obj.selected_profile.is_none() && !select {
        Err(ErrorType::AuthNoProfile)
    } else if obj.selected_profile.is_some() {
        // 服务器返回了新的角色信息，更新本地账户
        let select = obj.selected_profile.unwrap();
        login.user_name = select.name;
        login.uuid = select.id;
        login.access_token = obj.access_token;
        login.client_token = obj.client_token;
        login.last_login = Local::now().fixed_offset();

        Ok(())
    } else {
        // 仅刷新令牌，不改变角色信息
        login.access_token = obj.access_token;
        login.client_token = obj.client_token;
        login.last_login = Local::now().fixed_offset();

        Ok(())
    }
}

/// 验证 Yggdrasil 令牌是否仍然有效，仅检查，不刷新
///
/// - `server`: 认证服务器地址
/// - `login`: 待验证的账户
///
/// # 返回值
///
/// 令牌有效返回 `Ok(true)`，无效返回 `Ok(false)`，网络错误返回 `Err`
pub async fn validate(server: &String, login: &LoginObj) -> CoreResult<bool> {
    let obj = RefreshObj {
        access_token: login.access_token.clone(),
        client_token: login.client_token.clone(),
        selected_profile: None,
    };

    let mut server = server.clone();

    if !server.ends_with('/') {
        server.push('/');
    }

    server.push_str("authserver/validate");

    let obj = mml_net::get_login_client()
        .post_json_get_req(&server, &obj)
        .await?;

    // HTTP 204 No Content 表示令牌有效
    Ok(obj.status() == StatusCode::NO_CONTENT)
}
