//! 手柄读取
//!
//! SDL 上下文、控制器句柄与事件泵始终留在读取线程，不初始化视频子系统。
//! 对外保持原来的按钮 / 轴顺序与 Y 轴向上为正的口径，供状态查询与转发线程使用。

use std::{
    collections::BTreeMap,
    sync::{LazyLock, Once, RwLock},
    thread,
    time::Duration,
};

use mml_names::i18_items::info_type::InfoType;
use sdl2::{
    GameControllerSubsystem,
    controller::{Axis as SdlAxis, Button as SdlButton, GameController},
    event::Event,
};

use crate::map::{Axis, Button, PadLayout};

/// 支持的按钮数，保留 C / Z 与模拟扳机的原有槽位
pub(crate) const BUTTON_COUNT: usize = 19;
/// 支持的轴数，包含由十字键合成的两根轴
pub(crate) const AXIS_COUNT: usize = 8;

/// 对外按钮顺序，映射与转发线程按此索引
const BUTTONS: [Button; BUTTON_COUNT] = [
    Button::South,
    Button::East,
    Button::North,
    Button::West,
    Button::C,
    Button::Z,
    Button::LeftTrigger,
    Button::LeftTrigger2,
    Button::RightTrigger,
    Button::RightTrigger2,
    Button::Select,
    Button::Start,
    Button::Mode,
    Button::LeftThumb,
    Button::RightThumb,
    Button::DPadUp,
    Button::DPadDown,
    Button::DPadLeft,
    Button::DPadRight,
];

/// 对外轴顺序，保持已有配置的索引不变
const AXES: [Axis; AXIS_COUNT] = [
    Axis::LeftStickX,
    Axis::LeftStickY,
    Axis::LeftZ,
    Axis::RightStickX,
    Axis::RightStickY,
    Axis::RightZ,
    Axis::DPadX,
    Axis::DPadY,
];

/// SDL 标准按钮与原有配置枚举的对应关系
const SDL_BUTTONS: [(SdlButton, Button); 15] = [
    (SdlButton::A, Button::South),
    (SdlButton::B, Button::East),
    (SdlButton::Y, Button::North),
    (SdlButton::X, Button::West),
    (SdlButton::LeftShoulder, Button::LeftTrigger),
    (SdlButton::RightShoulder, Button::RightTrigger),
    (SdlButton::Back, Button::Select),
    (SdlButton::Start, Button::Start),
    (SdlButton::Guide, Button::Mode),
    (SdlButton::LeftStick, Button::LeftThumb),
    (SdlButton::RightStick, Button::RightThumb),
    (SdlButton::DPadUp, Button::DPadUp),
    (SdlButton::DPadDown, Button::DPadDown),
    (SdlButton::DPadLeft, Button::DPadLeft),
    (SdlButton::DPadRight, Button::DPadRight),
];

/// 连接时读取的模拟轴
const SDL_AXES: [SdlAxis; 6] = [
    SdlAxis::LeftX,
    SdlAxis::LeftY,
    SdlAxis::RightX,
    SdlAxis::RightY,
    SdlAxis::TriggerLeft,
    SdlAxis::TriggerRight,
];

/// SDL 能识别、但不在原有固定按钮表内的按键
const EXTRA_BUTTONS: [SdlButton; 6] = [
    SdlButton::Misc1,
    SdlButton::Paddle1,
    SdlButton::Paddle2,
    SdlButton::Paddle3,
    SdlButton::Paddle4,
    SdlButton::Touchpad,
];

/// 已连接手柄的状态，键为 SDL 实例编号
static PADS: LazyLock<RwLock<BTreeMap<u64, Pad>>> = LazyLock::new(|| RwLock::new(BTreeMap::new()));
/// 读取线程只启动一次
static INIT: Once = Once::new();

