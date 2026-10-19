//! Maven 坐标工具
//!
//! 提供 Maven 坐标，即 Gradle 风格坐标，与文件路径之间的转换，
//! 以及从 Maven 仓库获取文件哈希校验值的功能。

use mml_base::file_item::FileHash;
use mml_config::config_obj::SourceLocal;
use mml_names::names;

use crate::{url_helper, urls};

/// SHA1 校验值的十六进制长度
const SHA1_HEX_LEN: usize = 40;

/// SHA256 校验值的十六进制长度
const SHA256_HEX_LEN: usize = 64;

/// SHA512 校验值的十六进制长度
const SHA512_HEX_LEN: usize = 128;

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
/// - `name`: Maven 坐标，形如 `com.example:artifact:1.0`；段数不足 3 时按 `组.名` 直接换路径
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
/// 依次尝试 `.sha256` / `.sha512` / `.sha1` 同名校验文件。Maven 仓库不保证每个构件都发全
/// 这三种：Forge 的 `1.12.2-14.23.5.2864` 就只有 `.sha1`，缺的那种会回 404 加一页 HTML
/// 错误页。所以这里只认成功响应，且正文必须真的长成该算法的校验值，否则继续往后试。
///
/// - `url`: 文件 URL
///
/// # 返回值
///
/// 返回第一个能拿到的校验值；全部拿不到时返回 [`FileHash::None`]
pub async fn try_get_hash(url: &str) -> FileHash {
    let client = crate::get_work_client();

    let sha256_url = url.to_string() + names::SHA256_DOT_EXT;
    if let Some(hash) = fetch_hash(&client, &sha256_url, SHA256_HEX_LEN).await {
        return FileHash::Sha256(hash);
    }

    let sha512_url = url.to_string() + names::SHA512_DOT_EXT;
    if let Some(hash) = fetch_hash(&client, &sha512_url, SHA512_HEX_LEN).await {
        return FileHash::Sha512(hash);
    }

    let sha1_url = url.to_string() + names::SHA1_DOT_EXT;
    if let Some(hash) = fetch_hash(&client, &sha1_url, SHA1_HEX_LEN).await {
        return FileHash::Sha1(hash);
    }

    FileHash::None
}

/// 取一个校验文件，只接受成功响应、且正文是校验值形状的响应
///
/// - `client`: 网络客户端
/// - `url`: 校验文件 URL
/// - `len`: 该算法的十六进制长度
///
/// # 返回值
///
/// 拿到合法校验值时返回小写形式，否则返回 `None`
async fn fetch_hash(client: &crate::Client, url: &str, len: usize) -> Option<String> {
    let text = client.get_text_success(url).await.ok()?;
    parse_hash_text(&text, len)
}

/// 从校验文件正文里取出校验值
///
/// 正文一般就是一个十六进制串，少数仓库会在后面跟一个文件名，所以只看第一个空白分隔的词。
/// 光看状态码挡不住仓库用 200 回错误页的情况，这里再按长度与字符集卡一道。
///
/// - `text`: 校验文件正文
/// - `len`: 该算法的十六进制长度
///
/// # 返回值
///
/// 形状对得上返回小写校验值，否则返回 `None`
fn parse_hash_text(text: &str, len: usize) -> Option<String> {
    let hash = text.split_whitespace().next()?;
    (hash.len() == len && hash.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| hash.to_ascii_lowercase())
}
