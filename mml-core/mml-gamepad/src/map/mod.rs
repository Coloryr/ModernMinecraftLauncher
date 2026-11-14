//! 手柄映射：默认映射、配置目录与规则编译
//!
//! 映射配置放在运行目录的 gamepad 下，一份映射一个 json，字段用大驼峰；
//! Selected 为 true 的那份生效，没有就取文件名排序第一份。动作写字符串：
//! 键盘键名（W / Space / F5）、MouseLeft / MouseRight / MouseMiddle / Mouse4 到 Mouse8、
//! 多个用 `+` 连起来表示按顺序输出（LeftAlt+E），松开时反序；
//! ScrollUp / ScrollDown。目录里没有可用配置时用内置默认。
//!
//! 摇杆不写进按钮 / 轴表：光标模式与游戏内转动模式各用一个摇杆，
//! 由游戏上报的鼠标抓取状态切换，见 crate::relay。

use std::{
    path::PathBuf,
    sync::{LazyLock, OnceLock, RwLock},
};

use serde::{Deserialize, Serialize};

use crate::reader::{axis_index, button_index};

mod defaults;
mod profiles;
mod runtime;
mod store;

/// 配置目录路径，加载时设置
static DIR: OnceLock<PathBuf> = OnceLock::new();

/// 当前生效的映射
static MAP: LazyLock<RwLock<CompiledMap>> = LazyLock::new(|| RwLock::new(CompiledMap::built_in()));

/// 上次扫描到的文件列表与修改时间
static SIGNATURE: LazyLock<RwLock<String>> = LazyLock::new(|| RwLock::new(String::new()));

/// 手柄按钮，统一到 Xbox 布局
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Button {
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
pub enum Axis {
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

/// 手柄外观：只影响配置界面上的按钮记号与状态图，不参与映射
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PadLayout {
    /// Xbox 布局：A / B / X / Y，LB / RB，LT / RT
    Xbox,
    /// PlayStation 布局：✕ / ○ / □ / △，L1 / R1，L2 / R2
    Playstation,
}

impl Default for PadLayout {
    fn default() -> Self {
        Self::Xbox
    }
}

/// 一份映射配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct GamepadConfig {
    /// 是否当前生效的配置；多份为 true 时取文件名排序第一份
    pub selected: bool,
    /// 是否启用游戏内手柄：关掉时转发线程不下发任何输入
    #[serde(default)]
    pub enabled: bool,
    /// 选中的手柄名；`None` 表示用第一个已连接的手柄
    #[serde(default)]
    pub device: Option<String>,
    /// 手柄外观，只影响配置界面的记号
    pub layout: PadLayout,
    /// 是否交换 A / B 两个位置，按 Nintendo 布局的习惯显示
    pub swap_ab: bool,
    /// 鼠标模式是否单独用一套按钮绑定；关着时两种模式共用 buttons
    pub separate_cursor: bool,
    /// 按钮绑定：游戏内模式，鼠标被游戏抓取时生效
    pub buttons: Vec<ButtonRule>,
    /// 按钮绑定：鼠标模式，未抓取鼠标时生效；缺省表示与 buttons 相同
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor_buttons: Option<Vec<ButtonRule>>,
    /// 轴区间绑定
    pub axes: Vec<AxisRule>,
    /// 摇杆当鼠标用
    pub mouse: MouseRule,
}

/// 按钮的触发方式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ButtonMode {
    /// 普通：按下就按下，抬起就抬起
    #[default]
    Normal,
    /// 连续：按一次保持按下，再按一次松开
    Toggle,
    /// 循环按下：按住期间反复触发
    Repeat,
}

/// 一个按钮的绑定
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ButtonRule {
    /// 手柄按钮
    pub button: Button,
    /// 修饰键：按住它时这条绑定才生效；不填表示不需要
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modifier: Option<Button>,
    /// 动作
    pub action: String,
    /// 触发方式；旧配置里没有这一项时按普通处理
    #[serde(default)]
    pub mode: ButtonMode,
}

/// 一个轴区间的绑定
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AxisRule {
    /// 手柄轴
    pub axis: Axis,
    /// 区间起点，-1.0 到 1.0
    pub from: f32,
    /// 区间终点，-1.0 到 1.0
    pub to: f32,
    /// 动作
    pub action: String,
}

