//! ColorMC 兼容云同步：SHA1 清单、差异压缩包与 HTTP 客户端。

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::{collections::BTreeMap, fmt, io};

pub mod archive;
pub mod client;

/// 相对文件名到 SHA1 的映射，排序保证修订号稳定。
pub type Manifest = BTreeMap<String, String>;

/// 删除清单在压缩包中的保留名字。
pub const DELETE_FILE: &str = "cloud_delete.json";
/// 单个请求压缩包上限，单位字节。
pub const MAX_ARCHIVE: usize = 256 * 1024 * 1024;
/// 单个快照解压上限，单位字节。
pub const MAX_UNPACKED: u64 = 1024 * 1024 * 1024;
/// 单个快照文件数上限。
pub const MAX_FILES: usize = 100_000;

/// 云同步错误，key 由调用界面翻译，detail 保留原始异常。
#[derive(Debug)]
pub struct Error {
    /// 界面语言表键。
    pub key: &'static str,
    /// 文件系统或 HTTP 的原始异常。
    pub detail: String,
}

impl Error {
    /// 创建没有额外异常文本的协议错误。
    pub fn new(key: &'static str) -> Self {
        Self {
            key,
            detail: String::new(),
        }
    }

    /// 把 ColorMC 返回码转换成界面错误。
    pub fn protocol(code: i64) -> Self {
        Self::new(match code {
            101 => "err.cloudNotEnabled",
            102 => "err.cloudMissing",
            104 => "err.cloudQuota",
            105 => "err.cloudConflict",
            300 => "err.cloudAuth",
            _ => "err.cloudProtocol",
        })
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.key, self.detail)
    }
}
impl std::error::Error for Error {}
impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self {
            key: "err.cloudIo",
            detail: err.to_string(),
        }
    }
}
impl From<zip::result::ZipError> for Error {
    fn from(err: zip::result::ZipError) -> Self {
        Self {
            key: "err.cloudArchive",
            detail: err.to_string(),
        }
    }
}
impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self {
            key: "err.cloudProtocol",
            detail: err.to_string(),
        }
    }
}
impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        // reqwest 异常可能带 URL；不把密钥放进 URL。
        Self {
            key: "err.cloudNetwork",
            detail: err.without_url().to_string(),
        }
    }
}

/// 云同步操作结果。
pub type Result<T> = std::result::Result<T, Error>;

/// 云端存档元数据，与 ColorMC 的 JSON 字段保持一致。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct World {
    /// saves 下的目录名。
    pub name: String,
    /// 服务器 UTC 更新时间。
    pub time: String,
    /// 可选的 Base64 PNG，缺省为空。
    #[serde(default)]
    pub icon: String,
    /// 完整文件清单。
    #[serde(default)]
    pub files: Manifest,
}

/// 生成小写 SHA1。
///
/// - `bytes`: 要计算的数据
///
/// # 返回值
///
/// 40 位十六进制字符串。
pub fn sha1(bytes: &[u8]) -> String {
    hex(&Sha1::digest(bytes))
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// 计算清单修订号，用于拒绝覆盖并发更新。
///
/// - `manifest`: 排序后的文件清单
///
/// # 返回值
///
/// 清单 JSON 的 SHA1。
pub fn revision(manifest: &Manifest) -> String {
    sha1(&serde_json::to_vec(manifest).expect("string map"))
}
