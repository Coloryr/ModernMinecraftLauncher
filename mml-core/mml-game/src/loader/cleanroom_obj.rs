//! Cleanroom 安装器 version.json 的原生库 DTO
//!
//! Cleanroom 的 version.json 是完整清单，带 `natives` 与 `downloads.classifiers`；
//! 普通 Forge 清单里没有这些，所以单独解析，不往共用的 Forge DTO 上挂字段。

use std::collections::HashMap;

use serde::Deserialize;

use crate::mojang::game_arg_obj::ArtifactObj;

/// Cleanroom 清单里分平台原生库的最小结构
#[derive(Deserialize, Default)]
pub struct CleanroomNativesObj {
    /// 运行库列表
    #[serde(default)]
    pub libraries: Vec<CleanroomNativesLibObj>,
}

/// 单个带原生库的运行库
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct CleanroomNativesLibObj {
    /// Maven 坐标名
    pub name: String,
    /// 下载信息
    pub downloads: CleanroomNativesDownloadsObj,
    /// 平台名（windows / linux / osx）→ classifier
    pub natives: HashMap<String, String>,
    /// 适用规则；同一个库会按 x64 / x86 / arm64 各列一条
    pub rules: Vec<CleanroomRuleObj>,
}

/// 原生库的适用规则
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct CleanroomRuleObj {
    /// allow / disallow
    pub action: String,
    /// 操作系统条件
    pub os: CleanroomRuleOsObj,
}

/// 规则里的操作系统条件
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct CleanroomRuleOsObj {
    /// 系统名：windows / linux / osx
    pub name: String,
    /// CPU 架构：x64 / x86 / arm64
    pub arch: Option<String>,
}

/// 原生库下载信息
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct CleanroomNativesDownloadsObj {
    /// classifier → 构件
    pub classifiers: HashMap<String, ArtifactObj>,
}
