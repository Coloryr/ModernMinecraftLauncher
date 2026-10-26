//! 云同步界面契约，持久化格式由 integrations 管理。

use serde::{Deserialize, Serialize};

/// 一个实例的云连接配置。
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudConfigDto {
    /// 云服务器地址。
    pub server: String,
    /// 服务器密钥。
    pub server_key: String,
    /// 当前客户端密钥。
    pub client_key: String,
    /// 云端实例 UUID，跨电脑使用相同值。
    pub remote_uuid: String,
}

/// 云端存档条目。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudWorldDto {
    /// 存档目录名。
    pub name: String,
    /// 云端 UTC 修改时间。
    pub time: String,
    /// 云端文件数量。
    pub files: u32,
}

/// 云同步面板当前状态。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudStatusDto {
    /// 实例是否开放同步。
    pub enabled: bool,
    /// 当前客户端已用空间，单位字节。
    pub used: u64,
    /// 当前客户端配额，单位字节。
    pub quota: u64,
    /// 云端存档，停用时为空。
    pub worlds: Vec<CloudWorldDto>,
}