/// 摇杆当鼠标用的参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MouseRule {
    /// 光标模式用的摇杆
    pub cursor_stick: Stick,
    /// 转动视角模式用的摇杆
    pub turn_stick: Stick,
    /// 光标模式速度，单位像素每秒
    pub cursor_speed: f32,
    /// 转动视角速度，单位像素每秒
    pub turn_speed: f32,
    /// 摇杆死区，绝对值小于该值按 0 处理
    pub deadzone: f32,
}

/// 摇杆
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Stick {
    /// 左摇杆
    Left,
    /// 右摇杆
    Right,
}

/// 配置目录里的一份映射
#[derive(Debug, Clone)]
pub struct ProfileInfo {
    /// 配置名，即文件名去掉 .json
    pub name: String,
    /// 是否当前生效
    pub active: bool,
    /// 读取或校验失败的原因；正常为 None
    pub error: Option<String>,
}

/// 配置操作的错误
#[derive(Debug, Clone)]
pub enum GamepadError {
    /// 配置名不合法
    InvalidName,
    /// 同名配置已存在
    Exists,
    /// 找不到配置
    Missing,
    /// 至少要保留一份配置
    Last,
    /// 映射内容不合法
    InvalidConfig,
    /// 读写文件失败，带原始说明
    Io(String),
}

/// 编译后的映射表，转发线程按状态数组下标直接查
#[derive(Debug, Clone)]
pub struct CompiledMap {
    /// 是否启用游戏内手柄
    pub enabled: bool,
    /// 选中的手柄名；`None` 表示用第一个已连接的手柄
    pub device: Option<String>,
    /// 按钮规则：游戏内模式
    pub buttons: Vec<ButtonRuleCompiled>,
    /// 是否用鼠标模式那套按钮
    pub separate_cursor: bool,
    /// 按钮规则：鼠标模式；关掉分离时为空
    pub cursor_buttons: Vec<ButtonRuleCompiled>,
    /// 轴规则
    pub axes: Vec<AxisRuleCompiled>,
    /// 摇杆当鼠标用
    pub mouse: MouseRuleCompiled,
}

/// 编译后的按钮规则
#[derive(Debug, Clone)]
pub struct ButtonRuleCompiled {
    /// 规则编号，按钮与轴共用一套，转发线程用它记哪些规则正按住
    pub id: usize,
    /// 按钮在状态数组中的下标
    pub index: usize,
    /// 修饰键在状态数组中的下标；None 表示不需要
    pub modifier: Option<usize>,
    /// 触发方式
    pub mode: ButtonMode,
    /// 要输出的动作序列
    pub combo: Combo,
}

/// 编译后的轴规则
#[derive(Debug, Clone)]
pub struct AxisRuleCompiled {
    /// 规则编号，与按钮规则同一套编号
    pub id: usize,
    /// 轴在状态数组中的下标
    pub index: usize,
    /// 区间起点
    pub from: f32,
    /// 区间终点
    pub to: f32,
    /// 要输出的动作序列
    pub combo: Combo,
    /// 轴所属的摇杆；十字键这类不归属任何摇杆时为 None
    pub stick: Option<Stick>,
}

/// 摇杆当鼠标用的参数
#[derive(Debug, Clone, Copy)]
pub struct MouseRuleCompiled {
    /// 光标模式
    pub cursor: StickRule,
    /// 转动视角模式
    pub turn: StickRule,
    /// 死区
    pub deadzone: f32,
}

/// 一个摇杆的两个轴与速度
#[derive(Debug, Clone, Copy)]
pub struct StickRule {
    /// 摇杆对应的轴
    pub axes: StickAxes,
    /// 速度，单位像素每秒
    pub speed: f32,
    /// 是哪个摇杆
    pub stick: Stick,
}

/// 一个摇杆两个轴在状态数组中的下标
#[derive(Debug, Clone, Copy)]
pub struct StickAxes {
    /// 水平轴
    pub x: usize,
    /// 垂直轴
    pub y: usize,
}

