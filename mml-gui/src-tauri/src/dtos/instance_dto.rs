//! 游戏实例 DTO
use serde::{Deserialize, Serialize};

/// 游戏实例信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInfoDto {
    /// 实例 UUID
    pub uuid: String,
    /// 实例名
    pub name: String,
    /// 分组 uuid，`null` 表示默认分组
    pub group: Option<String>,
    /// 游戏版本号
    pub version: String,
    /// 游戏版本类型：release / snapshot / other
    pub version_type: Option<String>,
    /// 加载器类型
    pub loader: String,
    /// 加载器版本号
    pub loader_version: Option<String>,
    /// 实例目录名
    pub dir: String,
    /// 是否正在运行
    pub running: bool,
    /// 整合包平台：CurseForge / Modrinth / McMod / 本地压缩包 / 文件夹
    pub modpack_type: Option<String>,
    /// 整合包项目 ID
    pub pid: Option<String>,
    /// 整合包文件 ID
    pub fid: Option<String>,
    /// 在线网络整合包地址，即 ServerPack
    pub server_url: Option<String>,
    /// 游戏内语言
    pub lang: Option<String>,
    /// 日志编码：utf8 / gbk
    pub log_encoding: Option<String>,
    /// 来源：导入的压缩包 / 文件夹 / 在线网址
    pub source: Option<String>,
    /// 组内排列次序，来自分组表 `group_save.json` 的 `order`，升序
    pub order: i32,
}

/// 分组条目
///
/// 分组以 **uuid 为身份**、组名为显示数据，见 mml-game 的 `game_group`，
/// 所以跨 IPC 传的是这一对，而不是光一个组名。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupDto {
    /// 分组 uuid
    pub uuid: String,
    /// 分组显示名，默认分组是空白，由前端翻译成"默认分组"
    pub name: String,
}
