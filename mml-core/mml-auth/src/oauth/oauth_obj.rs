//! Microsoft OAuth 2.0 数据模型
//!
//! 本模块定义了与 Microsoft 设备授权端点通信时
//! 所需的序列化数据结构。

use serde::{Deserialize, Serialize};

/// OAuth 设备码授权——第一步返回结果
///
/// 包含用户完成浏览器授权所需的信息。
#[derive(Clone)]
pub struct OAuthGetCodeResObj {
    /// 用户需要在浏览器中输入的设备码
    pub code: String,
    /// 用户需要访问的验证网址，如 `https://microsoft.com/link`
    pub url: String,
    /// 设备码，用于后续轮询令牌
    pub device_code: String,
    /// 设备码的有效期，单位秒，超时后需重新获取
    pub expires_in: i64,
}

/// OAuth 设备码获取请求的响应
///
/// 来自 Microsoft 设备授权端点的原始 JSON 响应。
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct OAuthObj {
    /// 用户码，显示给用户，用于手动输入
    pub user_code: String,
    /// 错误信息，授权失败时才有值
    pub error: Option<String>,
    /// 设备码，用于后续令牌轮询
    pub device_code: String,
    /// 验证网址，用户需访问的 URL
    pub verification_uri: String,
    /// 有效时间，单位秒
    pub expires_in: i64,
}

/// OAuth 令牌获取请求的响应
///
/// 轮询 Microsoft 令牌端点后返回的结果。
/// 成功时 `access_token` 和 `refresh_token` 非空，`error` 为 `None`。
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct OAuthGetCodeObj {
    /// 错误码：
    /// - `authorization_pending` — 用户尚未完成授权
    /// - `slow_down` — 轮询频率过高
    /// - `expired_token` — 设备码已过期
    pub error: Option<String>,
    /// Microsoft OAuth 访问令牌
    pub access_token: String,
    /// Microsoft OAuth 刷新令牌，用于长期保持登录状态
    pub refresh_token: String,
}

#[cfg(test)]
mod tests {
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
}
