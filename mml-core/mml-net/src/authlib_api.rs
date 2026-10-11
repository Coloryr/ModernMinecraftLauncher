//! Authlib-Injector API
//!
//! 提供从 Authlib-Injector 项目获取最新版本信息和下载链接的功能。
//! Authlib-Injector 是一个用于 Minecraft 外置登录的 JAR 注入工具。

use std::sync::LazyLock;

use mml_names::i18_items::error_type::{CoreResult, DataNotFoundData, ErrorType};
use serde::{Deserialize, Serialize};

use crate::url_helper;

/// Authlib-Injector 元数据，对应元数据接口的响应体
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct AuthlibInjectorMetaObj {
    /// 最新构建号，对应 `latest_build_number`
    pub latest_build_number: i32,
    /// 各版本构建产物列表，对应 `artifacts`
    pub artifacts: Vec<ArtifactsObj>,
}

impl Default for AuthlibInjectorMetaObj {
    fn default() -> Self {
        Self {
            latest_build_number: Default::default(),
            artifacts: Default::default(),
        }
    }
}

/// 单个构建产物，对应 `artifacts[]` 的元素
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct ArtifactsObj {
    /// 构建号，对应 `build_number`，与 `latest_build_number` 比对挑最新
    pub build_number: i32,
}

impl Default for ArtifactsObj {
    fn default() -> Self {
        Self {
            build_number: Default::default(),
        }
    }
}

/// Authlib-Injector 下载信息，对应单个构建的详情接口响应
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct AuthlibInjectorObj {
    /// 构建号，对应 `build_number`
    pub build_number: i32,
    /// 版本号，对应 `version`
    pub version: String,
    /// 下载 URL，对应 `download_url`
    pub download_url: String,
    /// 校验值，对应 `checksums`
    pub checksums: ChecksumsObj,
}

impl Default for AuthlibInjectorObj {
    fn default() -> Self {
        Self {
            build_number: Default::default(),
            version: Default::default(),
            download_url: Default::default(),
            checksums: Default::default(),
        }
    }
}

/// 校验值集合，对应详情接口的 `checksums`
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct ChecksumsObj {
    /// SHA-256 哈希，对应 `sha256`，下载后校验用
    pub sha256: String,
}

impl Default for ChecksumsObj {
    fn default() -> Self {
        Self {
            sha256: Default::default(),
        }
    }
}

/// 内置的 Authlib-Injector 兜底版本，元数据接口不可用时使用
pub static LOCAL_AUTHLIB: LazyLock<AuthlibInjectorObj> = LazyLock::new(|| AuthlibInjectorObj {
    build_number: 55,
    version: String::from("1.2.7"),
    download_url: String::from(
        "https://authlib-injector.yushi.moe/artifact/55/authlib-injector-1.2.7.jar",
    ),
    checksums: ChecksumsObj {
        sha256: String::from("eaf14bc5acffc7d885bd5bd5942b99f36d6299302beae356b2fc5807fe42652b"),
    },
});

/// 获取最新 AuthlibInjector 信息
///
/// # 返回值
///
/// 返回最新构建的下载信息；找不到对应构建号时返回 `DataNotFound`
pub async fn get_obj() -> CoreResult<AuthlibInjectorObj> {
    let url = url_helper::get_authlib_injector_meta();
    let meta = crate::get_work_client()
        .get_json::<AuthlibInjectorMetaObj>(&url)
        .await?;

    let item = meta
        .artifacts
        .iter()
        .find(|item| item.build_number == meta.latest_build_number);

    match item {
        None => Err(ErrorType::DataNotFound(DataNotFoundData::Info)),
        Some(data) => Ok(crate::get_work_client()
            .get_json::<AuthlibInjectorObj>(&url_helper::get_authlib_injector(data))
            .await?),
    }
}