/// 手柄动作，编译后直接对应一条 IPC 消息
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    /// 键盘按键，GLFW 键码
    Key(i32),
    /// 鼠标按键，GLFW 编号
    Mouse(i32),
    /// 滚轮，格数正数向上
    Scroll(i32),
}

/// 一条绑定要输出的动作序列
///
/// 配置里写成 `LeftAlt+E` 这样的形式，按下按左到右的顺序发，松开按反序发，
/// 保证"先按住修饰键、再按主键、最后按反序松开"。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Combo {
    /// 按下顺序
    pub steps: Vec<Action>,
}

impl Combo {
    /// 按下顺序
    pub fn press(&self) -> impl Iterator<Item = Action> + '_ {
        self.steps.iter().copied()
    }

    /// 松开顺序：按下顺序反过来
    pub fn release(&self) -> impl Iterator<Item = Action> + '_ {
        self.steps.iter().rev().copied()
    }
}

impl Default for GamepadConfig {
    fn default() -> Self {
        Self {
            // 缺 Selected 的配置不参与生效，默认那份在写文件时显式打开
            selected: false,
            // 游戏内手柄默认关：要配好映射、装好 coloryrasm 再打开
            enabled: false,
            // 默认不挑设备，用第一个连上的
            device: None,
            layout: PadLayout::default(),
            swap_ab: false,
            // 默认分离：鼠标模式下大多是点击 / 翻背包，与游戏内按键不是一套
            separate_cursor: true,
            cursor_buttons: Some(vec![
                repeat_rule(Button::DPadUp, "ScrollUp"),
                repeat_rule(Button::DPadDown, "ScrollDown"),
                button_rule(Button::South, "MouseLeft"),
                button_rule(Button::East, "Escape"),
                button_rule(Button::LeftTrigger2, "LeftShift"),
            ]),
            // 面键走鼠标：A 左键、B 右键，Y 跳、X 潜行
            buttons: vec![
                button_rule(Button::South, "MouseLeft"),
                button_rule(Button::East, "MouseRight"),
                button_rule(Button::North, "Space"),
                button_rule(Button::West, "LeftShift"),
                button_rule(Button::LeftTrigger, "MouseLeft"),
                button_rule(Button::RightTrigger, "MouseRight"),
                button_rule(Button::LeftTrigger2, "ScrollUp"),
                button_rule(Button::RightTrigger2, "ScrollDown"),
                button_rule(Button::Select, "Escape"),
                button_rule(Button::Start, "E"),
                button_rule(Button::DPadUp, "Q"),
                button_rule(Button::DPadDown, "F3"),
                button_rule(Button::DPadRight, "F"),
            ],
            axes: vec![
                // 读取层已统一为 Y 轴向上为正
                axis_rule(Axis::LeftStickY, 0.35, 1.0, "W"),
                axis_rule(Axis::LeftStickY, -1.0, -0.35, "S"),
                axis_rule(Axis::LeftStickX, -1.0, -0.35, "A"),
                axis_rule(Axis::LeftStickX, 0.35, 1.0, "D"),
            ],
            mouse: MouseRule {
                cursor_stick: Stick::Left,
                turn_stick: Stick::Right,
                cursor_speed: 1200.0,
                turn_speed: 1500.0,
                deadzone: 0.15,
            },
        }
    }
}

impl GamepadConfig {
    /// 鼠标模式实际用的按钮绑定：没单独配过时就是游戏内那套
    pub fn cursor_rules(&self) -> &[ButtonRule] {
        match self.cursor_buttons.as_deref() {
            Some(rules) => rules,
            None => &self.buttons,
        }
    }

    /// 补上鼠标模式那套按钮绑定：旧配置里没有这一项时先与游戏内模式一致
    pub fn normalize(&mut self) {
        if self.cursor_buttons.is_none() {
            self.cursor_buttons = Some(self.buttons.clone());
        }
    }

