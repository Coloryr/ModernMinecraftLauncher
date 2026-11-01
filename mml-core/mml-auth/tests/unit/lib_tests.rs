use super::*;
use std::collections::HashMap;

/// 测试用的假凭据，与真实账户无关
const FAKE_UUID: &str = "0000000000000000000000000000aaaa";
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

/// 带连字符与不带连字符的 UUID 都应统一成不带连字符的账户键
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

    assert_eq!(normalize_uuid(&hyphenated.uuid), plain.uuid);
    assert_eq!(plain.get_key(), hyphenated.get_key());
    assert_eq!(hyphenated.get_key().uuid, plain.uuid);
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

