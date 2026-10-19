//! Modrinth 用户 DTO
//!
//! 批量查用户用（`users?ids=[...]`）：搜索结果只给作者名与作者 ID，头像要从这里取。

use serde::{Deserialize, Serialize};

/// Modrinth 用户，对应 `users?ids=` 返回数组里的元素
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct ModrinthUserObj {
    /// 用户 ID，与搜索命中项里的 `author_id` 对应
    pub id: String,
    /// 用户名
    pub username: String,
    /// 头像 URL，可缺省为 `null`
    pub avatar_url: Option<String>,
}
