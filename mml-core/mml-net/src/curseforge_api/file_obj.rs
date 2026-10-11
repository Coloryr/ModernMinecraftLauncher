//! CurseForge 文件 DTO

use serde::{Deserialize, Serialize};

use crate::curseforge_api::list_obj::CurseForgeListPaginationObj;

/// 文件列表接口 `/mods/{id}/files` 的响应
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct CurseFogreFilePageObj {
    /// 文件列表
    pub data: Vec<CurseForgeFileDataObj>,
    /// 分页信息
    pub pagination: CurseForgeListPaginationObj,
}

impl Default for CurseFogreFilePageObj {
    fn default() -> Self {
        Self {
            data: Default::default(),
            pagination: Default::default(),
        }
    }
}

/// 单文件接口 `/mods/{pid}/files/{fid}` 的响应
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct CurseForgeFileObj {
    /// 文件数据
    pub data: CurseForgeFileDataObj,
}

impl Default for CurseForgeFileObj {
    fn default() -> Self {
        Self {
            data: Default::default(),
        }
    }
}

/// 单个文件信息
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct CurseForgeFileDataObj {
    /// 文件 ID
    pub id: u64,
    /// 所属模组 ID
    #[serde(rename = "modId")]
    pub mod_id: u64,
    /// 显示名称，可含空格与中文
    #[serde(rename = "displayName")]
    pub display_name: String,
    /// 文件名
    #[serde(rename = "fileName")]
    pub file_name: String,
    /// 文件哈希列表，含 SHA-1 与 MD5
    #[serde(rename = "hashes")]
    pub hashes: Vec<HashesObj>,
    /// 上传时间，ISO 8601 UTC 字符串
    #[serde(rename = "fileDate")]
    pub file_date: String,
    /// 文件大小，单位字节
    #[serde(rename = "fileLength")]
    pub file_length: u64,
    /// 下载量
    #[serde(rename = "downloadCount")]
    pub download_count: u64,
    /// 下载地址；接口对不允许分发的文件返回 null，可缺省
    #[serde(rename = "downloadUrl")]
    pub download_url: Option<String>,
    /// 依赖文件列表，接口未返回时为 `None`
    pub dependencies: Option<Vec<DependenciesObj>>,
}

impl Default for CurseForgeFileDataObj {
    fn default() -> Self {
        Self {
            id: Default::default(),
            mod_id: Default::default(),
            display_name: Default::default(),
            file_name: Default::default(),
            hashes: Default::default(),
            file_date: Default::default(),
            file_length: Default::default(),
            download_count: Default::default(),
            download_url: Default::default(),
            dependencies: Default::default(),
        }
    }
}

/// 文件哈希
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct HashesObj {
    /// 哈希值的十六进制小写字符串
    pub value: String,
    /// 哈希算法，1=SHA-1，2=MD5
    pub algo: i32,
}

impl Default for HashesObj {
    fn default() -> Self {
        Self {
            value: Default::default(),
            algo: Default::default(),
        }
    }
}

/// 依赖文件信息
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct DependenciesObj {
    /// 依赖的模组 ID
    #[serde(rename = "modId")]
    pub mod_id: u64,
    /// 依赖关系类型，1=嵌入，2=可选，3=必选，4=工具，5=不兼容
    #[serde(rename = "relationType")]
    pub relation_type: i32,
}

impl Default for DependenciesObj {
    fn default() -> Self {
        Self {
            mod_id: Default::default(),
            relation_type: Default::default(),
        }
    }
}
