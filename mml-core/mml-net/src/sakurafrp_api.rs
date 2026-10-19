//! SakuraFrp 樱花内网穿透 API
//!
//! 提供通道列表查询、frpc 配置生成与 frpc 客户端下载信息。

use mml_base::file_item::{FileHash, FileItemObj, LaterRun};
use mml_names::i18_items::error_type::{CoreResult, ErrorType};
use mml_sys::Os;
use serde::{Deserialize, Serialize};

use crate::urls;

/// 单个通道，对应 `tunnels` 接口返回的一项
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct SakuraFrpChannelObj {
    /// 通道 ID，对应响应键 `id`
    pub id: i32,
    /// 通道名称，对应响应键 `name`
    pub name: String,
    /// 通道协议类型，对应响应键 `type`，如 tcp / udp / http
    #[serde(rename = "type")]
    pub c_type: String,
    /// 远程访问地址，对应响应键 `remote`，形如 `frp.example.com:12345`
    pub remote: String,
}

/// 通道配置查询请求体，对应 `tunnel/config` 接口
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct SakuraFrpGetChannelObj {
    /// 查询的通道 ID，对应请求键 `query`
    pub query: i32,
}

/// frpc 客户端下载信息，对应 `system/clients` 接口
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct SakuraFrpDownloadObj {
    /// frpc 各架构下载项，对应响应键 `frpc`
    pub frpc: SakuraFrpDownloadItemObj,
}

/// frpc 各架构下载信息
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct SakuraFrpDownloadItemObj {
    /// 各平台架构的下载项，对应响应键 `archs`
    pub archs: ArchsObj,
    /// frpc 版本号，对应响应键 `ver`
    pub ver: String,
}

/// 各平台架构的下载项集合，键名即平台与架构
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct ArchsObj {
    /// Windows amd64 的 frpc 下载项
    pub windows_amd64: ArchItemObj,
    /// Windows arm64 的 frpc 下载项
    pub windows_arm64: ArchItemObj,
    /// Linux amd64 的 frpc 下载项
    pub linux_amd64: ArchItemObj,
    /// Linux arm64 的 frpc 下载项
    pub linux_arm64: ArchItemObj,
    /// macOS amd64 的 frpc 下载项
    pub darwin_amd64: ArchItemObj,
    /// macOS arm64 的 frpc 下载项
    pub darwin_arm64: ArchItemObj,
}

/// 单个架构的下载项
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct ArchItemObj {
    /// 架构标题，用于拼下载任务名
    pub title: String,
    /// frpc 压缩包下载 URL
    pub url: String,
    /// 文件哈希，SHA1 十六进制
    pub hash: String,
}

/// 获取通道列表
///
/// - `key`: 账户密钥
///
/// # 返回值
///
/// 返回账户下所有通道
pub async fn get_channel(key: &str) -> CoreResult<Vec<SakuraFrpChannelObj>> {
    let client = crate::get_work_client();
    let url = format!("{}tunnels?token={key}", urls::SAKURA_FRP);

    client.get_json(&url).await
}

/// 获取通道配置
///
/// - `key`: 账户密钥
/// - `id`: 通道 ID
/// - `version`: frpc 版本号
///
/// # 返回值
///
/// 返回该通道的 frpc 配置文本
pub async fn get_channel_config(key: &str, id: i32, version: &str) -> CoreResult<String> {
    let client = crate::get_work_client();
    let url = format!(
        "{}tunnel/config?token={key}&frpc={version}",
        urls::SAKURA_FRP
    );

    client
        .post_json_get_text(&url, &SakuraFrpGetChannelObj { query: id })
        .await
}

/// 获取下载列表
///
/// # 返回值
///
/// 返回 frpc 各架构的下载信息
pub async fn get_download() -> CoreResult<SakuraFrpDownloadObj> {
    let client = crate::get_work_client();
    let url = format!("{}system/clients", urls::SAKURA_FRP);

    client.get_json(&url).await
}

/// 创建 frpc 下载任务
///
/// # 返回值
///
/// 返回与当前系统架构匹配的 frpc 下载任务；不支持的系统返回 `InvalidOperation`
pub async fn build_download_item() -> CoreResult<FileItemObj> {
    let obj = get_download().await?;

    let sys = mml_sys::get_system_info();
    let obj = if sys.os == Os::Windows {
        if sys.is_arm {
            obj.frpc.archs.windows_arm64
        } else {
            obj.frpc.archs.windows_amd64
        }
    } else if sys.os == Os::Linux {
        if sys.is_arm {
            obj.frpc.archs.linux_arm64
        } else {
            obj.frpc.archs.linux_amd64
        }
    } else if sys.os == Os::MacOS {
        if sys.is_arm {
            obj.frpc.archs.darwin_arm64
        } else {
            obj.frpc.archs.darwin_amd64
        }
    } else {
        return Err(ErrorType::InvalidOperation);
    };

    Ok(FileItemObj {
        name: format!("SakuraFrp {}", obj.title),
        file: Default::default(),
        url: obj.url,
        hash: FileHash::Sha1(obj.hash),
        later: LaterRun::None,
    })
}
