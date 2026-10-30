//! OpenFrp 联机平台 API
//!
//! 接口口径见 <https://github.com/ZGIT-Network/OPENFRP-APIDOC>：
//! 隧道列表可用账户密钥查询，用户信息与隧道详情要登录后的 `Authorization` 头。

use mml_base::file_item::{FileHash, FileItemObj, LaterRun};
use mml_names::i18_items::error_type::{CoreResult, ErrorType};
use mml_sys::Os;
use serde::{Deserialize, Serialize};

use crate::urls;

/// 通道列表，对应 `?action=getallproxies` 的响应体
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct OpenFrpChannelObj {
    /// HTTP 状态码，对应 `status`
    pub status: i32,
    /// 请求是否成功，对应 `success`
    pub success: bool,
    /// 服务器消息，对应 `message`
    pub message: String,
    /// 节点分组列表，对应 `data`
    pub data: Vec<OpenFrpChannelData>,
}

/// 通道分组，对应 `data[]` 的元素
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct OpenFrpChannelData {
    /// 节点名称，对应 `node`；旧接口里这个键叫 `name`
    #[serde(rename = "node", alias = "name")]
    pub name: String,
    /// 组内隧道列表，对应 `proxies`
    pub proxies: Vec<ProxieObj>,
}

/// 单个隧道，对应 `proxies[]` 的元素
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct ProxieObj {
    /// 隧道名称，对应 `name`
    pub name: String,
    /// 隧道 ID，对应 `id`，简易启动时作为 `-p` 参数
    pub id: i64,
    /// 隧道协议，对应 `type`
    #[serde(rename = "type")]
    pub p_type: String,
    /// 远程访问地址，对应 `remote`
    pub remote: String,
    /// 本地地址，对应 `local`
    pub local: String,
}

/// 用户信息，对应 `getUserInfo` 的响应体
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct OpenFrpUserInfoObj {
    /// 用户数据，对应 `data`
    pub data: OpenFrpUserInfoData,
    /// 请求是否成功，对应 `flag`
    pub flag: bool,
    /// 服务器消息，对应 `msg`
    pub msg: String,
}

/// 用户数据，对应 `getUserInfo` 的 `data`
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct OpenFrpUserInfoData {
    /// 上行带宽，单位 Kbps
    pub out_limit: i64,
    /// 已用隧道数
    pub used: i64,
    /// 用户密钥，旧接口 `getallproxies` 的 `user` 参数
    pub token: String,
    /// 是否已完成实名认证
    pub realname: bool,
    /// 注册时间
    pub reg_time: String,
    /// 下行带宽，单位 Kbps
    pub in_limit: i64,
    /// 用户组显示名
    pub friendly_group: String,
    /// 隧道总数
    pub proxies: i64,
    /// 用户注册 ID
    pub id: i64,
    /// 注册邮箱
    pub email: String,
    /// 用户名
    pub username: String,
    /// 用户组标识
    pub group: String,
    /// 剩余流量，单位 MiB
    pub traffic: i64,
}

/// 用户隧道列表，对应 `getUserProxies` 的响应体
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct OpenFrpUserProxyListObj {
    /// 隧道数据，对应 `data`
    pub data: OpenFrpUserProxyListData,
    /// 请求是否成功，对应 `flag`
    pub flag: bool,
    /// 服务器消息，对应 `msg`
    pub msg: String,
}

/// 隧道数据，对应 `getUserProxies` 的 `data`
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct OpenFrpUserProxyListData {
    /// 隧道总数
    pub total: i64,
    /// 隧道列表
    pub list: Vec<OpenFrpUserProxyObj>,
}

/// 单条隧道详情，对应 `getUserProxies` 的 `data.list[]`
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct OpenFrpUserProxyObj {
    /// 自动 TLS 配置，可能是 `"true"` / `"false"` / 证书名；旧隧道没有这个键
    pub auto_tls: String,
    /// 远程连接地址，形如 `域名:端口`
    pub connect_address: String,
    /// 自定义 frpc 配置
    pub custom: String,
    /// 绑定的域名，仅 HTTP / HTTPS 隧道有
    pub domain: String,
    /// 是否强制使用 HTTPS
    pub force_https: bool,
    /// 节点显示名
    pub friendly_node: String,
    /// 隧道 ID
    pub id: i64,
    /// 最后一次启动时间，Unix 时间戳；从未启动为 `None`
    pub last_login: Option<i64>,
    /// 最后一次修改时间，Unix 时间戳
    pub last_update: i64,
    /// 本地地址
    pub local_ip: String,
    /// 本地端口
    pub local_port: i64,
    /// 节点 ID
    pub nid: i64,
    /// 是否在线
    pub online: bool,
    /// 隧道名称
    pub proxy_name: String,
    /// 是否启用 proxy 协议
    pub proxy_protocol_version: bool,
    /// 隧道协议，如 `tcp` / `udp` / `http` / `https`
    pub proxy_type: String,
    /// 远程端口，HTTP / HTTPS 隧道没有
    pub remote_port: Option<i64>,
    /// 隧道是否启用
    pub status: bool,
    /// 隧道拥有者的注册 ID
    pub uid: i64,
    /// 是否启用数据压缩
    pub use_compression: bool,
    /// 是否启用数据加密
    pub use_encryption: bool,
}

