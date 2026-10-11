//! Java 下载窗口 DTO
use serde::{Deserialize, Serialize};

/// Java 搜索源，wire 名小写，即源名称本身
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum JavaTypes {
    /// Adoptium，即 Eclipse Temurin
    Adoptium,
    /// Azul Zulu
    Zulu,
    /// IBM Semeru，基于 OpenJ9
    OpenJ9,
    /// Foojay Disco，聚合各发行版
    Foojay,
}

impl JavaTypes {
    /// 全部搜索源，即下拉框的候选列表
    ///
    /// # 返回值
    ///
    /// 返回四个搜索源，顺序即下拉框展示顺序
    pub fn all() -> Vec<Self> {
        vec![Self::Adoptium, Self::Zulu, Self::OpenJ9, Self::Foojay]
    }
}

/// Java 下载列表项，即按当前筛选条件匹配出的单个包
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaDownloadItemDto {
    /// 包 ID，搜索源侧标识，下载时用
    pub uuid: String,
    /// 名字
    pub name: String,
    /// OpenJDK 版本号，如 21.0.5+11
    pub java_version: String,
    /// 包文件名
    pub filename: String,
    /// 包大小，单位字节
    pub size: u64,
}

/// Java 下载可选项，即四个下拉框的候选列表
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaDownloadOptionsDto {
    /// 发行类型：JDK / JRE
    pub types: Vec<String>,
    /// 主版本号：8 / 11 / 17 / 21 …
    pub majors: Vec<u32>,
    /// 系统：windows / linux / macos
    pub systems: Vec<String>,
    /// 架构：x86 / x64 / arm64
    pub archs: Vec<String>,
}
