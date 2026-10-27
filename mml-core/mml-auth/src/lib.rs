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
/// 微软 Profile 返回的 UUID 不带连字符，登录与刷新必须落成同一种写法，
/// 否则同一账户会因存储键不同被存成两条记录。
///
/// - `uuid`: UUID 文本，带不带连字符均可
///
/// # 返回值
///
/// 合法 UUID 统一为带连字符的小写形式；无法解析时返回去掉首尾空白的原文本
pub fn normalize_uuid(uuid: &str) -> String {
    let uuid = uuid.trim();
    Uuid::parse_str(uuid)
        .map(|u| u.to_string())
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
    /// 账户 UUID，Minecraft 格式，含连字符
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
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// 测试用的假凭据，与真实账户无关
    const FAKE_UUID: &str = "00000000-0000-0000-0000-00000000aaaa";
    const FAKE_TOKEN: &str = "fake-access-token";
    const FAKE_CLIENT: &str = "fake-client-token";

    /// AuthType 按 `#[repr(u8)]` 顺位序列化为 0..=5
    #[test]
    fn test_auth_type_repr_values() {
        let cases = [
            (AuthType::Offline, 0u8),
            (AuthType::OAuth, 1),
            (AuthType::Nide8, 2),
            (AuthType::AuthlibInjector, 3),
            (AuthType::LittleSkin, 4),
            (AuthType::SelfLittleSkin, 5),
        ];
        for (ty, num) in cases {
            let json = serde_json::to_string(&ty).unwrap();
            assert_eq!(json, num.to_string(), "{:?} 应序列化为 {}", ty, num);
            let back: AuthType = serde_json::from_str(&json).unwrap();
            assert_eq!(back, ty);
        }
    }

    /// AuthType 反序列化非法数字应失败，没有 `#[serde(other)]` 兜底
    #[test]
    fn test_auth_type_deserialize_invalid() {
        assert!(serde_json::from_str::<AuthType>("6").is_err());
        assert!(serde_json::from_str::<AuthType>("-1").is_err());
        assert!(serde_json::from_str::<AuthType>("\"Offline\"").is_err());
    }

    /// from_str 应识别全部认证类型名称
    #[test]
    fn test_auth_type_from_str() {
        assert_eq!(AuthType::from_str("Offline"), AuthType::Offline);
        assert_eq!(AuthType::from_str("OAuth"), AuthType::OAuth);
        assert_eq!(AuthType::from_str("Nide8"), AuthType::Nide8);
        assert_eq!(
            AuthType::from_str("AuthlibInjector"),
            AuthType::AuthlibInjector
        );
        assert_eq!(AuthType::from_str("LittleSkin"), AuthType::LittleSkin);
        assert_eq!(
            AuthType::from_str("SelfLittleSkin"),
            AuthType::SelfLittleSkin
        );
    }

    /// from_str 对未知名称回退为 Offline，固化现有行为
    #[test]
    fn test_auth_type_from_str_fallback() {
        assert_eq!(AuthType::from_str("unknown"), AuthType::Offline);
        assert_eq!(AuthType::from_str("offline"), AuthType::Offline);
        assert_eq!(AuthType::from_str(""), AuthType::Offline);
    }

    /// LoginObj 序列化应使用 PascalCase 字段名，且可完整往返
    #[test]
    fn test_login_obj_json_round_trip() {
        let mut obj = LoginObj::new(
            "Steve".to_string(),
            FAKE_UUID.to_string(),
            FAKE_TOKEN.to_string(),
            FAKE_CLIENT.to_string(),
        );
        obj.auth_type = AuthType::OAuth;
        obj.text1 = Some("fake-refresh-token".to_string());
        obj.text2 = Some("extra".to_string());

        let json = serde_json::to_value(&obj).unwrap();
        let map = json.as_object().unwrap();
        // 字段名与旧版持久化格式保持兼容
        for key in [
            "UserName",
            "UUID",
            "AccessToken",
            "ClientToken",
            "AuthType",
            "Text1",
            "Text2",
            "LastLogin",
        ] {
            assert!(map.contains_key(key), "缺少字段 {}", key);
        }
        assert_eq!(map["UserName"], "Steve");
        assert_eq!(map["AuthType"], 1);

        let back: LoginObj = serde_json::from_value(json).unwrap();
        assert_eq!(back.user_name, obj.user_name);
        assert_eq!(back.uuid, obj.uuid);
        assert_eq!(back.access_token, obj.access_token);
        assert_eq!(back.client_token, obj.client_token);
        assert_eq!(back.auth_type, obj.auth_type);
        assert_eq!(back.text1, obj.text1);
        assert_eq!(back.text2, obj.text2);
        assert_eq!(back.last_login, obj.last_login);
    }

    /// 缺失字段的旧 JSON 应通过 `#[serde(default)]` 填充默认值
    #[test]
    fn test_login_obj_partial_json_defaults() {
        let json = r#"{"UserName":"Alex"}"#;
        let obj: LoginObj = serde_json::from_str(json).unwrap();
        assert_eq!(obj.user_name, "Alex");
        assert_eq!(obj.uuid, "");
        assert_eq!(obj.access_token, "");
        assert_eq!(obj.client_token, "");
        assert_eq!(obj.auth_type, AuthType::Offline);
        assert_eq!(obj.text1, None);
        assert_eq!(obj.text2, None);
    }

    /// new() 应使用当前时间，new_empty()/new_token() 应只填指定字段
    #[test]
    fn test_login_obj_constructors() {
        let full = LoginObj::new(
            "Steve".to_string(),
            FAKE_UUID.to_string(),
            FAKE_TOKEN.to_string(),
            FAKE_CLIENT.to_string(),
        );
        assert_eq!(full.user_name, "Steve");
        assert_eq!(full.uuid, FAKE_UUID);
        assert_eq!(full.auth_type, AuthType::Offline);
        assert_eq!(full.text1, None);
        // 最后登录时间应为当前时间，与默认值 0 明显不同
        assert!(full.last_login.timestamp() > 0);

        let empty = LoginObj::new_empty("Alex".to_string(), FAKE_UUID.to_string());
        assert_eq!(empty.user_name, "Alex");
        assert_eq!(empty.access_token, "");
        assert_eq!(empty.client_token, "");
        // 空账户的最后登录时间为默认值，即 Unix 纪元
        assert_eq!(empty.last_login.timestamp(), 0);

        let token = LoginObj::new_token(FAKE_TOKEN.to_string(), FAKE_CLIENT.to_string());
        assert_eq!(token.access_token, FAKE_TOKEN);
        assert_eq!(token.user_name, "");
        assert_eq!(token.uuid, "");
    }

    /// get_key 应由 UUID + 认证类型组成；HashMap 中同 UUID 不同类型视为不同账户
    #[test]
    fn test_user_key_obj_distinct() {
        let mut a = LoginObj::new(
            "Steve".to_string(),
            FAKE_UUID.to_string(),
            FAKE_TOKEN.to_string(),
            FAKE_CLIENT.to_string(),
        );
        let key = a.get_key();
        assert_eq!(key.uuid, FAKE_UUID);
        assert_eq!(key.auth_type, AuthType::Offline);

        let mut map: HashMap<UserKeyObj, LoginObj> = HashMap::new();
        map.insert(a.get_key(), a.clone());

        // 同 UUID 不同认证类型应视为不同账户
        let mut b = a.clone();
        b.auth_type = AuthType::OAuth;
        map.insert(b.get_key(), b);
        assert_eq!(map.len(), 2);

        // 相同键应覆盖
        a.access_token = "new-token".to_string();
        map.insert(a.get_key(), a.clone());
        assert_eq!(map.len(), 2);
        assert_eq!(map[&a.get_key()].access_token, "new-token");
    }

    /// 微软 Profile 的无连字符 UUID 与登录时的带连字符 UUID 应落到同一个账户键
    #[test]
    fn test_uuid_format_shares_key() {
        let mut hyphenated = LoginObj::new(
            "Steve".to_string(),
            "00000000-0000-0000-0000-00000000aaaa".to_string(),
            FAKE_TOKEN.to_string(),
            FAKE_CLIENT.to_string(),
        );
        hyphenated.auth_type = AuthType::OAuth;
        let mut plain = hyphenated.clone();
        plain.uuid = "0000000000000000000000000000aaaa".to_string();

        assert_eq!(normalize_uuid(&plain.uuid), hyphenated.uuid);
        assert_eq!(plain.get_key(), hyphenated.get_key());
    }

    /// 离线账户 refresh 无需网络，应直接返回成功
    #[tokio::test]
    async fn test_refresh_offline_noop() {
        let mut obj = LoginObj::new(
            "Steve".to_string(),
            FAKE_UUID.to_string(),
            FAKE_TOKEN.to_string(),
            FAKE_CLIENT.to_string(),
        );
        obj.refresh(CancellationToken::new()).await.unwrap();
    }

    /// 账户占用锁按实例登记：别的实例还在用就算占用，自己退出只摘自己
    #[test]
    fn test_user_lock_tracks_instances() {
        let key = UserKeyObj {
            uuid: Uuid::new_v4().to_string(),
            auth_type: AuthType::Offline,
        };
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();

        assert!(!have_lock(&key));
        lock_user(&key, first);
        assert!(have_lock(&key));
        assert!(!locked_by_other(&key, &first), "自己占用不算别人");
        assert!(locked_by_other(&key, &second));

        // 两个实例同时用同一账户：退掉一个仍然是占用
        lock_user(&key, second);
        unlock_user(&first);
        assert!(have_lock(&key));
        assert!(
            locked_by_other(&key, &first),
            "第二个实例还占着，对已退出的实例仍算别人"
        );

        unlock_user(&second);
        assert!(!have_lock(&key), "最后一个实例退出后解锁");
    }
}
