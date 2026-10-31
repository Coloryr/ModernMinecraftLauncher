use serde::{Deserialize, Serialize};

/// 公共存储中的一份资源文件
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct ResourceInfoObj {
    /// 项目 ID
    pub pid: String,
    /// 显示名
    pub name: String,
    /// 文件 SHA1，小写
    pub hash: String,
    /// 公共目录下的文件名
    pub file: String,
    /// 下载地址；重新下载缺失的公共文件时使用
    pub url: String,
}

/// 实例内的一条资源映射
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct LinkInfoObj {
    /// 是否真的建成了硬链接；跨盘回退为复制时为 `false`
    pub is_link: bool,
    /// 项目 ID
    pub pid: String,
    /// 文件 ID
    pub fid: String,
    /// 实例根目录下的相对路径，使用 `/` 分隔
    pub path: String,
}

/// 资源类型
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceType {
    /// 模组
    Mod,
    /// 资源包
    ResourcePack,
}
