use super::*;

/// 设备码端点响应应正确解析，微软实际返回中还含 interval / message 等多余字段
#[test]
fn test_oauth_obj_parse_device_code() {
    // 假数据，字段布局与微软设备授权端点一致
    let json = r#"{
        "user_code": "ABCD-EFGH",
        "device_code": "FAKE_DEVICE_CODE",
        "verification_uri": "https://microsoft.com/link",
        "expires_in": 900,
        "interval": 5,
        "message": "To sign in, use a web browser to open the page"
    }"#;

    let obj: OAuthObj = serde_json::from_str(json).unwrap();
    assert_eq!(obj.user_code, "ABCD-EFGH");
    assert_eq!(obj.device_code, "FAKE_DEVICE_CODE");
    assert_eq!(obj.verification_uri, "https://microsoft.com/link");
    assert_eq!(obj.expires_in, 900);
    assert_eq!(obj.error, None);
}

/// 带错误信息的设备码响应，如客户端 ID 无效
#[test]
fn test_oauth_obj_parse_error() {
    let json = r#"{
        "error": "invalid_client",
        "error_description": "AADSTS7000218: invalid client"
    }"#;

    let obj: OAuthObj = serde_json::from_str(json).unwrap();
    assert_eq!(obj.error, Some("invalid_client".to_string()));
    // `#[serde(default)]`：错误响应中没有设备码字段时应为空串而非解析失败
    assert_eq!(obj.device_code, "");
}

/// OAuthObj 序列化往返
#[test]
fn test_oauth_obj_round_trip() {
    let obj = OAuthObj {
        user_code: "CODE".to_string(),
        error: None,
        device_code: "DEV".to_string(),
        verification_uri: "https://microsoft.com/link".to_string(),
        expires_in: 900,
    };
    let json = serde_json::to_string(&obj).unwrap();
    let back: OAuthObj = serde_json::from_str(&json).unwrap();
    assert_eq!(back.user_code, "CODE");
    assert_eq!(back.device_code, "DEV");
    assert_eq!(back.expires_in, 900);
}

/// 令牌端点成功响应应解析出 access / refresh token，多余字段忽略
#[test]
fn test_oauth_get_code_obj_parse_success() {
    let json = r#"{
        "token_type": "Bearer",
        "scope": "XboxLive.signin offline_access",
        "expires_in": 86400,
        "access_token": "fake-access-token",
        "refresh_token": "fake-refresh-token"
    }"#;

    let obj: OAuthGetCodeObj = serde_json::from_str(json).unwrap();
    assert_eq!(obj.error, None);
    assert_eq!(obj.access_token, "fake-access-token");
    assert_eq!(obj.refresh_token, "fake-refresh-token");
}

/// 令牌端点轮询期间的各种错误响应
#[test]
fn test_oauth_get_code_obj_parse_pending() {
    // authorization_pending / slow_down：轮询期间的无 token 响应
    let json = r#"{
        "error": "authorization_pending",
        "error_description": "AADSTS70016: pending end-user authorization"
    }"#;
    let obj: OAuthGetCodeObj = serde_json::from_str(json).unwrap();
    assert_eq!(obj.error, Some("authorization_pending".to_string()));
    // `#[serde(default)]`：错误响应中缺少 token 字段时应为空串
    assert_eq!(obj.access_token, "");
    assert_eq!(obj.refresh_token, "");

    let json = r#"{"error": "slow_down"}"#;
    let obj: OAuthGetCodeObj = serde_json::from_str(json).unwrap();
    assert_eq!(obj.error, Some("slow_down".to_string()));

    let json = r#"{"error": "expired_token"}"#;
    let obj: OAuthGetCodeObj = serde_json::from_str(json).unwrap();
    assert_eq!(obj.error, Some("expired_token".to_string()));
}

/// OAuthGetCodeObj 序列化往返
#[test]
fn test_oauth_get_code_obj_round_trip() {
    let obj = OAuthGetCodeObj {
        error: None,
        access_token: "at".to_string(),
        refresh_token: "rt".to_string(),
    };
    let json = serde_json::to_string(&obj).unwrap();
    let back: OAuthGetCodeObj = serde_json::from_str(&json).unwrap();
    assert_eq!(back.access_token, "at");
    assert_eq!(back.refresh_token, "rt");
    assert_eq!(back.error, None);
}

/// 设备码流程第一步的返回结构字段透传
#[test]
fn test_oauth_get_code_res_fields() {
    let res = OAuthGetCodeResObj {
        code: "ABCD-EFGH".to_string(),
        url: "https://microsoft.com/link".to_string(),
        device_code: "DEV".to_string(),
        expires_in: 900,
    };
    assert_eq!(res.code, "ABCD-EFGH");
    assert_eq!(res.url, "https://microsoft.com/link");
    assert_eq!(res.device_code, "DEV");
    assert_eq!(res.expires_in, 900);
}

