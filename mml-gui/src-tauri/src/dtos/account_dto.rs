//! 账户窗口 DTO：账户列表视图，IPC 返回，前端 wire 为 camelCase
//!
//! 账户在 Rust 侧以 `mml_auth::LoginObj` 存完整凭据并持久化到 `accounts.json`，
//! 通过 [`AccountStoreDto::from_login`] 转成前端消费的 DTO：
//! 登录信息 userName / authType / loginTime，加上前端显示用的 avatarColor / skin / tokenStatus / avatar。

use mml_auth::{AuthType, LoginObj};
use serde::{Deserialize, Serialize};

/// 账户列表视图，IPC 返回
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStoreViewDto {
    /// 全部账户
    pub accounts: Vec<AccountStoreDto>,
    /// 当前选中账户 UUID
    pub current_uuid: Option<String>,
}

/// 单个账户，前端 wire 为 camelCase，字段即 `mml-vue/src/lib/types.ts` 的 Account
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStoreDto {
    /// 玩家用户名
    pub user_name: String,
    /// 账户 UUID
    pub uuid: String,
    /// 账户类型：offline / microsoft / littleskin / authlib / nide8
    pub auth_type: String,
    /// 最后登录时间，仅用于展示
    pub login_time: String,
    /// 头像渐变起点色，CSS 颜色，无皮肤时前端回退占位图
    pub avatar_color: String,
    /// 皮肤主色，用于生成 SVG 占位图
    pub skin: String,
    /// Token 状态：valid / expired
    pub token_status: String,
    /// 皮肤头像，mml-skin-draw 渲染的 data URI PNG；无皮肤为 None
    pub avatar: Option<String>,
    /// 服务器地址，重登时预填：littleskin/authlib 为服务器 URL，nide8 为服务器 ID；
    /// offline 没有，微软的 text1 存的是 refresh_token，绝不能下发给前端
    pub server: Option<String>,
    /// 自定义字段 1，详情页展示；微软的 text1 是 refresh_token，仅本地显示
    pub ext1: Option<String>,
    /// 自定义字段 2，详情页展示
    pub ext2: Option<String>,
    /// 是否有可刷新的令牌，决定前端是否显示刷新按钮
    pub can_refresh: bool,
    /// 是否支持重新登录，决定前端是否显示重登按钮
    pub can_relogin: bool,
    /// 是否可编辑，决定前端是否显示编辑按钮
    pub can_edit: bool,
}

/// OAuth 登录开始
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountOAuthDto {
    /// 登录码
    pub code: String,
    /// 登录网址
    pub url: String,
}

/// OAuth 登录阶段进度，state 取值：waiting / xbox / xsts / token / profile / ok / fail
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountOAuthStateDto {
    /// 当前阶段标识，前端按 key 做 i18n
    pub state: String,
    /// 附加信息，fail 时为错误文案
    pub message: Option<String>,
}

impl AccountStoreDto {
    /// 从 LoginObj 构造前端 DTO，LoginObj 即账户完整凭据
    ///
    /// 有皮肤时 `avatar` 应为 `mml-skin-draw` 渲染的头像 data URI，渲染管线见
    /// `windows/account.rs`；当前 LoginObj 尚未携带皮肤贴图，故为 None，
    /// 前端回退到原来的占位图显示。
    ///
    /// - `login`: 账户完整凭据
    ///
    /// # 返回值
    ///
    /// 返回前端显示用 DTO
    pub fn from_login(login: &LoginObj) -> Self {
        let (c1, c2) = palette(&login.uuid);
        Self {
            user_name: login.user_name.clone(),
            uuid: login.uuid.clone(),
            auth_type: auth_type_str(&login.auth_type).to_string(),
            login_time: login.last_login.format("%Y-%m-%d %H:%M").to_string(),
            avatar_color: format!("linear-gradient(135deg, {c1}, {c2})"),
            skin: c1,
            // 本地判过期：仅 OAuth 记录了 expire_at，其余类型恒有效
            token_status: if login.is_expired() {
                "expired".to_string()
            } else {
                "valid".to_string()
            },
            avatar: None,
            server: match login.auth_type {
                AuthType::OAuth | AuthType::Offline => None,
                _ => login.text1.clone(),
            },
            ext1: login.text1.clone(),
            ext2: login.text2.clone(),
            can_refresh: login.auth_type.can_refresh(),
            can_relogin: login.auth_type.can_relogin(),
            can_edit: login.auth_type.can_edit(),
        }
    }
}

/// 前端账户类型字符串 → AuthType
///
/// - `s`: 前端账户类型字符串
///
/// # 返回值
///
/// 返回对应账户类型，无法识别按离线处理
pub fn auth_type_from_str(s: &str) -> AuthType {
    match s {
        "microsoft" => AuthType::OAuth,
        "littleskin" => AuthType::LittleSkin,
        "selflittleskin" => AuthType::SelfLittleSkin,
        "authlib" => AuthType::AuthlibInjector,
        "nide8" => AuthType::Nide8,
        _ => AuthType::Offline,
    }
}

/// AuthType → 前端账户类型字符串
///
/// - `t`: 账户类型
///
/// # 返回值
///
/// 返回前端账户类型字符串
fn auth_type_str(t: &AuthType) -> &'static str {
    match t {
        AuthType::Offline => "offline",
        AuthType::OAuth => "microsoft",
        AuthType::LittleSkin => "littleskin",
        AuthType::SelfLittleSkin => "selflittleskin",
        AuthType::AuthlibInjector => "authlib",
        AuthType::Nide8 => "nide8",
    }
}

/// 按 uuid 哈希取配色，用作无皮肤时的占位色
///
/// - `uuid`: 账户 UUID
///
/// # 返回值
///
/// 返回渐变起点色与终点色，均为 CSS 颜色
fn palette(uuid: &str) -> (String, String) {
    const P: [(&str, &str); 6] = [
        ("#3f8cff", "#5f6cff"),
        ("#34d399", "#22d3ee"),
        ("#a855f7", "#ec4899"),
        ("#f59e0b", "#ef4444"),
        ("#06b6d4", "#6366f1"),
        ("#f472b6", "#8b5cf6"),
    ];
    let h = uuid
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    let (c1, c2) = P[(h as usize) % P.len()];
    (c1.to_string(), c2.to_string())
}
