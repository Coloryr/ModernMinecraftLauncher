//! 添加资源窗口 DTO 的公共小件：下载源定位、作者 / 标签 / 截图、Modrinth 链接
//!
//! 从 `add_resource_dto/mod.rs` 拆出来的：这些类型被文件列表、项目列表、项目详情三处共用。

use serde::{Deserialize, Serialize};

use mml_game::launcher::FileType;
use mml_net::urls;

/// 下载源定位信息，pid + fid 唯一确定一个资源 / 文件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceTypeDto {
    /// 资源类型，即 `FileType` 的字符串形态
    pub file_type: String,
    /// 下载源：CurseForge / Modrinth
    pub source: String,
    /// 项目 ID，即源站的项目标识
    pub pid: String,
    /// 文件 ID，即项目下的版本 / 文件标识；列表级条目还没有具体文件时为空串
    pub fid: String,
}

/// MC 百科 mcmod.cn 的翻译条目
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McmodDto {
    /// 百科页面 ID
    pub mcmod_id: String,
    /// 百科页面名
    pub mcmod_name: String,
}

/// 图像条目，如作者头像
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PicDto {
    /// 名字
    pub name: String,
    /// 图像地址，经 mml-image 转发，无则 None
    pub logo: Option<String>,
}

/// 标签条目
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagDto {
    /// 标签名
    pub name: String,
    /// 位图图标地址，来自 CurseForge，无则 None
    pub logo: Option<String>,
    /// SVG 图标内容，来自 Modrinth，无则 None
    pub svg: Option<String>,
}

/// 截图条目
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecPicDto {
    /// 标题
    pub name: String,
    /// 图像地址，经 mml-image 转发
    pub logo: String,
    /// 描述
    pub description: String,
}

/// 拼接 Modrinth 项目网页地址
///
/// - `file_type`: 资源类型，决定 URL 段
/// - `id`: 项目 ID
///
/// # 返回值
///
/// 返回项目网页地址
pub(super) fn get_modrinth_url(file_type: &FileType, id: &str) -> String {
    format!(
        "{}{}/{id}",
        urls::MODRINTH,
        match file_type {
            FileType::Modpack => "modpack",
            FileType::Shaderpack => "shaders",
            FileType::Resourcepack => "resourcepacks",
            FileType::DataPacks => "datapacks",
            _ => "mod",
        }
    )
}
