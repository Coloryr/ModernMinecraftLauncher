//! Modrinth 项目团队成员 DTO

use serde::{Deserialize, Serialize};

/// Modrinth 项目团队成员信息，对应项目成员接口返回数组的元素
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct ModrinthTeamObj {
    /// 成员的用户信息，对应 `user`
    pub user: TeamserObj,
}

/// 团队成员的用户信息，对应 `user`
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct TeamserObj {
    /// 用户名，对应 `username`
    pub username: String,
    /// 头像 URL，对应 `avatar_url`，可缺省为 `null`
    pub avatar_url: Option<String>,
}
