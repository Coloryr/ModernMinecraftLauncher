//! 手柄读取
//!
//! 用 gilrs 在独立线程监听系统手柄，维护每个手柄的按钮 / 摇杆快照，
//! 供状态查询与转发线程使用。按钮与轴统一到 Xbox 布局，见 crate::map 的 Button / Axis。

use std::{
    collections::BTreeMap,
    sync::{LazyLock, Once, RwLock},
    thread,
    time::Duration,
};

use gilrs::{Axis as GilrsAxis, Button as GilrsButton, Event, EventType, Gamepad, Gilrs};
use mml_names::i18_items::info_type::InfoType;

use crate::map::{Axis, Button};

/// 支持的按钮数
pub(crate) const BUTTON_COUNT: usize = 19;
/// 支持的轴数
pub(crate) const AXIS_COUNT: usize = 8;

/// 支持的按钮，按固定顺序与状态数组对应
const GILRS_BUTTONS: [GilrsButton; BUTTON_COUNT] = [
    GilrsButton::South,
    GilrsButton::East,
    GilrsButton::North,
    GilrsButton::West,
    GilrsButton::C,
    GilrsButton::Z,
    GilrsButton::LeftTrigger,
    GilrsButton::LeftTrigger2,
    GilrsButton::RightTrigger,
    GilrsButton::RightTrigger2,
    GilrsButton::Select,
    GilrsButton::Start,
    GilrsButton::Mode,
    GilrsButton::LeftThumb,
    GilrsButton::RightThumb,
    GilrsButton::DPadUp,
    GilrsButton::DPadDown,
    GilrsButton::DPadLeft,
    GilrsButton::DPadRight,
];

/// 支持的按钮对应的公开枚举，顺序与 GILRS_BUTTONS 一致
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

/// 支持的轴，按固定顺序与状态数组对应
const GILRS_AXES: [GilrsAxis; AXIS_COUNT] = [
    GilrsAxis::LeftStickX,
    GilrsAxis::LeftStickY,
    GilrsAxis::LeftZ,
    GilrsAxis::RightStickX,
    GilrsAxis::RightStickY,
    GilrsAxis::RightZ,
    GilrsAxis::DPadX,
    GilrsAxis::DPadY,
];

/// 支持的轴对应的公开枚举，顺序与 GILRS_AXES 一致
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

/// 已连接手柄的状态，键为运行期间的编号
static PADS: LazyLock<RwLock<BTreeMap<u64, Pad>>> = LazyLock::new(|| RwLock::new(BTreeMap::new()));

/// 读取线程只启动一次
static INIT: Once = Once::new();

/// 单个手柄的当前值
struct Pad {
    /// 系统给出的名称
    name: String,
    /// 与 BUTTONS 同序的按钮值，0.0 到 1.0
    buttons: [f32; BUTTON_COUNT],
    /// 与 AXES 同序的轴值，-1.0 到 1.0
    axes: [f32; AXIS_COUNT],
}

impl Pad {
    /// 建一个空状态
    fn empty(name: String) -> Self {
        Self {
            name,
            buttons: [0.0; BUTTON_COUNT],
            axes: [0.0; AXIS_COUNT],
        }
    }

    /// 从 gilrs 的当前状态建快照，用于刚连上的手柄
    fn from_gamepad(gamepad: &Gamepad<'_>) -> Self {
        let mut pad = Self::empty(gamepad.name().to_string());
        for (index, button) in GILRS_BUTTONS.iter().enumerate() {
            if let Some(data) = gamepad.button_data(*button) {
                pad.buttons[index] = data.value();
            }
        }
        for (index, axis) in GILRS_AXES.iter().enumerate() {
            if let Some(data) = gamepad.axis_data(*axis) {
                pad.axes[index] = data.value();
            }
        }
        pad
    }

    /// 记录一个按钮值
    fn set_button(&mut self, button: GilrsButton, value: f32) {
        if let Some(index) = GILRS_BUTTONS.iter().position(|item| *item == button) {
            self.buttons[index] = value;
        }
    }

    /// 记录一个轴值
    fn set_axis(&mut self, axis: GilrsAxis, value: f32) {
        if let Some(index) = GILRS_AXES.iter().position(|item| *item == axis) {
            self.axes[index] = value;
        }
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
        }
    }
}

/// 已连接的手柄
#[derive(Debug, Clone)]
pub struct PadInfo {
    /// 运行期间的编号，重连后可能变化
    pub id: u64,
    /// 系统给出的手柄名称
    pub name: String,
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
}

/// 启动读取线程，只启动一次；进程退出时线程随进程结束
pub(crate) fn start() {
    INIT.call_once(|| {
        let _ = thread::Builder::new()
            .name("Mml Gamepad".to_string())
            .spawn(run);
    });
}

