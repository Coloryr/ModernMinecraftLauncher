//! OpenFrp 联机平台 API
//!
//! 提供通道列表查询、通道配置查询与 frpc 客户端下载信息。

use std::collections::HashMap;

use mml_base::file_item::{FileHash, FileItemObj, LaterRun};
use mml_names::i18_items::error_type::{CoreResult, ErrorType};
use mml_sys::Os;
use serde::{Deserialize, Serialize};

use crate::urls;

/// 通道列表，对应 `OPENFRP` 取通道接口的响应体
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct OpenFrpChannelObj {
    /// 节点分组列表，对应 `data`
    pub data: Vec<OpenFrpChannelData>,
}

impl Default for OpenFrpChannelObj {
    fn default() -> Self {
        Self {
            data: Default::default(),
        }
    }
}

/// 通道分组，对应 `data[]` 的元素
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct OpenFrpChannelData {
    /// 分组名称，对应 `name`
    pub name: String,
    /// 组内节点列表，对应 `proxies`
    pub proxies: Vec<ProxieObj>,
}

impl Default for OpenFrpChannelData {
    fn default() -> Self {
        Self {
            name: Default::default(),
            proxies: Default::default(),
        }
    }
}

/// 单个节点，对应 `proxies[]` 的元素
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct ProxieObj {
    /// 节点名称，对应 `name`
    pub name: String,
    /// 节点 ID，对应 `id`，查节点配置时作为 `proxy` 参数
    pub id: i32,
    /// 节点协议类型，对应 `type`
    #[serde(rename = "type")]
    pub p_type: String,
    /// 远程访问地址，对应 `remote`
    pub remote: String,
}

impl Default for ProxieObj {
    fn default() -> Self {
        Self {
            name: Default::default(),
            id: Default::default(),
            p_type: Default::default(),
            remote: Default::default(),
        }
    }
}

/// 通道配置信息，对应 `OPENFRP` 取节点配置接口的响应体
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct OpenFrpChannelInfoObj {
    /// 配置键值对，对应 `proxies`，直接落进 frpc 的 ini 配置
    pub proxies: HashMap<String, String>,
}

impl Default for OpenFrpChannelInfoObj {
    fn default() -> Self {
        Self {
            proxies: Default::default(),
        }
    }
}

/// frpc 客户端下载信息，对应 `OPENFRP_DOWNLOAD` 的响应体
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct OpenFrpDownloadObj {
    /// 下载配置，对应 `data`
    pub data: OpenFrpDownloadItemObj,
}

impl Default for OpenFrpDownloadObj {
    fn default() -> Self {
        Self {
            data: Default::default(),
        }
    }
}

/// frpc 下载配置，对应响应体的 `data`
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct OpenFrpDownloadItemObj {
    /// 最新版本号，对应 `latest`，是拼下载地址的路径段
    pub latest: String,
    /// 最新完整版本号，对应 `latest_full`
    pub latest_full: String,
    /// 下载源列表，对应 `source`，取第一个即可
    pub source: Vec<SourceObj>,
}

impl Default for OpenFrpDownloadItemObj {
    fn default() -> Self {
        Self {
            latest: Default::default(),
            latest_full: Default::default(),
            source: Default::default(),
        }
    }
}

/// 下载源，对应 `source[]` 的元素
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct SourceObj {
    /// 源地址前缀，对应 `value`，下载地址 = 前缀 + 版本号 + 文件名
    pub value: String,
}

impl Default for SourceObj {
    fn default() -> Self {
        Self {
            value: Default::default(),
        }
    }
}

/// 获取通道列表
///
/// - `key`: 账户密钥
///
/// # 返回值
///
/// 返回账户下所有节点分组
pub async fn get_channel(key: &str) -> CoreResult<OpenFrpChannelObj> {
    let client = crate::get_work_client();
    let url = format!("{}?action=getallproxies&user={key}", urls::OPENFRP);

    client.get_json(&url).await
}

/// 获取通道配置
///
/// - `key`: 账户密钥
/// - `id`: 通道 ID，取节点列表里的 `id`
///
/// # 返回值
///
/// 返回该节点的配置键值对
pub async fn get_channel_config(key: &str, id: i32) -> CoreResult<OpenFrpChannelInfoObj> {
    let client = crate::get_work_client();
    let url = format!("{}?action=getproxy&proxy={id}&user={key}", urls::OPENFRP);

    client.get_json(&url).await
}

/// 获取下载列表
///
/// # 返回值
///
/// 返回 frpc 各版本的下载源信息
pub async fn get_download() -> CoreResult<OpenFrpDownloadObj> {
    let client = crate::get_work_client();

    client.get_json(urls::OPENFRP_DOWNLOAD).await
}

/// 创建 Frp 下载项目
///
/// # 返回值
///
/// 返回与当前系统架构匹配的 frpc 下载任务；不支持的系统返回 `InvalidOperation`
pub async fn build_download_item() -> CoreResult<FileItemObj> {
    let data = get_download().await?;

    let sys = mml_sys::get_system_info();
    let name = if sys.os == Os::Windows {
        if sys.is_arm {
            "frpc_windows_arm64.zip"
        } else {
            "frpc_windows_amd64.zip"
        }
    } else if sys.os == Os::Linux {
        if sys.is_arm {
            "frpc_linux_arm64.tar.gz"
        } else {
            "frpc_linux_amd64.tar.gz"
        }
    } else if sys.os == Os::MacOS {
        if sys.is_arm {
            "frpc_darwin_arm64.tar.gz"
        } else {
            "frpc_darwin_amd64.tar.gz"
        }
    } else {
        ""
    };

    if name.is_empty() {
        return Err(ErrorType::InvalidOperation);
    }

    Ok(FileItemObj {
        name: format!("OpenFrp {}", name),
        file: Default::default(),
        url: format!("{}{}{}", data.data.source[0].value, data.data.latest, name),
        hash: FileHash::None,
        later: LaterRun::None,
    })
}
