use super::*;
use mml_names::{VERSION_NUM, names};

/// 伪装 Minecraft 启动器时应使用官方名称和协议版本 1
#[test]
fn test_agent_obj_minecraft() {
    let agent = AgentObj::new(true);
    assert_eq!(agent.name, names::MINECRAFT);
    assert_eq!(agent.version, 1);
}

/// 不伪装时应使用本启动器名称和当前版本号
#[test]
fn test_agent_obj_mml() {
    let agent = AgentObj::new(false);
    assert_eq!(agent.name, names::MML);
    assert_eq!(agent.version, VERSION_NUM);
}

/// 认证请求应使用 Yggdrasil 规范的 camelCase 字段名
#[test]
fn test_authenticate_obj_serialize() {
    let obj = AuthenticateObj {
        agent: AgentObj::new(true),
        username: "user@example.com".to_string(),
        password: "fake-password".to_string(),
        client_token: "fake-client-token".to_string(),
    };

    let json: serde_json::Value = serde_json::to_value(&obj).unwrap();
    assert_eq!(json["username"], "user@example.com");
    assert_eq!(json["password"], "fake-password");
    assert_eq!(json["clientToken"], "fake-client-token");
    assert_eq!(json["agent"]["name"], names::MINECRAFT);
    assert_eq!(json["agent"]["version"], 1);
}

/// 认证响应应解析出 accessToken/clientToken/camelCase 角色字段
#[test]
fn test_authenticate_res_obj_parse_selected() {
    let json = r#"{
        "accessToken": "fake-access-token",
        "clientToken": "fake-client-token",
        "selectedProfile": {
            "id": "00000000-0000-0000-0000-00000000bbbb",
            "name": "Steve"
        }
    }"#;

    let obj: AuthenticateResObj = serde_json::from_str(json).unwrap();
    assert_eq!(obj.access_token, "fake-access-token");
    assert_eq!(obj.client_token, "fake-client-token");
    assert_eq!(obj.error_message, None);
    let profile = obj.selected_profile.unwrap();
    assert_eq!(profile.name, "Steve");
    assert_eq!(profile.id, "00000000-0000-0000-0000-00000000bbbb");
    assert!(obj.available_profiles.is_none());
}

/// 多角色响应应解析出 availableProfiles 列表
#[test]
fn test_authenticate_res_obj_parse_available() {
    let json = r#"{
        "accessToken": "fake-access-token",
        "clientToken": "fake-client-token",
        "availableProfiles": [
            { "id": "00000000-0000-0000-0000-00000000bbbb", "name": "Steve" },
            { "id": "00000000-0000-0000-0000-00000000cccc", "name": "Alex" }
        ]
    }"#;

    let obj: AuthenticateResObj = serde_json::from_str(json).unwrap();
    let list = obj.available_profiles.unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].name, "Steve");
    assert_eq!(list[1].name, "Alex");
    assert!(obj.selected_profile.is_none());
}

/// 错误响应应解析出 errorMessage
#[test]
fn test_authenticate_res_obj_parse_error() {
    let json = r#"{
        "error": "ForbiddenOperationException",
        "errorMessage": "Invalid credentials."
    }"#;

    let obj: AuthenticateResObj = serde_json::from_str(json).unwrap();
    assert_eq!(obj.error_message, Some("Invalid credentials.".to_string()));
}

/// 刷新请求：带/不带选定角色的两种序列化形态
#[test]
fn test_refresh_obj_serialize() {
    let with_profile = RefreshObj {
        access_token: "fake-access-token".to_string(),
        client_token: "fake-client-token".to_string(),
        selected_profile: Some(SelectedProfileObj {
            name: "Steve".to_string(),
            id: "00000000-0000-0000-0000-00000000bbbb".to_string(),
        }),
    };
    let json: serde_json::Value = serde_json::to_value(&with_profile).unwrap();
    assert_eq!(json["accessToken"], "fake-access-token");
    assert_eq!(json["clientToken"], "fake-client-token");
    assert_eq!(json["selectedProfile"]["name"], "Steve");
    assert_eq!(
        json["selectedProfile"]["id"],
        "00000000-0000-0000-0000-00000000bbbb"
    );

    let without_profile = RefreshObj {
        access_token: "fake-access-token".to_string(),
        client_token: "fake-client-token".to_string(),
        selected_profile: None,
    };
    let json: serde_json::Value = serde_json::to_value(&without_profile).unwrap();
    // None 应序列化为 null 而非省略字段，与 Yggdrasil 服务端兼容
    assert_eq!(json["selectedProfile"], serde_json::Value::Null);
}

/// `#[serde(default)]`：空响应应解析为默认值而非失败
#[test]
fn test_authenticate_res_obj_empty() {
    let obj: AuthenticateResObj = serde_json::from_str("{}").unwrap();
    assert_eq!(obj.access_token, "");
    assert_eq!(obj.client_token, "");
    assert!(obj.selected_profile.is_none());
    assert!(obj.available_profiles.is_none());
    assert!(obj.error_message.is_none());
}

