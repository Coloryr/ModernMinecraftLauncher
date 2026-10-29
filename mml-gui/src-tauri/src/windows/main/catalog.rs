//! 主窗口的"目录类"数据：版本清单缓存与 MOTD 地址解析
//!
//! 从 `main/mod.rs` 拆出来的纯逻辑，不碰窗口、不碰模型，连同它自己的单测。

use std::sync::{LazyLock, RwLock};

use crate::dtos::VersionInfoDto;

/// 版本列表缓存，进程级：主窗口 / 添加实例窗口共用，不挂在窗口模型上
pub(super) static VERSIONS_CACHE: LazyLock<RwLock<Vec<VersionInfoDto>>> =
    LazyLock::new(|| RwLock::new(Vec::new()));

/// 从 mml_net 拉取官方版本清单
///
/// 按类型分组排序，正式版 > 快照 > 旧版 Beta > 旧版 Alpha，组内保持清单顺序，
/// 因为清单本身按新旧排列。
///
/// # 返回值
///
/// 版本列表；拉取或解析失败返回空列表
pub(super) async fn fetch_versions() -> Vec<VersionInfoDto> {
    /// 版本清单 JSON 的顶层结构，只取 `versions`
    #[derive(serde::Deserialize)]
    struct Manifest {
        /// 清单里的全部版本条目，顺序即新旧顺序
        versions: Vec<ManifestVersion>,
    }
    /// 清单里的单个版本条目
    #[derive(serde::Deserialize)]
    struct ManifestVersion {
        /// 版本号，如 `1.21.1` / `25w14a`
        id: String,
        /// 版本类型，取值 `release` / `snapshot` / `old_beta` / `old_alpha`
        #[serde(rename = "type")]
        version_type: String,
    }
    let Ok(bytes) = mml_net::mojang_api::get_versions(None).await else {
        return Vec::new();
    };
    let Ok(manifest) = serde_json::from_slice::<Manifest>(&bytes) else {
        return Vec::new();
    };
    let mut list: Vec<VersionInfoDto> = manifest
        .versions
        .into_iter()
        .map(|v| VersionInfoDto {
            id: v.id,
            version_type: v.version_type,
        })
        .collect();
    // 只按类型分组排序，组内不动：清单本身即最新在前，
    // 按版本号数值重排会把快照 25w14a 排到 1.21.x 之上、打乱 rc / 旧版顺序
    list.sort_by_key(|v| match v.version_type.as_str() {
        "release" => 0,
        "snapshot" => 1,
        "old_beta" => 2,
        "old_alpha" => 3,
        _ => 4,
    });
    // 不截断：快照 / 旧版类型也要有数据，否则切换版本类型后过滤结果为空
    list
}

/// MOTD 展示的默认端口
const MOTD_DEFAULT_PORT: u16 = 25565;

/// 解析出的服务器地址
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MotdAddr {
    /// 主机名或 IP；地址为空时是空串
    pub(super) host: String,
    /// 端口；没写端口或地址为空时取 [`MOTD_DEFAULT_PORT`]
    pub(super) port: u16,
}

/// 解析服务器地址：host / host:port / [IPv6]:port / 裸 IPv6
///
/// - `address`: 用户输入的服务器地址，前后空白会被去掉
///
/// # 返回值
///
/// 见 [`MotdAddr`]
pub(super) fn parse_motd_addr(address: &str) -> MotdAddr {
    let addr = address.trim();
    if addr.is_empty() {
        return MotdAddr {
            host: String::new(),
            port: MOTD_DEFAULT_PORT,
        };
    }
    // [IPv6]:port
    if let Some(rest) = addr.strip_prefix('[')
        && let Some((host, port)) = rest.split_once(']')
    {
        let port = port
            .strip_prefix(':')
            .and_then(|p| p.parse().ok())
            .unwrap_or(MOTD_DEFAULT_PORT);
        return MotdAddr {
            host: host.to_string(),
            port,
        };
    }
    // 只有一个冒号才算 host:port，多个冒号视为裸 IPv6
    if let Some((host, port)) = addr.rsplit_once(':')
        && !host.contains(':')
        && !port.is_empty()
        && let Ok(p) = port.parse::<u16>()
    {
        return MotdAddr {
            host: host.to_string(),
            port: p,
        };
    }
    MotdAddr {
        host: addr.to_string(),
        port: MOTD_DEFAULT_PORT,
    }
}

#[cfg(test)]
mod tests {
    use super::{MotdAddr, parse_motd_addr};

    /// 造一个期望的解析结果
    ///
    /// - `host`: 期望主机
    /// - `port`: 期望端口
    fn addr(host: &str, port: u16) -> MotdAddr {
        MotdAddr {
            host: host.into(),
            port,
        }
    }

    /// 各种地址写法的解析结果，含空串与裸 IPv6
    #[test]
    fn test_parse_motd_addr() {
        assert_eq!(
            parse_motd_addr("mc.example.com"),
            addr("mc.example.com", 25565)
        );
        assert_eq!(
            parse_motd_addr("mc.example.com:25566"),
            addr("mc.example.com", 25566)
        );
        assert_eq!(
            parse_motd_addr("  mc.example.com:25566 "),
            addr("mc.example.com", 25566)
        );
        assert_eq!(parse_motd_addr("[::1]:25565"), addr("::1", 25565));
        assert_eq!(parse_motd_addr("::1"), addr("::1", 25565));
        assert_eq!(parse_motd_addr(""), addr("", 25565));
    }
}
