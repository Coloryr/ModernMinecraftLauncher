//! Xbox Live / XSTS 认证数据模型
//!
//! 本模块定义了与 Xbox Live 认证服务 XBL 和 Xbox 安全令牌服务 XSTS
//! 通信所需的 JSON 数据结构。
//!
//! # 认证流程中的位置
//!
//! Microsoft Token → **Xbox Live** → **XSTS** → Minecraft Token → Profile
//!
//! # 核心类型
//!
//! - [`XBoxLoginObj`] — XBL 认证请求
//! - [`XSTSLoginObj`] — XSTS 认证请求
//! - [`XBoxLoginResObj`] — 统一的认证响应结构
//! - [`XBoxLiveResObj`] — 提取后的认证结果，含 token 与 UHS

use serde::{Deserialize, Serialize};

/// Xbox Live 认证请求的属性部分
///
/// 包含认证方法、站点名称和从 Microsoft Token 生成的 RPS 票据。
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct XBoxLoginPropertiesObj {
    /// 认证方法，固定为 "RPS"
    #[serde(rename = "AuthMethod")]
    pub auth_method: String,
    /// 站点名称，固定为 "user.auth.xboxlive.com"
    #[serde(rename = "SiteName")]
    pub site_name: String,
    /// RPS 票据，格式为 "d={Microsoft Access Token}"
    #[serde(rename = "RpsTicket")]
    pub rps_ticket: String,
}

impl Default for XBoxLoginPropertiesObj {
    fn default() -> Self {
        Self {
            auth_method: Default::default(),
            site_name: Default::default(),
            rps_ticket: Default::default(),
        }
    }
}

/// Xbox Live 认证请求
///
/// 发送至 `https://user.auth.xboxlive.com/user/authenticate`。
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct XBoxLoginObj {
    /// 认证属性
    #[serde(rename = "Properties")]
    pub properties: XBoxLoginPropertiesObj,
    /// 依赖方标识，固定为 "http://auth.xboxlive.com"
    #[serde(rename = "RelyingParty")]
    pub relying_party: String,
    /// 令牌类型，固定为 "JWT"
    #[serde(rename = "TokenType")]
    pub token_type: String,
}

impl Default for XBoxLoginObj {
    fn default() -> Self {
        Self {
            properties: Default::default(),
            relying_party: Default::default(),
            token_type: Default::default(),
        }
    }
}

/// XSTS 认证请求的属性部分
///
/// 包含沙盒标识和从 XBL 获取的令牌。
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct XSTSLoginPropertiesObj {
    /// 沙盒标识，固定为 "RETAIL"，即零售版 Minecraft
    #[serde(rename = "SandboxId")]
    pub sandbox_id: String,
    /// 上游令牌列表，即 XBL Token
    #[serde(rename = "UserTokens")]
    pub user_tokens: Vec<String>,
}

impl Default for XSTSLoginPropertiesObj {
    fn default() -> Self {
        Self {
            sandbox_id: Default::default(),
            user_tokens: Default::default(),
        }
    }
}

/// XSTS 认证请求
///
/// 发送至 `https://xsts.auth.xboxlive.com/xsts/authorize`。
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct XSTSLoginObj {
    /// 认证属性
    #[serde(rename = "Properties")]
    pub properties: XSTSLoginPropertiesObj,
    /// 依赖方标识，固定为 "rp://api.minecraftservices.com/"
    #[serde(rename = "RelyingParty")]
    pub relying_party: String,
    /// 令牌类型，固定为 "JWT"
    #[serde(rename = "TokenType")]
    pub token_type: String,
}

impl Default for XSTSLoginObj {
    fn default() -> Self {
        Self {
            properties: Default::default(),
            relying_party: Default::default(),
            token_type: Default::default(),
        }
    }
}

/// Xbox Live 显示声明中的 XUI 条目
///
/// 包含认证返回的用户哈希，即 User Hash，在后续获取
/// Minecraft Token 时需要用到。
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct XBoxLoginDisplayClaimsXuiObj {
    /// 用户哈希，即 User Hash，用于 Minecraft 认证
    pub uhs: String,
}

impl Default for XBoxLoginDisplayClaimsXuiObj {
    fn default() -> Self {
        Self {
            uhs: Default::default(),
        }
    }
}

/// Xbox Live 认证响应中的显示声明部分
///
/// 包含 XUI 条目列表，其中第一个条目包含用户哈希。
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct XBoxLoginDisplayClaimsObj {
    /// XUI 声明列表
    pub xui: Vec<XBoxLoginDisplayClaimsXuiObj>,
}

impl Default for XBoxLoginDisplayClaimsObj {
    fn default() -> Self {
        Self {
            xui: Default::default(),
        }
    }
}

/// Xbox Live 和 XSTS 认证的统一响应结构
///
/// 两个端点返回格式相同，因此共用此结构体。
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct XBoxLoginResObj {
    /// 认证令牌，JWT 格式
    #[serde(rename = "Token")]
    pub token: String,
    /// 显示声明，包含用户哈希信息
    #[serde(rename = "DisplayClaims")]
    pub display_claims: XBoxLoginDisplayClaimsObj,
}

impl Default for XBoxLoginResObj {
    fn default() -> Self {
        Self {
            token: Default::default(),
            display_claims: Default::default(),
        }
    }
}

/// Xbox Live 认证成功后提取的结果
///
/// 包含后续步骤所需的令牌和用户哈希。
pub struct XBoxLiveResObj {
    /// XBL 或 XSTS 令牌
    pub xbl_token: String,
    /// 用户哈希 UHS，用于 Minecraft 服务认证
    pub xbl_uhs: String,
}

#[cfg(test)]
mod tests {
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
}
