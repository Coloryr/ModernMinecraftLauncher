//! 联机大厅视图 DTO，前端 wire 为 camelCase
//!
//! 不包含账户令牌，也不暴露陶瓦的本地控制端口。陶瓦只在本机回环地址上跑，
//! 这里给前端的是房间状态快照；状态序号与世代号随视图一起下发，
//! 前后端都据它们丢弃迟到的旧响应。

use serde::{Deserialize, Serialize};

/// 房间成员上报的账户资料
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbyProfileDto {
    /// 陶瓦侧的设备标识，成员列表按它做 key
    pub id: String,
    /// 玩家名
    pub name: String,
    /// 角色，`HOST` 为房主
    pub kind: String,
    /// 认证来源，非房主成员显示在角色位置
    pub vendor: String,
}

/// 陶瓦房间状态快照
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbyStateDto {
    /// 状态机 id：stopped / waiting / host-scanning / host-starting / host-ok /
    /// guest-connecting / guest-starting / guest-ok；前端按 id 翻文案
    pub state: String,
    /// 状态序号，陶瓦每次状态变化递增；后端用它丢弃迟到的旧轮询
    pub index: u64,
    /// 当前房间的邀请码；不在房间里为 None
    pub room: Option<String>,
    /// 直连地址；房主是本机转发端口，加入方是房主地址，没有房间时为 None
    pub address: Option<String>,
    /// 连接难度文本，由陶瓦给出，界面原样显示
    pub difficulty: Option<String>,
    /// 异常码，取值 0–5，与前端 `lobby.exception.*` 一一对应；没出错为 None
    pub exception: Option<u32>,
    /// 房间成员，房主在内
    pub profiles: Vec<LobbyProfileDto>,
}

/// 大厅页面的完整视图，即 prepare / status / start 三个命令的返回值
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbyViewDto {
    /// 已锁定的正版账户名，空串表示还没验证
    pub account: String,
    /// 已锁定的账户 uuid，空串表示大厅已关闭
    pub account_uuid: String,
    /// 陶瓦版本，取自它的 `/meta`；启动前为空串
    pub version: String,
    /// 陶瓦重建时递增，用于丢弃旧会话的迟到响应。
    pub session_generation: u32,
    /// 陶瓦已就绪，即工作进程在跑且读到了版本
    pub ready: bool,
    /// 正在下载或校验陶瓦程序
    pub downloading: bool,
    /// 陶瓦下载进度，单位百分比
    pub download_percent: u32,
    /// 公共节点列表，创建 / 加入房间时勾选
    pub nodes: Vec<String>,
    /// 房间状态快照
    pub state: LobbyStateDto,
}

/// 映射大厅里的一个房间
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MappingRoomDto {
    /// 房间标识
    pub id: String,
    /// 房间名
    pub name: String,
    /// 在线人数
    pub players: u32,
}

/// 映射大厅列表
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MappingLobbyDto {
    /// 后端服务是否已接入；尚未接入时为 false，前端显示对应提示
    pub available: bool,
    /// 可加入的房间；不可用或没有房间时为空数组
    pub rooms: Vec<MappingRoomDto>,
}
