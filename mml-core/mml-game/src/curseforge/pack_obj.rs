//! CurseForge 整合包清单 manifest.json 的 DTO

use serde::{Deserialize, Serialize};

/// CurseForge 整合包 manifest.json 的顶层结构
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct CurseForgePackObj {
    /// Minecraft 段：游戏版本与加载器
    pub minecraft: MinecraftObj,
    /// 清单类型，固定为 `minecraftModpack`
    #[serde(rename = "manifestType")]
    pub manifest_type: String,
    /// 清单格式版本号，目前为 1
    #[serde(rename = "manifestVersion")]
    pub manifest_version: i32,
    /// 整合包名称
    pub name: String,
    /// 整合包版本，作者自定义的字符串
    pub version: String,
    /// 作者名
    pub author: String,
    /// 需要下载的文件列表，空列表表示只靠 overrides
    pub files: Vec<FilesObj>,
    /// 覆盖包在压缩包里的目录名，通常是 `overrides`
    pub overrides: String,
}

impl Default for CurseForgePackObj {
    /// 全字段取默认值
    fn default() -> Self {
        Self {
            minecraft: Default::default(),
            manifest_type: Default::default(),
            manifest_version: Default::default(),
            name: Default::default(),
            version: Default::default(),
            author: Default::default(),
            files: Default::default(),
            overrides: Default::default(),
        }
    }
}

/// manifest 的 minecraft 段
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct MinecraftObj {
    /// 目标游戏版本，如 `1.21.1`
    pub version: String,
    /// 加载器列表，通常只有一个
    #[serde(rename = "modLoaders")]
    pub mod_loaders: Vec<ModLoadersObj>,
}

impl Default for MinecraftObj {
    /// 全字段取默认值
    fn default() -> Self {
        Self {
            version: Default::default(),
            mod_loaders: Default::default(),
        }
    }
}

/// 单个模组加载器条目
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct ModLoadersObj {
    /// 加载器 ID，如 `forge-14.23.5.2859`
    pub id: String,
    /// 是否为主加载器，决定启动时用哪一套加载链
    pub primary: bool,
}

impl Default for ModLoadersObj {
    /// 全字段取默认值
    fn default() -> Self {
        Self {
            id: Default::default(),
            primary: Default::default(),
        }
    }
}

/// 整合包内需要下载的单个文件
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct FilesObj {
    /// CurseForge 项目 ID
    #[serde(rename = "projectID")]
    pub project_id: u64,
    /// CurseForge 文件 ID
    #[serde(rename = "fileID")]
    pub file_id: u64,
    /// 是否必选，`false` 表示可选安装
    pub required: bool,
}

impl Default for FilesObj {
    /// 全字段取默认值
    fn default() -> Self {
        Self {
            project_id: Default::default(),
            file_id: Default::default(),
            required: Default::default(),
        }
    }
}
