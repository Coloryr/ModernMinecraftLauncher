//! 手柄输入只发送给选中的游戏 IPC 通道，不操作系统键鼠。

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

pub mod runtime;

/// 持久化手柄设置。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Settings {
    /// 是否向游戏发送手柄输入，缺省关闭。
    pub enabled: bool,
    /// 接收输入的游戏实例 UUID。
    pub target: Option<Uuid>,
    /// 设备 GUID，空串表示第一只已连接手柄。
    pub device: String,
    /// 摇杆径向死区，范围 0.05 至 0.8。
    pub deadzone: f64,
    /// 游戏视角移动速度，单位像素每秒。
    pub look_speed: f64,
    /// 菜单光标移动速度，单位像素每秒。
    pub cursor_speed: f64,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            target: None,
            device: String::new(),
            deadzone: 0.18,
            look_speed: 900.0,
            cursor_speed: 700.0,
        }
    }
}
impl Settings {
    /// 检查配置范围，拒绝非有限数值。
    ///
    /// # 返回值
    ///
    /// 全部配置合法时返回 true。
    pub fn valid(&self) -> bool {
        self.deadzone.is_finite()
            && (0.05..=0.8).contains(&self.deadzone)
            && self.look_speed.is_finite()
            && (50.0..=5000.0).contains(&self.look_speed)
            && self.cursor_speed.is_finite()
            && (50.0..=5000.0).contains(&self.cursor_speed)
            && (!self.enabled || self.target.is_some())
    }
}

/// 统一的标准手柄按钮。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Button {
    /// A / Cross。
    South,
    /// B / Circle。
    East,
    /// X / Square。
    West,
    /// Y / Triangle。
    North,
    /// 菜单按钮。
    Start,
    /// 返回按钮。
    Select,
    /// 左肩键。
    LeftShoulder,
    /// 右肩键。
    RightShoulder,
    /// 左扳机。
    LeftTrigger,
    /// 右扳机。
    RightTrigger,
    /// 左摇杆按下。
    LeftStick,
    /// 右摇杆按下。
    RightStick,
    /// 十字键上。
    Up,
    /// 十字键下。
    Down,
    /// 十字键左。
    Left,
    /// 十字键右。
    Right,
}

/// 一帧手柄状态，摇杆正 Y 指向上方。
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    /// 左摇杆横轴，范围 -1 至 1。
    pub left_x: f64,
    /// 左摇杆纵轴，范围 -1 至 1。
    pub left_y: f64,
    /// 右摇杆横轴，范围 -1 至 1。
    pub right_x: f64,
    /// 右摇杆纵轴，范围 -1 至 1。
    pub right_y: f64,
    /// 当前按住的按钮。
    pub buttons: BTreeSet<Button>,
}

/// 可持续按下的 GLFW 输入。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Action {
    /// GLFW 键盘编码。
    Key(i32),
    /// GLFW 鼠标按钮编码。
    Mouse(i32),
}

/// 映射后的游戏输入消息。
#[derive(Clone, Debug, PartialEq)]
pub enum Output {
    /// 一个按键状态变化。
    Button { action: Action, down: bool },
    /// 相对光标或视角移动。
    Move { dx: f64, dy: f64 },
    /// 正数向上滚动。
    Scroll(i32),
}
impl Output {
    /// 将映射结果交给游戏 IPC 通道。
    ///
    /// - `uuid`: 目标游戏 UUID
    ///
    /// # 返回值
    ///
    /// 通道不存在或已关闭时返回 false。
    pub fn send(&self, uuid: Uuid) -> bool {
        match *self {
            Self::Button {
                action: Action::Key(key),
                down,
            } => mml_ipc::send_key(uuid, key, down),
            Self::Button {
                action: Action::Mouse(button),
                down,
            } => mml_ipc::send_mouse_button(uuid, button, down),
            Self::Move { dx, dy } => mml_ipc::send_mouse_move(uuid, dx, dy),
            Self::Scroll(amount) => mml_ipc::send_mouse_scroll(uuid, amount),
        }
    }
}

/// 无设备依赖的映射状态机，切模式与断线时显式释放按键。
#[derive(Default)]
pub struct Mapper {
    /// 上一帧抓取模式，None 表示尚未连接目标。
    mode: Option<bool>,
    /// 已发出的持续按键。
    pressed: BTreeSet<Action>,
    /// 上一帧按钮，用于滚轮边沿检测。
    previous: BTreeSet<Button>,
    /// 模式切换时按着的按钮，松开后才能使用新映射。
    blocked: BTreeSet<Button>,
}

