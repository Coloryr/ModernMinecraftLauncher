//! 云同步与手柄界面契约，持久化格式由 integrations 管理。

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

/// 手柄映射设置。
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamepadSettingsDto {
    /// 是否发送游戏输入。
    pub enabled: bool,
    /// 接收输入的实例 UUID，未选择为空。
    pub target: Option<String>,
    /// 设备 GUID，空串自动选第一只。
    pub device: String,
    /// 径向死区，0.05 至 0.8。
    pub deadzone: f64,
    /// 视角速度，单位像素每秒。
    pub look_speed: f64,
    /// 菜单光标速度，单位像素每秒。
    pub cursor_speed: f64,
}

impl From<mml_gamepad::Settings> for GamepadSettingsDto {
    fn from(value: mml_gamepad::Settings) -> Self {
        Self {
            enabled: value.enabled,
            target: value.target.map(|id| id.to_string()),
            device: value.device,
            deadzone: value.deadzone,
            look_speed: value.look_speed,
            cursor_speed: value.cursor_speed,
        }
    }
}

/// 已连接的手柄。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamepadDeviceDto {
    /// 设备 GUID。
    pub id: String,
    /// 驱动报告的设备名称。
    pub name: String,
}

/// 手柄实时测试数据。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamepadStatusDto {
    /// 已连接设备。
    pub devices: Vec<GamepadDeviceDto>,
    /// 左横、左纵、右横、右纵，正 Y 向上。
    pub axes: Vec<f64>,
    /// 标准按钮标识。
    pub buttons: Vec<String>,
    /// 是否正在向目标游戏发输入。
    pub active: bool,
    /// 驱动初始化错误键。
    pub error: Option<String>,
}
