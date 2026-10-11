//! Adoptium (Eclipse Temurin) API
//!
//! 提供从 Adoptium 官方 API 获取可用 Java 版本列表和下载链接的功能。
//! Adoptium 是 Eclipse 基金会维护的 OpenJDK 发行版，前身为 AdoptOpenJDK。

use std::sync::OnceLock;

use mml_names::i18_items::error_type::ErrorType;
use serde::{Deserialize, Serialize};

use crate::urls::ADOPTIUM_URL;

/// 可用 Java 主版本缓存
static JAVA_VERSION: OnceLock<Vec<String>> = OnceLock::new();

/// 下载包信息，对应 `binary.package`
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct PackageObj {
    /// 包校验值，对应 `checksum`
    pub checksum: String,
    /// 下载链接，对应 `link`
    pub link: String,
    /// 包文件名，对应 `name`
    pub name: String,
    /// 包大小，单位字节，对应 `size`
    pub size: i64,
}

impl Default for PackageObj {
    fn default() -> Self {
        Self {
            checksum: String::new(),
            link: String::new(),
            name: String::new(),
            size: 0,
        }
    }
}

/// 构建产物二进制信息，对应 `ADOPTIUM_URL` 的 `binary`
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct BinaryObj {
    /// 目标 CPU 架构，对应 `architecture`，如 `x64` / `aarch64`
    pub architecture: String,
    /// 镜像类型，对应 `image_type`，取值 `jdk` / `jre`
    pub image_type: String,
    /// 目标操作系统，对应 `os`
    pub os: String,
    /// 下载包信息，对应 `package`
    pub package: PackageObj,
    /// 源码版本引用，对应 `scm_ref`
    pub scm_ref: String,
    /// 产物文件名，对应 `name`
    pub name: String,
}

impl Default for BinaryObj {
    fn default() -> Self {
        Self {
            architecture: Default::default(),
            image_type: Default::default(),
            os: Default::default(),
            package: Default::default(),
            scm_ref: Default::default(),
            name: Default::default(),
        }
    }
}

/// OpenJDK 版本信息，对应 `ADOPTIUM_URL` 的 `version`
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct AdoptiumVersionObj {
    /// OpenJDK 完整版本号，对应 `semver`
    pub semver: String,
}

impl Default for AdoptiumVersionObj {
    fn default() -> Self {
        Self {
            semver: Default::default(),
        }
    }
}

/// Adoptium 单个 Java 发行项，对应版本列表响应 `[]` 的元素
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct AdoptiumObj {
    /// 二进制产物信息，对应 `binary`
    pub binary: BinaryObj,
    /// OpenJDK 版本信息，对应 `version`
    pub version: AdoptiumVersionObj,
}

impl Default for AdoptiumObj {
    fn default() -> Self {
        Self {
            binary: Default::default(),
            version: Default::default(),
        }
    }
}

/// Adoptium 可用版本信息，对应 `v3/info/available_releases` 的响应体
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct AdoptiumJavaVersionObj {
    /// 可用的 Java 主版本列表，对应 `available_releases`
    pub available_releases: Vec<i32>,
}

impl Default for AdoptiumJavaVersionObj {
    fn default() -> Self {
        Self {
            available_releases: Vec::new(),
        }
    }
}

/// 获取支持的 Java 主版本
///
/// # 返回值
///
/// 返回可用的 Java 主版本列表，首次查询后缓存
pub async fn get_java_version() -> Result<Vec<String>, ErrorType> {
    if let Some(list) = JAVA_VERSION.get() {
        return Ok(list.to_vec());
    }

    let url = ADOPTIUM_URL.to_string() + "v3/info/available_releases";

    let res = crate::get_work_client()
        .get_json::<AdoptiumJavaVersionObj>(&url)
        .await?;

    let mut list = Vec::new();
    for item in res.available_releases.iter() {
        list.push(item.to_string());
    }

    // 并发下可能已被别的任务写入，写失败不影响本次返回
    let _ = JAVA_VERSION.set(list.clone());

    Ok(list)
}

/// 获取 Java 文件列表
///
/// - `version`: Java 主版本
/// - `os`: 系统类型，传给接口的 `os` 参数
///
/// # 返回值
///
/// 返回该版本各构建产物的下载信息列表
pub async fn get_java_list(version: u32, os: &str) -> Result<Vec<AdoptiumObj>, ErrorType> {
    let mut url = String::from(ADOPTIUM_URL);
    url += &format!("v3/assets/latest/{}/hotspot?os={}", version, os);

    let res = crate::get_work_client()
        .get_json::<Vec<AdoptiumObj>>(&url)
        .await?;

    Ok(res)
}