/// frpc 客户端下载信息，对应 `commonQuery/get?key=software` 的响应体
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct OpenFrpDownloadObj {
    /// 下载配置，对应 `data`
    pub data: OpenFrpDownloadItemObj,
    /// 请求是否成功，对应 `flag`
    pub flag: bool,
    /// 服务器消息，对应 `msg`
    pub msg: String,
}

/// frpc 下载配置，对应响应体的 `data`
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct OpenFrpDownloadItemObj {
    /// 最新版本的目录名，对应 `latest`，形如 `/OF_0.61.1_xxx/`
    pub latest: String,
    /// 最新完整版本号，对应 `latest_full`
    pub latest_full: String,
    /// 下载源列表，对应 `source`，取第一个即可
    pub source: Vec<SourceObj>,
}

/// 下载源，对应 `source[]` 的元素
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct SourceObj {
    /// 源地址前缀，对应 `value`，与版本目录、文件名拼成下载地址
    pub value: String,
}

/// 获取通道列表
///
/// 这是只用账户密钥的旧接口，响应按节点分组。
///
/// - `key`: 账户密钥，即用户信息里的 `token`
///
/// # 返回值
///
/// 返回账户下所有节点分组
pub async fn get_channel(key: &str) -> CoreResult<OpenFrpChannelObj> {
    let client = crate::get_work_client();
    let url = format!("{}/api?action=getallproxies&user={key}", urls::OPENFRP);

    client.get_json(&url).await
}

/// 获取用户信息
///
/// - `authorization`: 登录后拿到的 `Authorization`
///
/// # 返回值
///
/// 返回账户信息，其中 `data.token` 可用于 `get_channel`
pub async fn get_user_info(authorization: &str) -> CoreResult<OpenFrpUserInfoObj> {
    let client = crate::get_work_client();
    let url = format!("{}/frp/api/getUserInfo", urls::OPENFRP);

    client.post_json_auth(&url, authorization).await
}

/// 获取用户隧道列表
///
/// - `authorization`: 登录后拿到的 `Authorization`
///
/// # 返回值
///
/// 返回账户下所有隧道的详情
pub async fn get_user_proxies(authorization: &str) -> CoreResult<OpenFrpUserProxyListObj> {
    let client = crate::get_work_client();
    let url = format!("{}/frp/api/getUserProxies", urls::OPENFRP);

    client.post_json_auth(&url, authorization).await
}

/// 获取通道配置
///
/// 新接口没有单独的通道配置端点，这里从 `get_user_proxies` 的结果里按 ID 取。
///
/// - `authorization`: 登录后拿到的 `Authorization`
/// - `id`: 隧道 ID，取隧道列表里的 `id`
///
/// # 返回值
///
/// 找到时返回该隧道的详情；找不到返回 `InvalidOperation`
pub async fn get_channel_config(authorization: &str, id: i64) -> CoreResult<OpenFrpUserProxyObj> {
    let list = get_user_proxies(authorization).await?;

    list.data
        .list
        .into_iter()
        .find(|proxy| proxy.id == id)
        .ok_or(ErrorType::InvalidOperation)
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

/// 拼 frpc 下载地址
///
/// - `source`: 下载源前缀，可能带结尾斜杠
/// - `latest`: 版本目录，可能带首尾斜杠
/// - `latest_full`: 版本目录的备用值，`latest` 为空时用它
/// - `file`: 文件名
///
/// # 返回值
///
/// 三段用单个斜杠拼起来的地址
fn download_url(source: &str, latest: &str, latest_full: &str, file: &str) -> String {
    let dir = if latest.trim_matches('/').is_empty() {
        latest_full
    } else {
        latest
    };

    format!(
        "{}/{}/{}",
        source.trim_end_matches('/'),
        dir.trim_matches('/'),
        file
    )
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

    let source = data
        .data
        .source
        .first()
        .ok_or(ErrorType::InvalidOperation)?;

    Ok(FileItemObj {
        name: format!("OpenFrp {name}"),
        file: Default::default(),
        url: download_url(
            &source.value,
            &data.data.latest,
            &data.data.latest_full,
            name,
        ),
        hash: FileHash::None,
        later: LaterRun::None,
    })
}

#[cfg(test)]
mod tests {
    use super::download_url;

    /// 下载源与版本目录带不带斜杠都要拼出单斜杠
    #[test]
    fn download_url_joins_slashes() {
        assert_eq!(
            download_url(
                "https://staticassets.naids.com/client",
                "/OF_0.61.1_xxx/",
                "OF_0.61.1_xxx",
                "frpc_windows_amd64.zip"
            ),
            "https://staticassets.naids.com/client/OF_0.61.1_xxx/frpc_windows_amd64.zip"
        );
        assert_eq!(
            download_url(
                "https://r.zyghit.cn/download/client/",
                "OF_0.61.1_xxx",
                "OF_0.61.1_xxx",
                "frpc_linux_amd64.tar.gz"
            ),
            "https://r.zyghit.cn/download/client/OF_0.61.1_xxx/frpc_linux_amd64.tar.gz"
        );
        // latest 为空时退回 latest_full
        assert_eq!(
            download_url("https://x/client", "", "OF_1", "frpc_darwin_arm64.tar.gz"),
            "https://x/client/OF_1/frpc_darwin_arm64.tar.gz"
        );
    }
}
