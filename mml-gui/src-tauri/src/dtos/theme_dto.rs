//! 主题包类型 DTO：TS 命名，camelCase wire
//!
//! 三种换皮包共用同一套类型取值，前端据此显示类型与「未声明类型」提示。
//! 判定与校验在 `windows/theme`，这里只负责 wire 形态。

use serde::{Deserialize, Serialize};

/// 主题包类型，wire 取值是 `home` / `css` / `page`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemeKindDto {
    /// 自定义主页面：替换主窗口主页内容区
    Home,
    /// 自定义 CSS：可装多份，按条目启用
    Css,
    /// 完全自定义界面：整页 / 按 kind 替换窗口
    Page,
}
