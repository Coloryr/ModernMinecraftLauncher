//! Windows 的 XInput 后备读取
//!
//! 蓝牙连的 Xbox 手柄在 Windows 上会同时出现在 HID 与 XInput 两处：gilrs 走 WGI，
//! 只收到"已连接"、收不到任何按键与摇杆事件（实机确认），而 XInput 那边一切正常。
//! 这里直接按槽位轮询 XInput，把这部分输入补进同一份手柄状态里。
//!
//! 只负责"读"：归属关系（哪个槽对应哪份状态）由 reader 维护，见 `reader::sync_xinput`。
#![cfg(windows)]

use windows::Win32::UI::Input::XboxController::{XINPUT_GAMEPAD, XINPUT_STATE, XInputGetState};

use crate::map::{Axis, Button};
use crate::reader::{AXIS_COUNT, BUTTON_COUNT, axis_index, button_index};

/// XInput 最多四个槽位
pub(crate) const SLOT_COUNT: usize = 4;

/// 一个槽位当前的输入
pub(crate) struct SlotState {
    /// 与 `BUTTONS` 同序的按钮值
    pub(crate) buttons: [f32; BUTTON_COUNT],
    /// 与 `AXES` 同序的轴值，扳机也在里面
    pub(crate) axes: [f32; AXIS_COUNT],
}

/// 读一个槽位的当前输入
///
/// - `slot`: 槽位，0 到 3
///
/// # 返回值
///
/// 这个槽上有手柄时返回快照；没插 / 已断开返回 `None`
pub(crate) fn poll(slot: u32) -> Option<SlotState> {
    let mut state = XINPUT_STATE::default();
    // 返回 0 才是"有手柄"；1167 是 ERROR_DEVICE_NOT_CONNECTED
    if unsafe { XInputGetState(slot, &mut state) } != 0 {
        return None;
    }
    let pad = &state.Gamepad;
    Some(SlotState {
        buttons: buttons_of(pad),
        axes: axes_of(pad),
    })
}

/// 按下状态转按钮数组
///
/// 扳机同时是按钮和轴：按钮这边给 0.0 到 1.0 的模拟量，界面与映射都按"按下程度"用；
/// 肩键（LB / RB）才是纯数字的那两位。
///
/// - `pad`: XInput 的按键位与扳机行程
fn buttons_of(pad: &XINPUT_GAMEPAD) -> [f32; BUTTON_COUNT] {
    let raw = pad.wButtons.0;
    let mut buttons = [0.0; BUTTON_COUNT];
    set_value(
        &mut buttons,
        Button::LeftTrigger,
        f32::from(pad.bLeftTrigger) / 255.0,
    );
    set_value(
        &mut buttons,
        Button::RightTrigger,
        f32::from(pad.bRightTrigger) / 255.0,
    );
    for (mask, button) in [
        (0x1000u16, Button::South),
        (0x2000, Button::East),
        (0x4000, Button::West),
        (0x8000, Button::North),
        (0x0100, Button::LeftTrigger2),
        (0x0200, Button::RightTrigger2),
        (0x0010, Button::Start),
        (0x0020, Button::Select),
        (0x0040, Button::LeftThumb),
        (0x0080, Button::RightThumb),
        (0x0001, Button::DPadUp),
        (0x0002, Button::DPadDown),
        (0x0004, Button::DPadLeft),
        (0x0008, Button::DPadRight),
    ] {
        if raw & mask != 0 {
            set_value(&mut buttons, button, 1.0);
        }
    }
    buttons
}

/// 按按钮写一个值
///
/// - `buttons`: 与 `BUTTONS` 同序的按钮值
/// - `button`: 目标按钮
/// - `value`: 按下程度，0.0 到 1.0
fn set_value(buttons: &mut [f32; BUTTON_COUNT], button: Button, value: f32) {
    if let Some(index) = button_index(button) {
        buttons[index] = value;
    }
}

/// 摇杆与扳机转轴数组
///
/// - `pad`: XInput 的轴值
fn axes_of(pad: &XINPUT_GAMEPAD) -> [f32; AXIS_COUNT] {
    let mut axes = [0.0; AXIS_COUNT];
    // 扳机也进轴数组：映射配置里可以用 leftZ / rightZ 当轴来用
    for (axis, value) in [
        (Axis::LeftStickX, f32::from(pad.sThumbLX) / 32767.0),
        (Axis::LeftStickY, f32::from(pad.sThumbLY) / 32767.0),
        (Axis::RightStickX, f32::from(pad.sThumbRX) / 32767.0),
        (Axis::RightStickY, f32::from(pad.sThumbRY) / 32767.0),
        (Axis::LeftZ, f32::from(pad.bLeftTrigger) / 255.0),
        (Axis::RightZ, f32::from(pad.bRightTrigger) / 255.0),
    ] {
        if let Some(index) = axis_index(axis) {
            axes[index] = value;
        }
    }
    axes
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::Input::XboxController::XINPUT_GAMEPAD_BUTTON_FLAGS;

    /// 扳机走模拟量，肩键才是数字位
    #[test]
    fn maps_analog_triggers_and_shoulders() {
        let mut pad = XINPUT_GAMEPAD::default();
        pad.bLeftTrigger = 200;
        pad.bRightTrigger = 60;
        // LB（0x0100）与 A（0x1000）同时按下
        pad.wButtons = XINPUT_GAMEPAD_BUTTON_FLAGS(0x0100 | 0x1000);

        let buttons = buttons_of(&pad);
        let value = |button: Button| buttons[button_index(button).unwrap()];

        assert!((value(Button::LeftTrigger) - 200.0 / 255.0).abs() < 0.01);
        assert!((value(Button::RightTrigger) - 60.0 / 255.0).abs() < 0.01);
        // 肩键是纯数字位，不能和扳机混在一起
        assert_eq!(value(Button::LeftTrigger2), 1.0);
        assert_eq!(value(Button::RightTrigger2), 0.0);
        assert_eq!(value(Button::South), 1.0);
        assert_eq!(value(Button::East), 0.0);

        let axes = axes_of(&pad);
        let axis = |axis: Axis| axes[axis_index(axis).unwrap()];
        assert!((axis(Axis::LeftZ) - 200.0 / 255.0).abs() < 0.01);
        assert!((axis(Axis::RightZ) - 60.0 / 255.0).abs() < 0.01);
    }
}
