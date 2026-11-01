//! Minecraft 启动器账户认证模块
//!
//! 本模块负责管理 Minecraft 启动器中的用户账户体系，支持多种认证方式：
//!
//! # 支持的认证类型
//!
//! | 认证类型 | 枚举变体 | 说明 |
//! |---------|---------|------|
//! | 离线账户 | `Offline` | 无需联网验证的离线模式 |
//! | 微软正版 | `OAuth` | 通过 Microsoft OAuth 2.0 流程认证 |
//! | 统一通行证 | `Nide8` | 第三方统一通行证认证 |
//! | 外置登录 | `AuthlibInjector` | Authlib-Injector 外置认证 |
//! | LittleSkin | `LittleSkin` | LittleSkin 皮肤站认证 |
//! | 自建皮肤站 | `SelfLittleSkin` | 自建 LittleSkin 皮肤站认证 |
//!
//! # 模块结构
//!
//! - [`auths`] — 账户持久化存储管理
//! - [`legacy`] — 旧版 Yggdrasil 认证协议：外置登录、皮肤站、统一通行证
//! - [`oauth`] — Microsoft OAuth 2.0 现代认证协议：Xbox Live → XSTS → Minecraft
//!
//! # 账户数据结构
//!
//! 核心类型 [`LoginObj`] 存储一个账户的完整凭据信息，
//! 包括用户名、UUID、access token、client token、认证类型等。
//! 账户通过 [`UserKeyObj`] 作为唯一键进行索引，键由 UUID 与认证类型组成。

use std::{
    collections::{HashMap, HashSet},
    sync::{LazyLock, RwLock},
};

use chrono::{DateTime, Duration, FixedOffset, Local};
use mml_names::i18_items::error_type::CoreResult;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub mod auths;
pub mod legacy;
pub mod oauth;

/// 统一 UUID 文本格式
///
/// 账户存储与 ColorMC 保持一致，用**不带连字符**的 32 位小写十六进制；
/// 微软 Profile、ColorMC 的 `auth.json` 都是这个写法，登录与刷新也落成同一种，
/// 否则同一账户会因存储键不同被存成两条记录。
///
/// - `uuid`: UUID 文本，带不带连字符均可
///
/// # 返回值
///
/// 合法 UUID 统一为不带连字符的小写形式；无法解析时返回去掉首尾空白的原文本
pub fn normalize_uuid(uuid: &str) -> String {
    let uuid = uuid.trim();
    Uuid::parse_str(uuid)
        .map(|u| u.simple().to_string())
        .unwrap_or_else(|_| uuid.to_string())
}

/// 账户认证类型
///
/// 定义了启动器支持的六种账户认证方式。
/// 使用 `#[repr(u8)]` 标记，可高效序列化为单字节存储。
#[derive(Serialize_repr, Deserialize_repr, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AuthType {
    /// 离线账户，无需联网验证，用户名可自定义
    Offline,
    /// 微软正版登录，走 Microsoft OAuth 2.0 + Xbox Live 认证链
    OAuth,
    /// 统一通行证 Nide8 第三方认证
    Nide8,
    /// Authlib-Injector 外置登录认证
    AuthlibInjector,
    /// LittleSkin 官方皮肤站认证
    LittleSkin,
    /// 自建 LittleSkin 皮肤站认证
    SelfLittleSkin,
}

/// 默认认证类型为离线账户
impl Default for AuthType {
    fn default() -> Self {
        AuthType::Offline
    }
}

impl AuthType {
    /// 是否有可刷新的令牌；离线账户没有凭据，无令牌可刷新
    ///
    /// # 返回值
    ///
    /// 不是离线账户时返回 `true`
    pub fn can_refresh(&self) -> bool {
        !matches!(self, AuthType::Offline)
    }

    /// 是否支持重新登录，即重输密码重新认证；离线无令牌，OAuth 走静默刷新没有这一步
    ///
    /// # 返回值
    ///
    /// 不是离线、也不是 OAuth 账户时返回 `true`
    pub fn can_relogin(&self) -> bool {
        !matches!(self, AuthType::Offline | AuthType::OAuth)
    }

    /// 是否可编辑；离线账户没有服务器凭据，改名 / 改 UUID 即改账户本身
    ///
    /// # 返回值
    ///
    /// 离线账户返回 `true`
    pub fn can_edit(&self) -> bool {
        matches!(self, AuthType::Offline)
    }

    /// 是否微软正版账户
    ///
    /// # 返回值
    ///
    /// 是微软正版账户时返回 `true`
    pub fn is_oauth(&self) -> bool {
        matches!(self, AuthType::OAuth)
    }

