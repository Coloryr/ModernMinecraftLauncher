//! 前置依赖 DTO：给「下载资源」窗口的前置勾选弹窗用

use serde::{Deserialize, Serialize};

/// 一个待勾选的前置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceDependencyDto {
    /// 项目 ID
    pub pid: String,
    /// 自动选中的文件 ID，即 `files` 的第一个；没有可用文件时是空串
    pub fid: String,
    /// 项目名
    pub name: String,
    /// 图标地址，经 image_manager 转发；没有时为 None
    pub icon: Option<String>,
    /// 是否可选：必选的默认勾上，可选的默认不勾
    pub optional: bool,
    /// 候选文件，与本实例的版本 / 加载器匹配的那些，第一个是自动选中的；可以空
    pub files: Vec<ResourceDependencyFileDto>,
}

/// 前置的一个候选文件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceDependencyFileDto {
    /// 文件 ID
    pub fid: String,
    /// 显示名，即版本名
    pub name: String,
    /// 发布时间
    pub time: String,
}
