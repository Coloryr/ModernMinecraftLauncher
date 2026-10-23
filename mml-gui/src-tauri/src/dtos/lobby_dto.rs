//! 联机大厅视图；不包含账户令牌或陶瓦本地控制端口。

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbyProfileDto {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub vendor: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbyStateDto {
    pub state: String,
    pub index: u64,
    pub room: Option<String>,
    pub address: Option<String>,
    pub difficulty: Option<String>,
    pub exception: Option<u32>,
    pub profiles: Vec<LobbyProfileDto>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbyViewDto {
    pub account: String,
    pub account_uuid: String,
    pub version: String,
    pub ready: bool,
    pub downloading: bool,
    pub download_percent: u32,
    pub nodes: Vec<String>,
    pub state: LobbyStateDto,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MappingRoomDto {
    pub id: String,
    pub name: String,
    pub players: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MappingLobbyDto {
    pub available: bool,
    pub rooms: Vec<MappingRoomDto>,
}