    /// 从字符串解析认证类型
    ///
    /// - `str`: 认证类型名称，如 `Offline`、`OAuth`、`Nide8`
    ///
    /// # 返回值
    ///
    /// 返回对应的认证类型，无法识别时回退为 [`AuthType::Offline`]
    pub fn from_str(str: &str) -> AuthType {
        if str == "Offline" {
            AuthType::Offline
        } else if str == "OAuth" {
            AuthType::OAuth
        } else if str == "Nide8" {
            AuthType::Nide8
        } else if str == "AuthlibInjector" {
            AuthType::AuthlibInjector
        } else if str == "LittleSkin" {
            AuthType::LittleSkin
        } else if str == "SelfLittleSkin" {
            AuthType::SelfLittleSkin
        } else {
            AuthType::Offline
        }
    }
}

/// 保存的账户信息
///
/// 存储一个 Minecraft 账户的完整凭据，用于序列化持久化和登录验证。
/// 字段使用 PascalCase 命名以兼容 JSON 序列化格式。
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct LoginObj {
    /// 用户名
    #[serde(rename = "UserName")]
    pub user_name: String,
    /// 账户 UUID，32 位小写十六进制，不带连字符
    #[serde(rename = "UUID")]
    pub uuid: String,
    /// 登录访问令牌，即 access token，用于验证身份
    #[serde(rename = "AccessToken")]
    pub access_token: String,
    /// 客户端标识令牌，即 client token，用于标识启动器实例
    #[serde(rename = "ClientToken")]
    pub client_token: String,
    /// 账户认证类型
    #[serde(rename = "AuthType")]
    pub auth_type: AuthType,
    /// 扩展字段 1，含义随认证类型而定
    ///
    /// - `OAuth`: refresh_token
    /// - `Nide8`: 服务器 UUID
    /// - `AuthlibInjector` / `LittleSkin`: 服务器地址
    #[serde(rename = "Text1")]
    pub text1: Option<String>,
    /// 扩展字段 2，预留，当前未使用
    #[serde(rename = "Text2")]
    pub text2: Option<String>,
    /// 最后登录时间，带时区
    #[serde(rename = "LastLogin")]
    pub last_login: DateTime<FixedOffset>,
    /// 访问令牌绝对过期时间，带时区
    ///
    /// 仅 OAuth 账户记录，登录 / 刷新时由响应的 expires_in 换算；
    /// `None` 表示无过期信息，如离线 / 皮肤站令牌或旧版数据，视为不过期
    #[serde(rename = "ExpireAt")]
    pub expire_at: Option<DateTime<FixedOffset>>,
}

impl LoginObj {
    /// 创建完整的账户信息
    ///
    /// 使用当前时间作为最后登录时间，其他扩展字段初始化为空。
    ///
    /// - `user_name`: 玩家用户名
    /// - `uuid`: 账户 UUID
    /// - `access_token`: 登录访问令牌
    /// - `client_token`: 客户端标识令牌
    ///
    /// # 返回值
    ///
    /// 新建的账户，最后登录时间为当前时间，扩展字段为空
    pub fn new(
        user_name: String,
        uuid: String,
        access_token: String,
        client_token: String,
    ) -> Self {
        let dt = Local::now();
        let dt_new: DateTime<FixedOffset> = dt.fixed_offset();

        Self {
            user_name,
            uuid,
            access_token,
            client_token,
            auth_type: Default::default(),
            text1: Default::default(),
            text2: Default::default(),
            last_login: dt_new,
            expire_at: None,
        }
    }

    /// 创建空白账户，仅有用户名和 UUID，无令牌
    ///
    /// 用于存储从认证服务器返回的可选角色列表中尚未选中的账户。
    ///
    /// - `user_name`: 玩家用户名
    /// - `uuid`: 账户 UUID
    ///
    /// # 返回值
    ///
    /// 新建的空白账户，只有用户名与 UUID，令牌与登录信息都为空
    pub fn new_empty(user_name: String, uuid: String) -> Self {
        Self {
            user_name,
            uuid,
            ..Default::default()
        }
    }

    /// 创建仅有令牌的账户，无用户名和 UUID
    ///
    /// 用于多角色选择场景：已获取令牌但尚未确定具体角色。
    ///
    /// - `access_token`: 登录访问令牌
    /// - `client_token`: 客户端标识令牌
    ///
    /// # 返回值
    ///
    /// 新建的仅令牌账户，用户名与 UUID 为空
    pub fn new_token(access_token: String, client_token: String) -> Self {
        Self {
            access_token,
            client_token,
            ..Default::default()
        }
    }

    /// 记录访问令牌的有效期，单位秒，按当前时间换算为绝对过期时间
    ///
    /// 登录 / 刷新成功后调用；`expires_in <= 0` 时忽略，不记录过期时间
    ///
    /// - `expires_in`: 令牌剩余有效期，单位秒
    pub fn set_expire_in(&mut self, expires_in: i64) {
        if expires_in > 0 {
            self.expire_at = Some((Local::now() + Duration::seconds(expires_in)).fixed_offset());
        }
    }

