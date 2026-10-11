//! Foojay Disco API
//!
//! foojay.io 聚合了 Temurin / Zulu / Semeru / Corretto 等各 OpenJDK 发行版
//! 的下载元数据，`/disco/v3.0/parameters` 给出全部参数的合法取值，
//! `/disco/v3.0/packages` 按条件筛选可下载的 Java 包。
//! 下载走 `links.pkg_download_redirect`，由服务端 302 到发行方 CDN 直链。

use std::sync::OnceLock;

use mml_names::i18_items::error_type::{CoreResult, DataNotFoundData, ErrorType};
use serde::{Deserialize, Serialize};

use crate::urls;

/// 可选项缓存，首次查询后复用
static OPTIONS: OnceLock<FoojayOptionsObj> = OnceLock::new();

/// 四个下拉框的候选列表
#[derive(Debug, Clone)]
pub struct FoojayOptionsObj {
    /// 发行类型，取值 jdk / jre
    pub types: Vec<String>,
    /// 主版本号，按数值降序
    pub majors: Vec<String>,
    /// 操作系统，如 windows / linux / macos
    pub systems: Vec<String>,
    /// 架构，如 x64 / x86 / aarch64
    pub archs: Vec<String>,
}

/// `/parameters` 响应包装
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct FoojayParametersResultObj {
    /// `/parameters` 返回的参数集合，通常只取第一条
    pub result: Vec<FoojayParametersObj>,
    /// 接口提示信息，成功时为空串
    pub message: String,
}

impl Default for FoojayParametersResultObj {
    fn default() -> Self {
        Self {
            result: Default::default(),
            message: Default::default(),
        }
    }
}

/// `/parameters` 响应中的单条参数集合
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct FoojayParametersObj {
    /// `/packages` 各参数的合法取值
    pub packages: FoojayPackagesParamsObj,
}

impl Default for FoojayParametersObj {
    fn default() -> Self {
        Self {
            packages: Default::default(),
        }
    }
}

/// `/packages` 各参数的合法取值，逗号分隔的字符串
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct FoojayPackagesParamsObj {
    /// 架构，对应查询参数 `architecture`
    pub architecture: String,
    /// 主版本号，对应查询参数 `version`
    pub major_version: String,
    /// 操作系统，对应查询参数 `operating_system`
    pub operating_system: String,
    /// 包类型，对应查询参数 `package_type`，取值 jdk / jre
    pub package_type: String,
}

impl Default for FoojayPackagesParamsObj {
    fn default() -> Self {
        Self {
            architecture: Default::default(),
            major_version: Default::default(),
            operating_system: Default::default(),
            package_type: Default::default(),
        }
    }
}

/// `/major_versions` 响应包装
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct FoojayMajorVersionsResultObj {
    /// `/major_versions` 返回的主版本列表
    pub result: Vec<FoojayMajorVersionObj>,
    /// 接口提示信息，成功时为空串
    pub message: String,
}

impl Default for FoojayMajorVersionsResultObj {
    fn default() -> Self {
        Self {
            result: Default::default(),
            message: Default::default(),
        }
    }
}

/// `/major_versions` 里的单个主版本信息
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct FoojayMajorVersionObj {
    /// 主版本号
    pub major_version: u32,
    /// 支持等级，取值 LTS / MTS / STS
    pub term_of_support: String,
    /// 是否仍在维护
    pub maintained: bool,
    /// 是否仅有早期访问版；为 true 时查 `/packages` 拿不到包
    pub early_access_only: bool,
}

impl Default for FoojayMajorVersionObj {
    fn default() -> Self {
        Self {
            major_version: Default::default(),
            term_of_support: Default::default(),
            maintained: Default::default(),
            early_access_only: Default::default(),
        }
    }
}

/// `/packages` 响应包装
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct FoojayResultObj {
    /// `/packages` 返回的包列表，无匹配时为空
    pub result: Vec<FoojayObj>,
    /// 接口提示信息，成功时为空串
    pub message: String,
}

impl Default for FoojayResultObj {
    fn default() -> Self {
        Self {
            result: Default::default(),
            message: Default::default(),
        }
    }
}

/// Foojay 单个 Java 包
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct FoojayObj {
    /// 包 ID，用于拼下载重定向地址
    pub id: String,
    /// 打包格式，如 msi / exe / zip / tar.gz / pkg
    pub archive_type: String,
    /// 发行版，如 zulu / temurin / semeru
    pub distribution: String,
    /// Java 主版本号
    pub major_version: i32,
    /// OpenJDK 版本号，如 21+35
    pub java_version: String,
    /// 发行方自己的版本号，如 21.28.85
    pub distribution_version: String,
    /// 发布状态，取值 ga / ea
    pub release_status: String,
    /// 目标操作系统
    pub operating_system: String,
    /// 目标架构
    pub architecture: String,
    /// 包类型，取值 jdk / jre
    pub package_type: String,
    /// 是否内置 JavaFX
    pub javafx_bundled: bool,
    /// 是否可经 API 直接下载
    pub directly_downloadable: bool,
    /// 包文件名
    pub filename: String,
    /// 包大小，单位字节；API 对未知大小返回 -1
    pub size: i64,
    /// 相关链接
    pub links: FoojayLinksObj,
}

