//! Modrinth 版本 DTO

use serde::{Deserialize, Serialize};

/// Modrinth 项目版本，对应版本接口返回的单个版本对象
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct ModrinthVersionObj {
    /// 版本 ID，对应 `id`
    pub id: String,
    /// 所属项目 ID，对应 `project_id`
    pub project_id: String,
    /// 版本名称，对应 `name`
    pub name: String,
    /// 版本号，对应 `version_number`
    pub version_number: String,
    /// 发布时间，对应 `date_published`，ISO 8601 字符串
    pub date_published: String,
    /// 下载量，对应 `downloads`
    pub downloads: u64,
    /// 该版本包含的文件列表，对应 `files`
    pub files: Vec<ModrinthVersionFileObj>,
    /// 依赖列表，对应 `dependencies`
    pub dependencies: Vec<DependencieObj>,
}

impl Default for ModrinthVersionObj {
    fn default() -> Self {
        Self {
            id: Default::default(),
            project_id: Default::default(),
            name: Default::default(),
            version_number: Default::default(),
            date_published: Default::default(),
            downloads: Default::default(),
            files: Default::default(),
            dependencies: Default::default(),
        }
    }
}

/// 版本内的单个文件，对应 `files[]` 的元素
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct ModrinthVersionFileObj {
    /// 文件哈希，对应 `hashes`，含 sha1 与 sha512
    pub hashes: HasheObj,
    /// 下载 URL，对应 `url`
    pub url: String,
    /// 文件名，对应 `filename`
    pub filename: String,
    /// 是否为主要文件，对应 `primary`，一个版本通常只有一个
    pub primary: bool,
    /// 文件大小，单位字节，对应 `size`
    pub size: u64,
}

impl Default for ModrinthVersionFileObj {
    fn default() -> Self {
        Self {
            hashes: Default::default(),
            url: Default::default(),
            filename: Default::default(),
            primary: Default::default(),
            size: Default::default(),
        }
    }
}

/// 文件哈希集合，对应 `hashes`
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct HasheObj {
    /// SHA-1 哈希，对应 `sha1`
    pub sha1: String,
    /// SHA-512 哈希，对应 `sha512`
    pub sha512: String,
}

impl Default for HasheObj {
    fn default() -> Self {
        Self {
            sha1: Default::default(),
            sha512: Default::default(),
        }
    }
}

/// 版本依赖，对应 `dependencies[]` 的元素
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct DependencieObj {
    /// 依赖的版本 ID，对应 `version_id`，可缺省为 `null`
    pub version_id: Option<String>,
    /// 依赖的项目 ID，对应 `project_id`，可缺省为 `null`；
    /// 内置打包文件如 Mod Menu Helper.zip 就没有这一项
    pub project_id: Option<String>,
    /// 依赖类型，对应 `dependency_type`，取值 `required` 必选 / `optional` 可选
    pub dependency_type: String,
}

impl Default for DependencieObj {
    fn default() -> Self {
        Self {
            version_id: Default::default(),
            project_id: Default::default(),
            dependency_type: Default::default(),
        }
    }
}
