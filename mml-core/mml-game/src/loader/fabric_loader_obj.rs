//! Fabric Loader 版本 JSON DTO

use serde::{Deserialize, Serialize};

/// Fabric 运行库
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct LibrariesObj {
    /// Maven 坐标，形如 group:artifact:version
    pub name: String,
    /// 下载地址
    pub url: String,
    /// SHA-256 校验值，十六进制串
    pub sha256: String,
}

impl Default for LibrariesObj {
    fn default() -> Self {
        Self {
            name: Default::default(),
            url: Default::default(),
            sha256: Default::default(),
        }
    }
}

/// Fabric 启动参数
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct FabricArgumentsObj {
    /// 游戏参数
    pub game: Vec<String>,
    /// JVM 参数
    pub jvm: Vec<String>,
}

impl Default for FabricArgumentsObj {
    fn default() -> Self {
        Self {
            game: Default::default(),
            jvm: Default::default(),
        }
    }
}

/// Fabric Loader 版本信息
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct FabricLoaderObj {
    /// 加载器 ID
    pub id: String,
    /// 主类
    #[serde(rename = "mainClass")]
    pub main_class: String,
    /// 启动参数
    pub arguments: FabricArgumentsObj,
    /// 运行库列表
    pub libraries: Vec<LibrariesObj>,
}

impl Default for FabricLoaderObj {
    fn default() -> Self {
        Self {
            id: Default::default(),
            main_class: Default::default(),
            arguments: Default::default(),
            libraries: Default::default(),
        }
    }
}

/// 加载器版本项
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct FabricLoaderVersionItemObj {
    /// 版本号
    pub version: String,
}

impl Default for FabricLoaderVersionItemObj {
    fn default() -> Self {
        Self {
            version: Default::default(),
        }
    }
}

/// 版本 JSON 中的加载器信息
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct FabricLoaderVersionObj {
    /// 加载器版本
    pub loader: FabricLoaderVersionItemObj,
}

impl Default for FabricLoaderVersionObj {
    fn default() -> Self {
        Self {
            loader: Default::default(),
        }
    }
}
