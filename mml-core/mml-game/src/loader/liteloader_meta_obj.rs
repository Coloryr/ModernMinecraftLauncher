//! LiteLoader 版本元数据 DTO：liteloader json

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// 仓库信息
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct ReopObj {
    /// 仓库流分支
    pub stream: String,
    /// 仓库地址
    pub url: String,
}

impl Default for ReopObj {
    /// 全字段取默认值：字符串为空串
    fn default() -> Self {
        Self {
            stream: Default::default(),
            url: Default::default(),
        }
    }
}

/// 运行库信息
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct LibrariesObj {
    /// Maven 坐标
    pub name: String,
    /// 下载地址
    pub url: String,
}

impl Default for LibrariesObj {
    /// 全字段取默认值：字符串为空串
    fn default() -> Self {
        Self {
            name: Default::default(),
            url: Default::default(),
        }
    }
}

/// 单个 LiteLoader 版本信息
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct LoaderObj {
    /// tweak 类名
    #[serde(rename = "tweakClass")]
    pub tweak_class: String,
    /// 依赖的运行库
    pub libraries: Vec<LibrariesObj>,
    /// 安装文件名
    pub file: String,
    /// 版本号
    pub version: String,
    /// 文件 MD5
    pub md5: String,
}

impl Default for LoaderObj {
    /// 全字段取默认值：字符串为空串、列表为空
    fn default() -> Self {
        Self {
            tweak_class: Default::default(),
            libraries: Default::default(),
            file: Default::default(),
            version: Default::default(),
            md5: Default::default(),
        }
    }
}

/// 快照版本组
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct SnapshotsObj {
    /// 公共依赖的运行库
    pub libraries: Vec<LibrariesObj>,
    /// 各游戏版本对应的 LiteLoader 版本
    #[serde(rename = "com.mumfrey:liteloader")]
    pub loader: HashMap<String, LoaderObj>,
}

impl Default for SnapshotsObj {
    /// 全字段取默认值：列表为空、映射为空
    fn default() -> Self {
        Self {
            libraries: Default::default(),
            loader: Default::default(),
        }
    }
}

/// 某游戏版本的 LiteLoader 版本信息
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct LiteloaderVersionObj {
    /// 仓库信息
    pub repo: ReopObj,
    /// 快照版本
    pub snapshots: SnapshotsObj,
    /// 正式版本
    pub artefacts: SnapshotsObj,
}

impl Default for LiteloaderVersionObj {
    /// 全字段取默认值：各字段取所属类型的空值
    fn default() -> Self {
        Self {
            repo: Default::default(),
            snapshots: Default::default(),
            artefacts: Default::default(),
        }
    }
}

/// LiteLoader 全部版本元数据
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct LiteloaderMetaObj {
    /// 各游戏版本对应的版本信息
    pub versions: HashMap<String, LiteloaderVersionObj>,
}

impl Default for LiteloaderMetaObj {
    /// 全字段取默认值：版本映射为空
    fn default() -> Self {
        Self {
            versions: Default::default(),
        }
    }
}