    /// 校验并编译成转发线程用的规则表
    pub fn compile(&self) -> Result<CompiledMap, String> {
        let buttons = compile_buttons(&self.buttons, 0)?;

        let mut axes = Vec::with_capacity(self.axes.len());
        for rule in &self.axes {
            let index =
                axis_index(rule.axis).ok_or_else(|| format!("未知手柄轴：{:?}", rule.axis))?;
            if !(-1.0..=1.0).contains(&rule.from) || !(-1.0..=1.0).contains(&rule.to) {
                return Err(format!(
                    "轴区间超出 -1.0 到 1.0：{} 到 {}",
                    rule.from, rule.to
                ));
            }
            if rule.from == rule.to {
                return Err(format!("轴区间起点与终点相同：{}", rule.from));
            }
            axes.push(AxisRuleCompiled {
                // 编号接着按钮规则往下排，两条列表合起来是唯一的
                id: buttons.len() + axes.len(),
                index,
                from: rule.from,
                to: rule.to,
                combo: parse_combo(&rule.action)?,
                stick: stick_of_axis(rule.axis),
            });
        }

        let cursor_buttons = if self.separate_cursor {
            compile_buttons(self.cursor_rules(), buttons.len() + axes.len())?
        } else {
            Vec::new()
        };

        let mouse = &self.mouse;
        if mouse.cursor_speed < 0.0 || mouse.turn_speed < 0.0 {
            return Err(String::from("摇杆速度不能为负"));
        }
        if !(0.0..1.0).contains(&mouse.deadzone) {
            return Err(String::from("死区应在 0.0 到 1.0 之间"));
        }

        Ok(CompiledMap {
            enabled: self.enabled,
            device: self.device.clone(),
            buttons,
            separate_cursor: self.separate_cursor,
            cursor_buttons,
            axes,
            mouse: MouseRuleCompiled {
                cursor: stick_rule(mouse.cursor_stick, mouse.cursor_speed),
                turn: stick_rule(mouse.turn_stick, mouse.turn_speed),
                deadzone: mouse.deadzone,
            },
        })
    }
}

/// 编译一组按钮规则，编号从 `base` 起接着排
fn compile_buttons(rules: &[ButtonRule], base: usize) -> Result<Vec<ButtonRuleCompiled>, String> {
    let mut out = Vec::with_capacity(rules.len());
    for rule in rules {
        let index = button_index(rule.button)
            .ok_or_else(|| format!("未知手柄按钮：{:?}", rule.button))?;
        let modifier = match rule.modifier {
            Some(modifier) => {
                let modifier = button_index(modifier)
                    .ok_or_else(|| format!("未知修饰键：{:?}", modifier))?;
                if modifier == index {
                    return Err(String::from("修饰键不能和按钮相同"));
                }
                Some(modifier)
            }
            None => None,
        };
        out.push(ButtonRuleCompiled {
            id: base + out.len(),
            index,
            modifier,
            mode: rule.mode,
            combo: parse_combo(&rule.action)?,
        });
    }
    Ok(out)
}

impl CompiledMap {
    /// 这一帧该用哪套按钮绑定：抓到鼠标时用游戏内那套，否则用鼠标模式那套
    pub fn buttons_for(&self, grabbed: bool) -> &[ButtonRuleCompiled] {
        if grabbed || !self.separate_cursor {
            &self.buttons
        } else {
            &self.cursor_buttons
        }
    }

    /// 内置默认映射
    fn built_in() -> Self {
        GamepadConfig::default()
            .compile()
            .expect("内置手柄映射应能编译")
    }
}

/// 一份可用的配置
struct Profile {
    /// 文件路径
    path: PathBuf,
    /// 是否标记为生效
    selected: bool,
    /// 编译后的映射
    map: CompiledMap,
}

/// 一份配置加载失败的说明
struct ProfileError {
    /// 文件路径
    path: PathBuf,
    /// 失败原因
    error: String,
}

/// 扫描配置目录的结果
struct Scan {
    /// 可用的配置，按文件名排序
    profiles: Vec<Profile>,
    /// 加载失败的条目
    errors: Vec<ProfileError>,
}

pub(crate) use self::defaults::*;
pub(crate) use self::runtime::*;
pub(crate) use self::store::*;

pub(crate) use self::defaults::combo_of;
pub use self::profiles::{action_names, config, create, delete, profiles, save, set_active};
pub use self::runtime::{current, generation, in_range, init, reload_if_changed};
