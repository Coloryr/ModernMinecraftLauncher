//! LiteLoader 版本元数据 DTO：liteloader json

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// 仓库信息
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct ReopObj {
    /// 仓库流分支
    pub stream: String,
    /// 仓库地址
    pub url: String,
}

/// 运行库信息
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct LibrariesObj {
    /// Maven 坐标
    pub name: String,
    /// 下载地址
    pub url: String,
}

/// 单个 LiteLoader 版本信息
#[derive(Serialize, Deserialize, Debug, Default)]
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

/// 快照版本组
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct SnapshotsObj {
    /// 公共依赖的运行库
    pub libraries: Vec<LibrariesObj>,
    /// 各游戏版本对应的 LiteLoader 版本
    #[serde(rename = "com.mumfrey:liteloader")]
    pub loader: HashMap<String, LoaderObj>,
}

/// 某游戏版本的 LiteLoader 版本信息
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct LiteloaderVersionObj {
    /// 仓库信息
    pub repo: ReopObj,
    /// 快照版本
    pub snapshots: SnapshotsObj,
    /// 正式版本
    pub artefacts: SnapshotsObj,
}

/// LiteLoader 全部版本元数据
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct LiteloaderMetaObj {
    /// 各游戏版本对应的版本信息
    pub versions: HashMap<String, LiteloaderVersionObj>,
}
