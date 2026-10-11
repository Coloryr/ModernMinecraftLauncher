//! Maven 坐标工具
//!
//! 提供 Maven 坐标，即 Gradle 风格坐标，与文件路径之间的转换，
//! 以及从 Maven 仓库获取文件哈希校验值的功能。

use mml_base::file_item::FileHash;
use mml_config::config_obj::SourceLocal;
use mml_names::names;

use crate::{url_helper, urls};

/// 将 Maven 坐标名转换为相对文件路径
///
/// # 转换规则
///
/// - `"group:artifact:version"` → `"group/artifact/version/artifact-version.jar"`
/// - `"group:artifact:version:ext"` → `"group/artifact/version/artifact-version-ext.jar"`
///
/// # 示例
///
/// ```
/// use mml_net::maven_utils::version_name_to_path;
///
/// // 标准坐标
/// version_name_to_path("com.example:artifact:1.0");
/// // -> "com/example/artifact/1.0/artifact-1.0.jar"
///
/// // 带扩展名
/// version_name_to_path("com.example:artifact:1.0:sources");
/// // -> "com/example/artifact/1.0/artifact-1.0-sources.jar"
/// ```
///
/// # 返回值
///
/// 返回 Maven 仓库内的相对路径
pub fn version_name_to_path(name: &str) -> String {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() < 3 {
        format!("{}.jar", name.replace('.', "/"))
    } else if parts.len() > 3 {
        format!(
            "{}/{}/{}/{}-{}-{}.jar",
            parts[0].replace('.', "/"),
            parts[1],
            parts[2],
            parts[1],
            parts[2],
            parts[3]
        )
    } else {
        format!(
            "{}/{}/{}/{}-{}.jar",
            parts[0].replace('.', "/"),
            parts[1],
            parts[2],
            parts[1],
            parts[2]
        )
    }
}

/// Maven 文件的哈希与下载地址
pub struct UrlHashObj {
    /// 文件校验值
    pub hash: FileHash,
    /// 文件下载 URL，已按当前下载源拼好
    pub url: String,
}

/// 测试这个 jar 文件是否能从网上下载
///
/// - `dir`: jar 文件在 Maven 仓库内的相对路径
///
/// # 返回值
///
/// 返回校验值与完整下载 URL；所有校验文件都拿不到时返回 `None`
pub async fn test_hash(dir: &str) -> Option<UrlHashObj> {
    let url = match url_helper::get_source() {
        SourceLocal::Offical => urls::MAVEN,
        SourceLocal::Bmclapi => urls::MAVEN_ALIYUN,
    };

    let url1 = String::from(url) + dir;

    let hash = try_get_hash(&url1).await;
    if matches!(hash, FileHash::None) {
        None
    } else {
        Some(UrlHashObj { hash, url: url1 })
    }
}

/// 尝试获取校验值
///
/// 依次尝试 `.sha256` / `.sha512` / `.sha1` 同名校验文件。
///
/// - `url`: 文件 URL
///
/// # 返回值
///
/// 返回第一个能拿到的校验值；全部拿不到时返回 [`FileHash::None`]
pub async fn try_get_hash(url: &str) -> FileHash {
    let sha1_url = url.to_string() + names::SHA1_DOT_EXT;
    let sha256_url = url.to_string() + names::SHA256_DOT_EXT;
    let sha512_url = url.to_string() + names::SHA512_DOT_EXT;

    let client = crate::get_work_client();

    if let Ok(data) = client.get_text(&sha256_url).await {
        FileHash::Sha256(data)
    } else if let Ok(data) = client.get_text(&sha512_url).await {
        FileHash::Sha512(data)
    } else if let Ok(data) = client.get_text(&sha1_url).await {
        FileHash::Sha1(data)
    } else {
        FileHash::None
    }
}