/// 单个手柄的当前值
struct Pad {
    /// 系统给出的名称
    name: String,
    /// 自动认出的外观，认不出时由界面回落到配置
    layout: Option<PadLayout>,
    /// 与 BUTTONS 同序，取值 0 到 1
    buttons: [f32; BUTTON_COUNT],
    /// 与 AXES 同序，摇杆取值 -1 到 1，扳机取值 0 到 1
    axes: [f32; AXIS_COUNT],
    /// 按第一次按下的顺序追加，松开后保留编号
    extras: Vec<ExtraButton>,
}

impl Pad {
    /// 建立空状态
    fn empty(name: String, layout: Option<PadLayout>) -> Self {
        Self {
            name,
            layout,
            buttons: [0.0; BUTTON_COUNT],
            axes: [0.0; AXIS_COUNT],
            extras: Vec::new(),
        }
    }

    /// 连接时采样，避免漏掉打开设备前已按住的按钮
    fn from_controller(controller: &GameController) -> Self {
        let mut pad = Self::empty(controller.name(), detect_layout(controller));
        for (button, _) in SDL_BUTTONS {
            pad.set_button(button, controller.button(button));
        }
        for button in EXTRA_BUTTONS {
            if controller.button(button) {
                pad.set_button(button, true);
            }
        }
        for axis in SDL_AXES {
            pad.set_axis(axis, controller.axis(axis));
        }
        pad
    }

    /// 更新标准按钮与额外按钮，并同步十字键轴
    fn set_button(&mut self, button: SdlButton, pressed: bool) {
        let value = u8::from(pressed) as f32;
        if let Some((_, mapped)) = SDL_BUTTONS.iter().find(|(item, _)| *item == button) {
            self.buttons[button_index(*mapped).unwrap()] = value;
            self.axes[axis_index(Axis::DPadX).unwrap()] = self.buttons
                [button_index(Button::DPadRight).unwrap()]
                - self.buttons[button_index(Button::DPadLeft).unwrap()];
            self.axes[axis_index(Axis::DPadY).unwrap()] = self.buttons
                [button_index(Button::DPadUp).unwrap()]
                - self.buttons[button_index(Button::DPadDown).unwrap()];
        } else {
            let code = button as u32;
            match self.extras.iter_mut().find(|item| item.code == code) {
                Some(item) => item.value = value,
                None if pressed => self.extras.push(ExtraButton { code, value }),
                None => {}
            }
        }
    }

    /// 归一化 SDL 轴值，模拟扳机同时更新按钮槽位
    fn set_axis(&mut self, axis: SdlAxis, raw: i16) {
        let value = (raw as f32 / i16::MAX as f32).clamp(-1.0, 1.0);
        let (mapped, value) = match axis {
            SdlAxis::LeftX => (Axis::LeftStickX, value),
            SdlAxis::LeftY => (Axis::LeftStickY, -value),
            SdlAxis::RightX => (Axis::RightStickX, value),
            SdlAxis::RightY => (Axis::RightStickY, -value),
            SdlAxis::TriggerLeft => {
                let value = value.max(0.0);
                self.buttons[button_index(Button::LeftTrigger2).unwrap()] = value;
                (Axis::LeftZ, value)
            }
            SdlAxis::TriggerRight => {
                let value = value.max(0.0);
                self.buttons[button_index(Button::RightTrigger2).unwrap()] = value;
                (Axis::RightZ, value)
            }
        };
        self.axes[axis_index(mapped).unwrap()] = value;
    }

    /// 转成对外快照
    fn snapshot(&self, id: u64) -> PadState {
        PadState {
            id,
            name: self.name.clone(),
            buttons: BUTTONS
                .iter()
                .enumerate()
                .map(|(index, button)| ButtonValue {
                    button: *button,
                    value: self.buttons[index],
                })
                .collect(),
            axes: AXES
                .iter()
                .enumerate()
                .map(|(index, axis)| AxisValue {
                    axis: *axis,
                    value: self.axes[index],
                })
                .collect(),
            extras: self.extras.clone(),
        }
    }
}