impl Default for FoojayObj {
    fn default() -> Self {
        Self {
            id: Default::default(),
            archive_type: Default::default(),
            distribution: Default::default(),
            major_version: Default::default(),
            java_version: Default::default(),
            distribution_version: Default::default(),
            release_status: Default::default(),
            operating_system: Default::default(),
            architecture: Default::default(),
            package_type: Default::default(),
            javafx_bundled: Default::default(),
            directly_downloadable: Default::default(),
            filename: Default::default(),
            size: Default::default(),
            links: Default::default(),
        }
    }
}

/// 包的相关链接
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct FoojayLinksObj {
    /// 包详情页
    pub pkg_info_uri: String,
    /// 下载重定向地址，服务端 302 到发行方 CDN 直链
    pub pkg_download_redirect: String,
}

impl Default for FoojayLinksObj {
    fn default() -> Self {
        Self {
            pkg_info_uri: Default::default(),
            pkg_download_redirect: Default::default(),
        }
    }
}

/// 拆分逗号分隔的取值串，去空项
fn split_values(data: &str) -> Vec<String> {
    data.split(',')
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

/// 获取四个下拉框的候选列表：发行类型 / 主版本 / 系统 / 架构
///
/// 类型 / 系统 / 架构来自 `/parameters`；主版本来自 `/major_versions`，
/// 因为 `/parameters` 会把仅有早期访问版的主版本也列出来，如 29+，那些版本
/// 在 `/packages` 里固定 `release_status=ga`，拿不到任何包，得过滤掉。
/// 首次查询后缓存。
///
/// # 返回值
///
/// 返回各下拉框的候选值列表
pub async fn get_options() -> CoreResult<FoojayOptionsObj> {
    if let Some(obj) = OPTIONS.get() {
        return Ok(obj.clone());
    }

    let url = format!("{}parameters", urls::FOOJAY);

    let res = crate::get_work_client()
        .get_json::<FoojayParametersResultObj>(&url)
        .await?;

    let Some(item) = res.result.first() else {
        return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
    };

    let packages = &item.packages;

    let types = split_values(&packages.package_type);

    let systems = split_values(&packages.operating_system);

    let mut archs = split_values(&packages.architecture);
    archs.sort();
    archs.dedup();

    // 主版本只取有 GA 正式版的，按数值降序
    let major_url = format!("{}major_versions", urls::FOOJAY);
    let major_res = crate::get_work_client()
        .get_json::<FoojayMajorVersionsResultObj>(&major_url)
        .await?;
    let mut majors: Vec<u32> = major_res
        .result
        .iter()
        .filter(|item| !item.early_access_only)
        .map(|item| item.major_version)
        .collect();
    majors.sort_unstable_by(|a, b| b.cmp(a));

    let obj = FoojayOptionsObj {
        types,
        majors: majors.iter().map(|item| item.to_string()).collect(),
        systems,
        archs,
    };

    let _ = OPTIONS.set(obj.clone());

    Ok(obj)
}

/// 获取 Java 包列表
///
/// 固定带 `release_status=ga` 与 `directly_downloadable=true`，只返回可直接下载的正式版。
///
/// - `major`: Java 主版本号
/// - `os`: 操作系统，取 API 的 operating_system 取值，如 windows / macos；空串表示不限
/// - `arch`: 架构，如 x64 / aarch64；空串表示不限
/// - `package_type`: 包类型，取值 jdk / jre；空串表示不限
///
/// # 返回值
///
/// 返回符合条件的 Java 包列表
pub async fn get_java_list(
    major: u32,
    os: &str,
    arch: &str,
    package_type: &str,
) -> CoreResult<Vec<FoojayObj>> {
    let mut url = format!("{}packages?version={major}", urls::FOOJAY);

    if !os.is_empty() {
        url += &format!("&operating_system={os}");
    }
    if !arch.is_empty() {
        url += &format!("&architecture={arch}");
    }
    if !package_type.is_empty() {
        url += &format!("&package_type={package_type}");
    }

    url += "&release_status=ga&directly_downloadable=true";

    let res = crate::get_work_client()
        .get_json::<FoojayResultObj>(&url)
        .await?;

    Ok(res.result)
}
