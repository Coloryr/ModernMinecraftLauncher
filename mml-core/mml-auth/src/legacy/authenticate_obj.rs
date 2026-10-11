//! Yggdrasil 认证协议的数据结构定义
//!
//! 本模块定义了与 Yggdrasil 认证服务器交互所需的 JSON 数据结构。
//! 这些结构体用于序列化请求和反序列化响应，遵循 Mojang Yggdrasil API 规范。

use mml_names::names;
use serde::{Deserialize, Serialize};

/// 启动器代理信息
///
/// 标识发起认证请求的启动器客户端。服务器可能根据此信息
/// 进行版本兼容性判断或统计。
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct AgentObj {
    /// 启动器名称，如 "Minecraft" 或 "M²L"
    pub name: String,
    /// 启动器协议版本号
    pub version: i32,
}

impl AgentObj {
    /// 创建启动器代理信息
    ///
    /// - `use_minecraft`: 是否伪装为 Minecraft 原版启动器头
    ///   - `true` → 使用 "Minecraft" 名称和版本 1
    ///   - `false` → 使用本启动器名称和当前版本号
    pub fn new(use_minecraft: bool) -> Self {
        AgentObj {
            name: String::from(if use_minecraft {
                names::MINECRAFT
            } else {
                names::MML
            }),
            version: if use_minecraft {
                1
            } else {
                mml_names::VERSION_NUM
            },
        }
    }
}

impl Default for AgentObj {
    fn default() -> Self {
        Self {
            name: Default::default(),
            version: Default::default(),
        }
    }
}

/// 认证请求对象
///
/// 发送给 `/authserver/authenticate` 端点的登录请求体。
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct AuthenticateObj {
    /// 启动器代理信息
    pub agent: AgentObj,
    /// 用户名，通常是邮箱地址
    pub username: String,
    /// 密码
    pub password: String,
    /// 客户端标识令牌，由启动器生成并持久化
    #[serde(rename = "clientToken")]
    pub client_token: String,
}

impl Default for AuthenticateObj {
    fn default() -> Self {
        Self {
            agent: Default::default(),
            username: Default::default(),
            password: Default::default(),
            client_token: Default::default(),
        }
    }
}

/// 认证响应对象
///
/// `/authserver/authenticate` 和 `/authserver/refresh` 端点的响应体。
/// 可能包含错误信息、选中的角色或可选角色列表。
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct AuthenticateResObj {
    /// 登录访问令牌，即 access token
    #[serde(rename = "accessToken")]
    pub access_token: String,
    /// 客户端标识令牌
    #[serde(rename = "clientToken")]
    pub client_token: String,
    /// 服务器选定的角色，单角色时非空
    #[serde(rename = "selectedProfile")]
    pub selected_profile: Option<SelectedProfileObj>,
    /// 可用角色列表，多角色时非空
    #[serde(rename = "availableProfiles")]
    pub available_profiles: Option<Vec<SelectedProfileObj>>,
    /// 错误消息，认证失败时非空
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
}

impl Default for AuthenticateResObj {
    fn default() -> Self {
        Self {
            access_token: Default::default(),
            client_token: Default::default(),
            selected_profile: Default::default(),
            available_profiles: Default::default(),
            error_message: Default::default(),
        }
    }
}

/// 令牌刷新请求对象
///
/// 发送给 `/authserver/refresh` 端点的刷新请求体。
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct RefreshObj {
    /// 当前登录访问令牌
    #[serde(rename = "accessToken")]
    pub access_token: String,
    /// 客户端标识令牌
    #[serde(rename = "clientToken")]
    pub client_token: String,
    /// 要选定的角色，可为空，为空时仅刷新令牌
    #[serde(rename = "selectedProfile")]
    pub selected_profile: Option<SelectedProfileObj>,
}

impl Default for RefreshObj {
    fn default() -> Self {
        Self {
            access_token: Default::default(),
            client_token: Default::default(),
            selected_profile: Default::default(),
        }
    }
}

/// 可选角色/账户信息
///
/// 表示 Yggdrasil 认证服务器返回的一个 Minecraft 游戏角色。
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct SelectedProfileObj {
    /// 角色名称，即玩家用户名
    pub name: String,
    /// 角色 UUID，Minecraft 格式，含连字符
    pub id: String,
}

impl Default for SelectedProfileObj {
    fn default() -> Self {
        Self {
            name: Default::default(),
            id: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
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
}