/// 从手柄身份猜外观
///
/// 只影响界面上的按钮记号与状态图，猜错也能在设置里手动改。
///
/// - `gamepad`: SDL 的手柄
///
/// # 返回值
///
/// 认得出是 Xbox 还是 PlayStation 时返回对应外观；认不出返回 `None`
fn detect_layout(gamepad: &GameController) -> Option<PadLayout> {
    // 蓝牙手柄的名字普遍带 "wireless controller"，先认 Xbox，免得把 Xbox 判成索尼
    if gamepad.vendor_id() == Some(0x045E) || gamepad.name().to_lowercase().contains("xbox") {
        return Some(PadLayout::Xbox);
    }
    // 索尼：DualShock / DualSense，Windows 上常报成通用的 "Wireless Controller"
    let name = gamepad.name().to_lowercase();
    if gamepad.vendor_id() == Some(0x054C)
        || name.contains("dualsense")
        || name.contains("dualshock")
        || name.contains("playstation")
        || name.contains("sony")
        || name.contains("wireless controller")
    {
        return Some(PadLayout::Playstation);
    }
    None
}

/// 已连接的手柄
#[derive(Debug, Clone)]
pub struct PadInfo {
    /// 运行期间的编号，重连后可能变化
    pub id: u64,
    /// 系统给出的手柄名称
    pub name: String,
    /// 自动认出的外观；认不出时为 `None`
    pub layout: Option<PadLayout>,
}

/// 一个按钮的当前值
#[derive(Debug, Clone, Copy)]
pub struct ButtonValue {
    /// 按钮
    pub button: Button,
    /// 按下程度，0.0 到 1.0；普通按钮只有 0 和 1
    pub value: f32,
}

/// 一个轴的当前值
#[derive(Debug, Clone, Copy)]
pub struct AxisValue {
    /// 轴
    pub axis: Axis,
    /// 轴的值，-1.0 到 1.0
    pub value: f32,
}

/// 固定按钮表里没有的按键
#[derive(Debug, Clone, Copy)]
pub struct ExtraButton {
    /// 底层编号，界面上直接显示
    pub code: u32,
    /// 按下程度，0.0 到 1.0
    pub value: f32,
}

/// 手柄状态快照
#[derive(Debug, Clone)]
pub struct PadState {
    /// 运行期间的编号
    pub id: u64,
    /// 系统给出的手柄名称
    pub name: String,
    /// 全部按钮的当前值
    pub buttons: Vec<ButtonValue>,
    /// 全部轴的当前值
    pub axes: Vec<AxisValue>,
    /// 固定表里没有的按键，按第一次按下的顺序
    pub extras: Vec<ExtraButton>,
}

/// 启动读取线程，只启动一次；进程退出时线程随进程结束
pub(crate) fn start() {
    INIT.call_once(|| {
        let _ = thread::Builder::new()
            .name("Mml Gamepad".to_string())
            .spawn(run);
    });
}

/// 初始化失败时保留空列表，不影响启动器其它功能
fn run() {
    if let Err(error) = read_events() {
        mml_log::warn_type(InfoType::GamepadInitFailed { error });
    }
}

