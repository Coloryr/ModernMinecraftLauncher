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
#[derive(Serialize, Deserialize, Debug, Default)]
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
    ///
    /// # 返回值
    ///
    /// 新建的代理信息：`name` 与 `version` 按 `use_minecraft` 取原版或本启动器的值
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

/// 认证请求对象
///
/// 发送给 `/authserver/authenticate` 端点的登录请求体。
#[derive(Serialize, Deserialize, Debug, Default)]
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

/// 认证响应对象
///
/// `/authserver/authenticate` 和 `/authserver/refresh` 端点的响应体。
/// 可能包含错误信息、选中的角色或可选角色列表。
#[derive(Serialize, Deserialize, Debug, Default)]
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

/// 令牌刷新请求对象
///
/// 发送给 `/authserver/refresh` 端点的刷新请求体。
#[derive(Serialize, Deserialize, Debug, Default)]
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

/// 可选角色/账户信息
///
/// 表示 Yggdrasil 认证服务器返回的一个 Minecraft 游戏角色。
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct SelectedProfileObj {
    /// 角色名称，即玩家用户名
    pub name: String,
    /// 角色 UUID，Minecraft 格式，含连字符
    pub id: String,
}

#[cfg(test)]
#[path = "../../tests/unit/legacy_authenticate_obj_tests.rs"]
mod tests;
