//! 手柄读取
//!
//! 用 gilrs 在独立线程监听系统手柄，维护每个手柄的按钮 / 摇杆快照，
//! 供状态查询与转发线程使用。按钮与轴统一到 Xbox 布局，见 crate::map 的 Button / Axis。

use std::{
    collections::BTreeMap,
    path::Path,
    sync::{
        LazyLock, Once, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use gilrs::{Axis as GilrsAxis, Button as GilrsButton, Event, EventType, Gamepad, Gilrs};
use mml_names::i18_items::info_type::InfoType;
use serde::{Deserialize, Serialize};

use crate::map::{Axis, Button, GamepadError, PadLayout};

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
    /// 自动认出的外观；认不出时为 `None`，界面按配置里的外观显示
    layout: Option<PadLayout>,
    /// 与 BUTTONS 同序的按钮值，0.0 到 1.0
    buttons: [f32; BUTTON_COUNT],
    /// 与 AXES 同序的轴值，-1.0 到 1.0
    axes: [f32; AXIS_COUNT],
    /// 固定按钮表里没有的按键，按第一次按下的顺序排
    extras: Vec<ExtraButton>,
    /// 这份状态是不是 XInput 单独建的（仅 Windows）：WGI 稍后报到同一只手柄时，把名字补进来
    #[cfg_attr(not(windows), allow(dead_code))]
    synthetic: bool,
}

impl Pad {
    /// 建一个空状态
    fn empty(name: String, layout: Option<PadLayout>) -> Self {
        Self {
            name,
            layout,
            buttons: [0.0; BUTTON_COUNT],
            axes: [0.0; AXIS_COUNT],
            extras: Vec::new(),
            synthetic: false,
        }
    }

    /// 从 gilrs 的当前状态建快照，用于刚连上的手柄
    fn from_gamepad(gamepad: &Gamepad<'_>) -> Self {
        let mut pad = Self::empty(gamepad.name().to_string(), detect_layout(gamepad));
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
    ///
    /// 固定表里没有的按键（高级手柄背面的拨片这类）按底层编号单独记一份，
    /// 界面上在手柄下面**按出现的先后往后追加**；松开只把值归零，编号留着，
    /// 列表不会跳来跳去。
    fn set_button(&mut self, button: GilrsButton, code: u32, value: f32) {
        match GILRS_BUTTONS.iter().position(|item| *item == button) {
            Some(index) => self.buttons[index] = value,
            None => match self.extras.iter_mut().find(|item| item.code == code) {
                Some(item) => item.value = value,
                None => self.extras.push(ExtraButton { code, value }),
            },
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
            extras: self.extras.clone(),
        }
    }
}

/// 从手柄身份猜外观
///
/// 只影响界面上的按钮记号与状态图，猜错也能在设置里手动改。
///
/// - `gamepad`: gilrs 的手柄
///
/// # 返回值
///
/// 认得出是 Xbox 还是 PlayStation 时返回对应外观；认不出返回 `None`
fn detect_layout(gamepad: &Gamepad<'_>) -> Option<PadLayout> {
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
    /// 固定表里没有的按键，按编号升序
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
    let mut pads = PADS.write().unwrap();
    for (id, gamepad) in gilrs.gamepads() {
        pads.insert(pad_id(id), Pad::from_gamepad(&gamepad));
    }
    drop(pads);

    loop {
        // 阻塞 16ms：没事件时让出 CPU，有事件立刻返回；同时这也是 XInput 的轮询节拍，
        // 慢过 16ms 摇杆会一顿一顿
        if let Some(event) = gilrs.next_event_blocking(Some(Duration::from_millis(16))) {
            handle(&mut gilrs, event);
        }
        #[cfg(windows)]
        if xinput() {
            sync_xinput();
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
            // 已经被 XInput 接手的那只手柄：WGI 这条只是同一只手柄的另一份枚举
            #[cfg(windows)]
            if adopt_xinput_name(&name) {
                mml_log::info_type(InfoType::GamepadConnected { name });
                return;
            }
            PADS.write().unwrap().insert(id, pad);
            mml_log::info_type(InfoType::GamepadConnected { name });
        }
        EventType::Disconnected => {
            let name = PADS.write().unwrap().remove(&id).map(|pad| pad.name);
            if let Some(name) = name {
                mml_log::info_type(InfoType::GamepadDisconnected { name });
            }
        }
        EventType::ButtonPressed(button, code) => set_button(id, button, code.into_u32(), 1.0),
        EventType::ButtonReleased(button, code) => set_button(id, button, code.into_u32(), 0.0),
        EventType::ButtonChanged(button, value, code) => {
            set_button(id, button, code.into_u32(), value)
        }
        EventType::AxisChanged(axis, value, _) => set_axis(id, axis, value),
        _ => {}
    }
}

/// XInput 槽位 → 手柄编号的归属
#[cfg(windows)]
static XINPUT_SLOTS: std::sync::Mutex<[Option<u64>; crate::xinput::SLOT_COUNT]> =
    std::sync::Mutex::new([None; crate::xinput::SLOT_COUNT]);

/// XInput 兼容模式是否打开，见 [set_xinput]
///
/// **默认开**：WGI 要求进程有前台窗口才收得到手柄事件，手柄映射的典型场景是
/// "游戏在前台、启动器在后台"，那时 WGI 一个事件都没有，只有 XInput 照常。
static XINPUT: AtomicBool = AtomicBool::new(true);

/// 读取选项，落盘用大驼峰
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
struct ReaderSettings {
    /// 是否用 XInput 读取；只影响 Windows
    xinput: bool,
}

impl Default for ReaderSettings {
    fn default() -> Self {
        // 文件里没写这一项时按打开处理，和 XINPUT 的初值一致
        Self { xinput: true }
    }
}

/// 读一次 `gamepad/reader.json` 里的读取选项
///
/// - `dir`: 手柄配置目录
pub(crate) fn load_settings(dir: &Path) {
    let file = dir.join(mml_names::names::GAMEPAD_READER_FILE);
    let Ok(text) = std::fs::read_to_string(&file) else {
        return;
    };
    match serde_json::from_str::<ReaderSettings>(&text) {
        Ok(settings) => XINPUT.store(settings.xinput, Ordering::Release),
        Err(_) => mml_log::info(String::from("[gamepad] reader.json 解析失败，按默认选项读手柄")),
    }
}

/// XInput 兼容模式是否打开
///
/// 只影响 Windows：其它平台的输入本来就来自 gilrs。
pub fn xinput() -> bool {
    XINPUT.load(Ordering::Acquire)
}

/// 打开 / 关掉 XInput 兼容模式并落盘
///
/// - `enabled`: 是否打开
///
/// # 返回值
///
/// 写盘失败返回对应错误；配置目录还没就绪时返回 `GamepadError::Missing`
pub fn set_xinput(enabled: bool) -> Result<(), GamepadError> {
    XINPUT.store(enabled, Ordering::Release);
    let dir = crate::map::config_dir().ok_or(GamepadError::Missing)?;
    let text = serde_json::to_string_pretty(&ReaderSettings { xinput: enabled })
        .map_err(|err| GamepadError::Io(err.to_string()))?;
    std::fs::write(dir.join(mml_names::names::GAMEPAD_READER_FILE), text)
        .map_err(|err| GamepadError::Io(err.to_string()))
}

/// 用 XInput 读手柄
///
/// gilrs 走 WGI，而 WGI 要求进程有前台窗口才收得到事件：后台时一个按键都没有，
/// 只有 XInput 照常。这里按槽位轮询，把值写进同一份手柄状态：槽位优先认领已有的
/// Xbox 外观手柄，认领不到才新建一份，界面因此不会看到两只手柄。
#[cfg(windows)]
fn sync_xinput() {
    let mut slots = XINPUT_SLOTS.lock().unwrap();
    for slot in 0..crate::xinput::SLOT_COUNT {
        let Some(state) = crate::xinput::poll(slot as u32) else {
            // 槽空了：自己新建的那份要收掉，认领来的交给 gilrs 的断开事件处理
            if let Some(id) = slots[slot].take()
                && id >= XINPUT_ID_BASE
            {
                PADS.write().unwrap().remove(&id);
            }
            continue;
        };

        let id = match slots[slot] {
            Some(id) if PADS.read().unwrap().contains_key(&id) => id,
            _ => {
                let id = claim_xinput(slot as u64, &slots);
                slots[slot] = Some(id);
                id
            }
        };
        if let Some(pad) = PADS.write().unwrap().get_mut(&id) {
            pad.buttons = state.buttons;
            pad.axes = state.axes;
        }
    }
}

/// 把 WGI 报上来的名字补给 XInput 单独建的那份状态
///
/// 蓝牙 Xbox 手柄会先后被两个后端看到：XInput 先给出读数，WGI 稍后才报"已连接"。
/// 后者是同一只手柄的另一份枚举，不能单独登记，否则界面上会出现两只手柄。
///
/// - `name`: WGI 给出的名字
///
/// # 返回值
///
/// 真的补上了（说明它是 XInput 建的那份）时返回 `true`，调用方不再登记这只手柄
#[cfg(windows)]
fn adopt_xinput_name(name: &str) -> bool {
    let slots = XINPUT_SLOTS.lock().unwrap();
    let mut pads = PADS.write().unwrap();
    for id in slots.iter().flatten() {
        if *id < XINPUT_ID_BASE {
            continue;
        }
        if let Some(pad) = pads.get_mut(id)
            && pad.synthetic
        {
            pad.name = name.to_string();
            pad.synthetic = false;
            return true;
        }
    }
    false
}

/// XInput 单独新建的手柄编号从这里开始，避开 gilrs 的小整数编号
#[cfg(windows)]
const XINPUT_ID_BASE: u64 = 0x0005_8000_0000;

/// 给一个 XInput 槽找一份手柄状态：优先认领还没被别的槽占用的 Xbox 外观手柄
///
/// - `slot`: 槽位
/// - `slots`: 当前归属，用来跳过已被认领的
///
/// # 返回值
///
/// 该槽使用的编号；没有可认领的时新建一份并返回它的编号
#[cfg(windows)]
fn claim_xinput(slot: u64, slots: &[Option<u64>; crate::xinput::SLOT_COUNT]) -> u64 {
    let claimed: Vec<u64> = slots.iter().flatten().copied().collect();
    let mut pads = PADS.write().unwrap();
    if let Some((id, _)) = pads
        .iter()
        .find(|(id, pad)| pad.layout == Some(PadLayout::Xbox) && !claimed.contains(id))
    {
        return *id;
    }
    let id = XINPUT_ID_BASE + slot;
    let mut pad = Pad::empty(String::from("Xbox Controller"), Some(PadLayout::Xbox));
    pad.synthetic = true;
    pads.insert(id, pad);
    id
}

/// 更新一个手柄的按钮值
///
/// - `id`: 手柄编号
/// - `button`: gilrs 的按钮
/// - `code`: 底层编号，固定表里没有的按键靠它区分
/// - `value`: 按下程度
fn set_button(id: u64, button: GilrsButton, code: u32, value: f32) {
    if let Some(pad) = PADS.write().unwrap().get_mut(&id) {
        pad.set_button(button, code, value);
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
