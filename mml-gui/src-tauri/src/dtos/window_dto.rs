//! 窗口尺寸 DTO——前端 JS 回退路径创建窗口时从后端取默认宽高。
//!
//! 窗口几何的唯一数据源在后端，即 `WINDOWS_INFO` 的默认尺寸 + `window_save.json`
//! 的历史几何；前端不再重复维护一份宽高表。

use serde::{Deserialize, Serialize};

/// 单个窗口的默认尺寸，kind 为前端窗口标识，如 "settings"
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowSizeDto {
    /// 前端窗口标识，如 "settings"
    pub kind: String,
    /// 默认宽度，单位逻辑像素
    pub width: f64,
    /// 默认高度，单位逻辑像素
    pub height: f64,
}

/// 拖入路径的探测结果，由 `window_probe_drop` 返回
///
/// 前端据此决定"拖进来的这个东西该往哪送"：文件夹 / 整合包压缩包 → 「添加实例」窗口，
/// 登录快捷方式 → 「账户管理」窗口，其余不接。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DropPathDto {
    /// `folder` / `archive` / `shortcut` / `other`
    pub kind: String,
    /// `kind = shortcut` 时：从快捷方式里解析出的地址，即认证服务器 / 网址；其余为空串
    pub url: String,
}

/// 拖入文件的真实路径，由 DOM 拖放的"路径桥"上报，见 [`crate::windows::file_drop`]
///
/// DOM 的 `drop` 事件里 `File` 对象**没有路径**，这是浏览器的安全设计，所以文件拖入由注入
/// 脚本把 `File` 对象交给原生、原生取出路径后发这条事件；文本 / 链接那类不走它。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDropDto {
    /// 拖入文件的绝对路径列表
    pub paths: Vec<String>,
}
