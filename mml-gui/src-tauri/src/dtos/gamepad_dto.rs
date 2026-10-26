//! 手柄输入的跨 IPC 对象
//!
//! 按钮与轴都统一到 Xbox 布局，前端不需要认识底层手柄库的类型。

use serde::{Deserialize, Serialize};

/// 手柄按钮
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GamepadButtonDto {
    /// 下方面键，Xbox 的 A
    South,
    /// 右方面键，Xbox 的 B
    East,
    /// 上方面键，Xbox 的 Y
    North,
    /// 左方面键，Xbox 的 X
    West,
    /// C 键，部分手柄有
    C,
    /// Z 键，部分手柄有
    Z,
    /// 左扳机
    LeftTrigger,
    /// 左扳机第二段
    LeftTrigger2,
    /// 右扳机
    RightTrigger,
    /// 右扳机第二段
    RightTrigger2,
    /// Select
    Select,
    /// Start
    Start,
    /// 中间功能键
    Mode,
    /// 左摇杆按下
    LeftThumb,
    /// 右摇杆按下
    RightThumb,
    /// 十字键上
    DPadUp,
    /// 十字键下
    DPadDown,
    /// 十字键左
    DPadLeft,
    /// 十字键右
    DPadRight,
}

/// 手柄轴
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GamepadAxisDto {
    /// 左摇杆水平
    LeftStickX,
    /// 左摇杆垂直
    LeftStickY,
    /// 左扳机轴
    LeftZ,
    /// 右摇杆水平
    RightStickX,
    /// 右摇杆垂直
    RightStickY,
    /// 右扳机轴
    RightZ,
    /// 十字键水平
    DPadX,
    /// 十字键垂直
    DPadY,
}

/// 一个按钮的当前值
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamepadButtonValueDto {
    /// 按钮
    pub button: GamepadButtonDto,
    /// 按下程度，0.0 到 1.0；普通按钮只有 0 和 1
    pub value: f32,
}

/// 一个轴的当前值
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamepadAxisValueDto {
    /// 轴
    pub axis: GamepadAxisDto,
    /// 轴的值，-1.0 到 1.0
    pub value: f32,
}

/// 已连接的手柄
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamepadDto {
    /// 运行期间的编号，重连后可能变化
    pub id: u64,
    /// 系统给出的手柄名称
    pub name: String,
}

/// 手柄状态快照
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamepadStateDto {
    /// 运行期间的编号
    pub id: u64,
    /// 系统给出的手柄名称
    pub name: String,
    /// 全部按钮的当前值
    pub buttons: Vec<GamepadButtonValueDto>,
    /// 全部轴的当前值
    pub axes: Vec<GamepadAxisValueDto>,
}
/// 摇杆
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GamepadStickDto {
    /// 左摇杆
    Left,
    /// 右摇杆
    Right,
}

/// 一条按钮绑定
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamepadButtonRuleDto {
    /// 手柄按钮
    pub button: GamepadButtonDto,
    /// 修饰键：按住它时这条绑定才生效；null 表示不需要
    pub modifier: Option<GamepadButtonDto>,
    /// 动作，键名或 MouseLeft / ScrollUp 这类写法
    pub action: String,
}

/// 一条轴区间绑定
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamepadAxisRuleDto {
    /// 手柄轴
    pub axis: GamepadAxisDto,
    /// 区间起点，-1.0 到 1.0
    pub from: f32,
    /// 区间终点，-1.0 到 1.0
    pub to: f32,
    /// 动作
    pub action: String,
}

/// 摇杆当鼠标用的参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamepadMouseDto {
    /// 光标模式用的摇杆
    pub cursor_stick: GamepadStickDto,
    /// 转动视角模式用的摇杆
    pub turn_stick: GamepadStickDto,
    /// 光标模式速度，单位像素每秒
    pub cursor_speed: f32,
    /// 转动视角速度，单位像素每秒
    pub turn_speed: f32,
    /// 摇杆死区，绝对值小于该值按 0 处理
    pub deadzone: f32,
}

/// 一份手柄映射，配置窗口读写的就是它
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamepadConfigDto {
    /// 是否当前生效
    pub selected: bool,
    /// 按钮绑定
    pub buttons: Vec<GamepadButtonRuleDto>,
    /// 轴区间绑定
    pub axes: Vec<GamepadAxisRuleDto>,
    /// 摇杆当鼠标用
    pub mouse: GamepadMouseDto,
}

/// 配置目录里的一份映射
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamepadProfileDto {
    /// 配置名，即文件名去掉 .json
    pub name: String,
    /// 是否当前生效
    pub active: bool,
    /// 读取或校验失败的原因；正常为 None
    pub error: Option<String>,
}
