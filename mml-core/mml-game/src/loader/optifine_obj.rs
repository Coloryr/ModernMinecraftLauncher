//! OptiFine 安装信息，即实例目录内保存的那份版本记录

use mml_config::config_obj::SourceLocal;
use mml_net::optifine_api::GetOptifineObj;
use serde::{Deserialize, Serialize};

/// OptiFine 安装信息，落盘在实例目录
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct OptifineObj {
    /// OptiFine 版本号
    #[serde(rename = "Version")]
    pub version: String,
    /// 目标游戏版本号
    #[serde(rename = "MCVersion")]
    pub mc_version: String,
    /// 配套的 Forge 版本，空串表示不需要 Forge
    #[serde(rename = "Forge")]
    pub forge: String,
    /// 安装包文件名
    #[serde(rename = "FileName")]
    pub file_name: String,
    /// 发布日期，官方源给的原始字符串
    #[serde(rename = "Date")]
    pub date: String,
    /// 下载地址：官方源是下载页，镜像源是 JAR 地址
    #[serde(rename = "Url1")]
    pub url1: Option<String>,
    /// 备用下载地址，仅官方源有
    #[serde(rename = "Url2")]
    pub url2: Option<String>,
    /// 数据来自哪个下载源
    #[serde(rename = "Local")]
    pub source: SourceLocal,
}

impl From<GetOptifineObj> for OptifineObj {
    /// 在线接口返回的结构转成本地落盘结构
    ///
    /// - `value`: 在线接口返回的 OptiFine 信息
    ///
    /// # 返回值
    ///
    /// 返回字段一一对应的本地结构
    fn from(value: GetOptifineObj) -> Self {
        OptifineObj {
            version: value.version,
            mc_version: value.mc_version,
            forge: value.forge,
            file_name: value.file_name,
            date: value.date,
            url1: value.url1,
            url2: value.url2,
            source: value.source,
        }
    }
}

impl Default for OptifineObj {
    /// 全字段取默认值
    fn default() -> Self {
        Self {
            version: Default::default(),
            mc_version: Default::default(),
            forge: Default::default(),
            file_name: Default::default(),
            date: Default::default(),
            url1: Default::default(),
            url2: Default::default(),
            source: Default::default(),
        }
    }
}