/// 在同一线程内初始化、持有和释放所有 SDL 对象
fn read_events() -> Result<(), String> {
    // 游戏在前台时启动器仍需读取；提示必须在控制器子系统初始化前设置。
    sdl2::hint::set("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "1");
    let sdl = sdl2::init()?;
    let subsystem = sdl.game_controller()?;
    let mut events = sdl.event_pump()?;
    let mut controllers = BTreeMap::new();

    for index in 0..subsystem.num_joysticks()? {
        open_controller(&subsystem, &mut controllers, index);
    }

    loop {
        for event in events.poll_iter() {
            match event {
                Event::ControllerDeviceAdded { which, .. } => {
                    open_controller(&subsystem, &mut controllers, which);
                }
                Event::ControllerDeviceRemoved { which, .. } => {
                    controllers.remove(&which);
                    let removed = PADS.write().unwrap().remove(&u64::from(which));
                    if let Some(pad) = removed {
                        mml_log::info_type(InfoType::GamepadDisconnected { name: pad.name });
                    }
                }
                Event::ControllerButtonDown { which, button, .. } => {
                    if let Some(pad) = PADS.write().unwrap().get_mut(&u64::from(which)) {
                        pad.set_button(button, true);
                    }
                }
                Event::ControllerButtonUp { which, button, .. } => {
                    if let Some(pad) = PADS.write().unwrap().get_mut(&u64::from(which)) {
                        pad.set_button(button, false);
                    }
                }
                Event::ControllerAxisMotion {
                    which, axis, value, ..
                } => {
                    if let Some(pad) = PADS.write().unwrap().get_mut(&u64::from(which)) {
                        pad.set_axis(axis, value);
                    }
                }
                _ => {}
            }
        }
        // 每轮先排空事件，再短暂休眠，兼顾摇杆响应与空闲占用。
        thread::sleep(Duration::from_millis(8));
    }
}

/// 新增事件的编号是设备索引，打开后的实例编号才用于后续状态与拔出事件
fn open_controller(
    subsystem: &GameControllerSubsystem,
    controllers: &mut BTreeMap<u32, GameController>,
    index: u32,
) {
    if !subsystem.is_game_controller(index) {
        return;
    }
    let controller = match subsystem.open(index) {
        Ok(controller) => controller,
        Err(error) => {
            mml_log::warn_type(InfoType::GamepadInitFailed {
                error: error.to_string(),
            });
            return;
        }
    };
    let id = controller.instance_id();
    // 初始枚举与队列里的新增事件可能指向同一只手柄。
    if controllers.contains_key(&id) {
        return;
    }
    let pad = Pad::from_controller(&controller);
    mml_log::info_type(InfoType::GamepadConnected {
        name: pad.name.clone(),
    });
    PADS.write().unwrap().insert(u64::from(id), pad);
    controllers.insert(id, controller);
}

/// 按钮在状态数组中的下标
pub(crate) fn button_index(button: Button) -> Option<usize> {
    BUTTONS.iter().position(|item| *item == button)
}

/// 轴在状态数组中的下标
pub(crate) fn axis_index(axis: Axis) -> Option<usize> {
    AXES.iter().position(|item| *item == axis)
}

/// 手柄的原始值，供转发线程按数组下标读取
pub(crate) struct PadValues {
    /// 与 BUTTONS 同序的按钮值
    pub(crate) buttons: [f32; BUTTON_COUNT],
    /// 与 AXES 同序的轴值
    pub(crate) axes: [f32; AXIS_COUNT],
}

/// 读取一个手柄的原始值
pub(crate) fn pad_values(id: u64) -> Option<PadValues> {
    PADS.read().unwrap().get(&id).map(|pad| PadValues {
        buttons: pad.buttons,
        axes: pad.axes,
    })
}

/// 取第一个已连接手柄的编号
pub(crate) fn first_pad_id() -> Option<u64> {
    PADS.read().unwrap().keys().next().copied()
}

/// 列出当前已连接的手柄
///
/// # 返回值
///
/// 按编号升序返回；没有手柄时为空列表
pub fn pads() -> Vec<PadInfo> {
    PADS.read()
        .unwrap()
        .iter()
        .map(|(id, pad)| PadInfo {
            id: *id,
            name: pad.name.clone(),
            layout: pad.layout,
        })
        .collect()
}

/// 读取一个手柄的当前状态
///
/// - `id`: pads 返回的编号
///
/// # 返回值
///
/// 手柄已连接时返回快照，找不到返回 None
pub fn pad_state(id: u64) -> Option<PadState> {
    PADS.read().unwrap().get(&id).map(|pad| pad.snapshot(id))
}

#[cfg(test)]
#[path = "../tests/unit/reader_tests.rs"]
mod tests;
