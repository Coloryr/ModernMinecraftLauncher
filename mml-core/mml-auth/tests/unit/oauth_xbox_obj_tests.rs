use super::*;

/// XBL 认证请求应序列化为 Xbox 要求的 PascalCase 字段名
#[test]
fn test_xbox_login_obj_serialize() {
    let obj = XBoxLoginObj {
        properties: XBoxLoginPropertiesObj {
            auth_method: "RPS".to_string(),
            site_name: "user.auth.xboxlive.com".to_string(),
            rps_ticket: "d=fake-token".to_string(),
        },
        relying_party: "http://auth.xboxlive.com".to_string(),
        token_type: "JWT".to_string(),
    };

    let json: serde_json::Value = serde_json::to_value(&obj).unwrap();
    let map = json.as_object().unwrap();
    for key in ["Properties", "RelyingParty", "TokenType"] {
        assert!(map.contains_key(key), "缺少字段 {}", key);
    }
    assert_eq!(map["Properties"]["AuthMethod"], "RPS");
    assert_eq!(map["Properties"]["SiteName"], "user.auth.xboxlive.com");
    assert_eq!(map["Properties"]["RpsTicket"], "d=fake-token");
    assert_eq!(map["RelyingParty"], "http://auth.xboxlive.com");
    assert_eq!(map["TokenType"], "JWT");
}

/// XSTS 认证请求应序列化为 Xbox 要求的 PascalCase 字段名
#[test]
fn test_xsts_login_obj_serialize() {
    let obj = XSTSLoginObj {
        properties: XSTSLoginPropertiesObj {
            sandbox_id: "RETAIL".to_string(),
            user_tokens: vec!["fake-xbl-token".to_string()],
        },
        relying_party: "rp://api.minecraftservices.com/".to_string(),
        token_type: "JWT".to_string(),
    };

    let json: serde_json::Value = serde_json::to_value(&obj).unwrap();
    assert_eq!(json["Properties"]["SandboxId"], "RETAIL");
    assert_eq!(json["Properties"]["UserTokens"][0], "fake-xbl-token");
    assert_eq!(json["RelyingParty"], "rp://api.minecraftservices.com/");
    assert_eq!(json["TokenType"], "JWT");
}

/// XBL/XSTS 统一响应应正确解析出 Token 和 UHS
#[test]
fn test_xbox_login_res_obj_parse() {
    // 假数据，字段布局与 Xbox 认证端点一致，xui 条目中还可能有其它字段
    let json = r#"{
        "IssueInstant": "2026-01-01T00:00:00.0000000Z",
        "NotAfter": "2026-01-02T00:00:00.0000000Z",
        "Token": "fake-xsts-token",
        "DisplayClaims": {
            "xui": [
                { "nhe": "1", "uhs": "fake-uhs" }
            ]
        }
    }"#;

    let obj: XBoxLoginResObj = serde_json::from_str(json).unwrap();
    assert_eq!(obj.token, "fake-xsts-token");
    assert_eq!(obj.display_claims.xui.len(), 1);
    assert_eq!(obj.display_claims.xui[0].uhs, "fake-uhs");
}

/// `#[serde(default)]`：空响应应解析为默认值而非失败
#[test]
fn test_xbox_login_res_obj_empty() {
    let obj: XBoxLoginResObj = serde_json::from_str("{}").unwrap();
    assert_eq!(obj.token, "");
    assert!(obj.display_claims.xui.is_empty());
}

/// XBoxLiveResObj 字段透传
#[test]
fn test_xbox_live_res_fields() {
    let res = XBoxLiveResObj {
        xbl_token: "fake-token".to_string(),
        xbl_uhs: "fake-uhs".to_string(),
    };
    assert_eq!(res.xbl_token, "fake-token");
    assert_eq!(res.xbl_uhs, "fake-uhs");
}