    /// 访问令牌是否已过本地记录的过期时间
    ///
    /// 未记录过期时间的账户，如离线 / 皮肤站令牌、旧版数据，视为未过期。
    /// 仅代表本地时钟判断，不代表服务端已吊销。
    ///
    /// # 返回值
    ///
    /// 已过期返回 `true`
    pub fn is_expired(&self) -> bool {
        self.expire_at
            .is_some_and(|t| Local::now().fixed_offset() > t)
    }

    /// 获取账户的唯一键，由 UUID 与认证类型组合而成
    ///
    /// 用于在账户存储中索引和去重。
    ///
    /// # 返回值
    ///
    /// 见 [`UserKeyObj`]
    pub fn get_key(&self) -> UserKeyObj {
        UserKeyObj {
            uuid: normalize_uuid(&self.uuid),
            auth_type: self.auth_type.clone(),
        }
    }

    /// 根据认证类型刷新登录凭据
    ///
    /// 此方法会根据 `auth_type` 字段分派到对应的刷新逻辑：
    /// - `OAuth` → 微软 OAuth 刷新链
    /// - `Nide8` → 统一通行证刷新
    /// - `AuthlibInjector` → 外置登录刷新
    /// - `LittleSkin` / `SelfLittleSkin` → 皮肤站刷新
    /// - 离线账户 → 直接返回成功，无需刷新
    ///
    /// - `cancel`: 取消令牌，用于中断异步操作
    ///
    /// # 返回值
    ///
    /// 刷新成功返回 `Ok(())`，认证失败或被取消时返回相应错误
    pub async fn refresh(&mut self, cancel: CancellationToken) -> CoreResult<()> {
        match &self.auth_type {
            AuthType::OAuth => self.refresh_oauth(cancel).await,
            AuthType::Nide8 => self.refresh_nide8(cancel).await,
            AuthType::AuthlibInjector => self.refresh_authlib(cancel).await,
            AuthType::LittleSkin | AuthType::SelfLittleSkin => {
                self.refresh_littleskin(cancel).await
            }
            _ => Ok(()),
        }
    }
}

/// 账户唯一键
///
/// 由 UUID 和认证类型组成，用于在账户存储中唯一标识一个账户。
/// 同一 UUID 的不同认证类型视为不同账户。
#[derive(Eq, Hash, PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct UserKeyObj {
    /// 账户标识，Minecraft UUID
    pub uuid: String,
    /// 账户认证类型
    pub auth_type: AuthType,
}

/// 正在被游戏实例占用的账户：账户键 → 占用它的实例 uuid 集合
///
/// 同一个账户允许被多个实例同时使用（用户选择继续启动、或关掉了占用检查），
/// 所以值用集合；实例退出时只摘掉自己，集合空了才删除账户键。
static USER_LOCK: LazyLock<RwLock<HashMap<UserKeyObj, HashSet<Uuid>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 该账户是否正被某个游戏实例占用
///
/// - `user`: 账户唯一键
///
/// # 返回值
///
/// 已被占用返回 `true`
pub fn have_lock(user: &UserKeyObj) -> bool {
    USER_LOCK.read().unwrap().contains_key(user)
}

/// 该账户是否被 `instance` 之外的实例占用
///
/// 启动检查用：本次要启动的实例还没登记，把"自己"排除掉，
/// 免得重入时把本实例的占用当成别的实例。
///
/// - `user`: 账户唯一键
/// - `instance`: 本次要启动的实例
///
/// # 返回值
///
/// 有别的实例正在使用该账户时返回 `true`
pub fn locked_by_other(user: &UserKeyObj, instance: &Uuid) -> bool {
    if !have_lock(user) {
        return false;
    }
    USER_LOCK
        .read()
        .unwrap()
        .get(user)
        .is_some_and(|set| set.iter().any(|id| id != instance))
}

/// 登记某个实例对账户的占用
///
/// - `user`: 账户唯一键
/// - `instance`: 占用它的实例
pub fn lock_user(user: &UserKeyObj, instance: Uuid) {
    USER_LOCK
        .write()
        .unwrap()
        .entry(user.clone())
        .or_default()
        .insert(instance);
}

/// 释放某个实例对账户的占用；实例退出或启动失败时调用
///
/// - `instance`: 实例 uuid
pub fn unlock_user(instance: &Uuid) {
    let mut map = USER_LOCK.write().unwrap();
    map.retain(|_, set| {
        set.remove(instance);
        !set.is_empty()
    });
}

#[cfg(test)]
#[path = "../tests/unit/lib_tests.rs"]
mod tests;