/// 读取线程主体
fn run() {
    let mut gilrs = match Gilrs::new() {
        Ok(gilrs) => gilrs,
        // 没有可用的手柄后端时保持空列表，不影响启动与其它功能
        Err(err) => {
            mml_log::warn_type(InfoType::GamepadInitFailed {
                error: err.to_string(),
            });
            return;
        }
    };

    // 启动前已经连着的手柄
    {
        let mut pads = PADS.write().unwrap();
        for (id, gamepad) in gilrs.gamepads() {
            pads.insert(pad_id(id), Pad::from_gamepad(&gamepad));
        }
    }

    loop {
        // 超时返回 None，让循环有机会结束；进程退出时线程直接随进程走
        if let Some(event) = gilrs.next_event_blocking(Some(Duration::from_millis(100))) {
            handle(&mut gilrs, event);
        }
    }
}

/// 处理一条 gilrs 事件
///
/// - `gilrs`: 读取器，取刚连上的手柄快照
/// - `event`: 本次事件
fn handle(gilrs: &mut Gilrs, event: Event) {
    let id = pad_id(event.id);
    match event.event {
        EventType::Connected => {
            let pad = Pad::from_gamepad(&gilrs.gamepad(event.id));
            let name = pad.name.clone();
            PADS.write().unwrap().insert(id, pad);
            mml_log::info_type(InfoType::GamepadConnected { name });
        }
        EventType::Disconnected => {
            let name = PADS.write().unwrap().remove(&id).map(|pad| pad.name);
            if let Some(name) = name {
                mml_log::info_type(InfoType::GamepadDisconnected { name });
            }
        }
        EventType::ButtonPressed(button, _) => set_button(id, button, 1.0),
        EventType::ButtonReleased(button, _) => set_button(id, button, 0.0),
        EventType::ButtonChanged(button, value, _) => set_button(id, button, value),
        EventType::AxisChanged(axis, value, _) => set_axis(id, axis, value),
        _ => {}
    }
}

/// 更新一个手柄的按钮值
fn set_button(id: u64, button: GilrsButton, value: f32) {
    if let Some(pad) = PADS.write().unwrap().get_mut(&id) {
        pad.set_button(button, value);
    }
}

/// 更新一个手柄的轴值
fn set_axis(id: u64, axis: GilrsAxis, value: f32) {
    if let Some(pad) = PADS.write().unwrap().get_mut(&id) {
        pad.set_axis(axis, value);
    }
}

/// gilrs 的手柄编号转成跨线程的编号
fn pad_id(id: gilrs::GamepadId) -> u64 {
    let raw: usize = id.into();
    raw as u64
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
mod tests {
    use super::*;

    /// 按钮 / 轴表与状态数组一一对应，且没有重复项
    #[test]
    fn button_and_axis_tables_match() {
        for (index, button) in BUTTONS.iter().enumerate() {
            assert!(!BUTTONS[..index].contains(button));
        }
        for (index, axis) in AXES.iter().enumerate() {
            assert!(!AXES[..index].contains(axis));
        }
        for (index, button) in GILRS_BUTTONS.iter().enumerate() {
            assert!(!GILRS_BUTTONS[..index].contains(button));
        }
        for (index, axis) in GILRS_AXES.iter().enumerate() {
            assert!(!GILRS_AXES[..index].contains(axis));
        }
        assert_eq!(GILRS_BUTTONS.len(), BUTTON_COUNT);
        assert_eq!(GILRS_AXES.len(), AXIS_COUNT);
    }

    /// 没有手柄时列表为空，查询不存在的编号返回 None
    #[test]
    fn empty_state() {
        assert!(pads().is_empty());
        assert!(pad_state(1).is_none());
    }

    /// 快照按顺序带上全部按钮 / 轴，并保留设置的值
    #[test]
    fn snapshot_keeps_values() {
        let mut pad = Pad::empty(String::from("test pad"));
        pad.set_button(GilrsButton::South, 1.0);
        pad.set_axis(GilrsAxis::LeftStickX, -0.5);

        let state = pad.snapshot(7);
        assert_eq!(state.id, 7);
        assert_eq!(state.name, "test pad");
        assert_eq!(state.buttons.len(), BUTTON_COUNT);
        assert_eq!(state.axes.len(), AXIS_COUNT);
        assert_eq!(
            state
                .buttons
                .iter()
                .find(|item| item.button == Button::South)
                .map(|item| item.value),
            Some(1.0)
        );
        assert_eq!(
            state
                .axes
                .iter()
                .find(|item| item.axis == Axis::LeftStickX)
                .map(|item| item.value),
            Some(-0.5)
        );
    }

    /// 手动验证手柄读取：接上手柄后运行
    /// cargo test -p mml-gamepad --lib gamepad_manual -- --ignored --nocapture
    #[test]
    #[ignore = "需要接手柄"]
    fn gamepad_manual() {
        let dir = mml_testutil::temp_dir().join(format!("gamepad_manual_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        mml_log::start(&dir).unwrap();

        crate::init(&dir);
        thread::sleep(Duration::from_secs(2));
        for pad in pads() {
            println!("手柄 {}：{}", pad.id, pad.name);
            if let Some(state) = pad_state(pad.id) {
                println!("{state:?}");
            }
        }
    }
}
