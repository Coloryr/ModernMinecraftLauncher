use serde::{Deserialize, Serialize};

/// 文件在线信息
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct OnlineInfoObj {
    /// 游戏路径文件夹
    #[serde(rename = "Path")]
    pub path: String,
    /// 名字
    #[serde(rename = "Name")]
    pub name: String,
    /// 文件名
    #[serde(rename = "File")]
    pub file: String,
    /// 校验值
    #[serde(rename = "SHA1")]
    pub sha1: String,
    /// 下载连接
    #[serde(rename = "Url")]
    pub url: String,
    /// 模组ID
    #[serde(rename = "ModId")]
    pub modid: String,
    /// 文件ID
    #[serde(rename = "FileId")]
    pub fileid: String,
}