/// 径向死区处理结果。
struct Stick {
    /// 归一化横轴。
    x: f64,
    /// 归一化纵轴。
    y: f64,
}
fn stick(x: f64, y: f64, deadzone: f64) -> Stick {
    if !x.is_finite() || !y.is_finite() {
        return Stick { x: 0.0, y: 0.0 };
    }
    let length = x.hypot(y);
    if length <= deadzone {
        return Stick { x: 0.0, y: 0.0 };
    }
    let scale = ((length.min(1.0) - deadzone) / (1.0 - deadzone)) / length;
    Stick {
        x: x * scale,
        y: y * scale,
    }
}

impl Mapper {
    /// 释放所有已发出的按键并重置模式。
    ///
    /// # 返回值
    ///
    /// 必须向旧目标发送的释放消息。
    pub fn reset(&mut self) -> Vec<Output> {
        let released = self
            .pressed
            .iter()
            .map(|action| Output::Button {
                action: *action,
                down: false,
            })
            .collect();
        *self = Self::default();
        released
    }

    /// 将一帧手柄状态转换成增量输入。
    ///
    /// - `input`: 本帧设备状态
    /// - `grabbed`: 游戏是否抓取鼠标
    /// - `elapsed`: 距离上一帧的秒数，最多按 50 毫秒处理
    /// - `settings`: 已验证的死区和速度设置
    ///
    /// # 返回值
    ///
    /// 按键变化、移动和滚轮消息。
    pub fn update(
        &mut self,
        input: &Snapshot,
        grabbed: bool,
        elapsed: f64,
        settings: &Settings,
    ) -> Vec<Output> {
        let mut output = Vec::new();
        if !settings.valid() {
            return self.reset();
        }
        if self.mode != Some(grabbed) {
            output.extend(self.reset());
            self.mode = Some(grabbed);
            self.blocked = input.buttons.clone();
            self.previous = input.buttons.clone();
        }
        self.blocked.retain(|button| input.buttons.contains(button));
        let active =
            |button: Button| input.buttons.contains(&button) && !self.blocked.contains(&button);
        let mut desired = BTreeSet::new();
        let left = stick(input.left_x, input.left_y, settings.deadzone);
        if grabbed {
            for (on, key) in [
                (left.y > 0.35, 87),
                (left.y < -0.35, 83),
                (left.x < -0.35, 65),
                (left.x > 0.35, 68),
            ] {
                if on {
                    desired.insert(Action::Key(key));
                }
            }
            for (button, action) in [
                (Button::South, Action::Key(32)),
                (Button::East, Action::Key(340)),
                (Button::West, Action::Key(69)),
                (Button::North, Action::Key(70)),
                (Button::Select, Action::Key(81)),
                (Button::LeftStick, Action::Key(341)),
                (Button::RightStick, Action::Key(294)),
                (Button::RightTrigger, Action::Mouse(0)),
                (Button::LeftTrigger, Action::Mouse(1)),
            ] {
                if active(button) {
                    desired.insert(action);
                }
            }
        } else {
            for (button, action) in [
                (Button::South, Action::Mouse(0)),
                (Button::West, Action::Mouse(1)),
                (Button::East, Action::Key(256)),
            ] {
                if active(button) {
                    desired.insert(action);
                }
            }
        }
        if active(Button::Start) {
            desired.insert(Action::Key(256));
        }
        for action in self.pressed.difference(&desired) {
            output.push(Output::Button {
                action: *action,
                down: false,
            });
        }
        for action in desired.difference(&self.pressed) {
            output.push(Output::Button {
                action: *action,
                down: true,
            });
        }
        self.pressed = desired;
        for (button, amount) in [
            (Button::LeftShoulder, 1),
            (Button::RightShoulder, -1),
            (Button::Up, 1),
            (Button::Down, -1),
        ] {
            if active(button) && !self.previous.contains(&button) {
                output.push(Output::Scroll(amount));
            }
        }
        self.previous = input.buttons.clone();
        let elapsed = if elapsed.is_finite() {
            elapsed.clamp(0.0, 0.05)
        } else {
            0.0
        };
        let movement = if grabbed {
            stick(input.right_x, input.right_y, settings.deadzone)
        } else {
            left
        };
        let speed = if grabbed {
            settings.look_speed
        } else {
            settings.cursor_speed
        };
        let dx = movement.x * speed * elapsed;
        let dy = -movement.y * speed * elapsed;
        if dx != 0.0 || dy != 0.0 {
            output.push(Output::Move { dx, dy });
        }
        output
    }
}
