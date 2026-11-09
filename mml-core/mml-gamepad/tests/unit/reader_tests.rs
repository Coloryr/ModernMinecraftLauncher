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
    for (index, (button, mapped)) in SDL_BUTTONS.iter().enumerate() {
        assert!(!SDL_BUTTONS[..index].iter().any(|(item, _)| item == button));
        assert!(button_index(*mapped).is_some());
    }
    for (index, axis) in SDL_AXES.iter().enumerate() {
        assert!(!SDL_AXES[..index].contains(axis));
    }
    assert_eq!(BUTTONS.len(), BUTTON_COUNT);
    assert_eq!(AXES.len(), AXIS_COUNT);
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
    let mut pad = Pad::empty(String::from("test pad"), None);
    pad.set_button(SdlButton::South, true);
    pad.set_axis(SdlAxis::LeftX, -i16::MAX);

    let state = pad.snapshot(7);
    assert_eq!(state.id, 7);
    assert_eq!(state.name, "test pad");
    assert_eq!(state.buttons.len(), BUTTON_COUNT);
    assert_eq!(state.axes.len(), AXIS_COUNT);
    // 固定表里的按钮走 buttons，不进额外按键列表
    assert!(state.extras.is_empty());
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
        Some(-1.0)
    );
}

/// 固定表以外的按键按第一次出现的顺序往后追加，松开后仍留在列表里
#[test]
fn extras_append_in_first_seen_order() {
    let mut pad = Pad::empty(String::from("test pad"), None);
    // 初始松开事件不能让未用过的额外按钮提前出现在界面上。
    pad.set_button(SdlButton::LeftPaddle1, false);
    assert!(pad.extras.is_empty());
    pad.set_button(SdlButton::RightPaddle1, true);
    pad.set_button(SdlButton::LeftPaddle1, true);
    // 松开第一颗：值归零，但它仍在原来的位置
    pad.set_button(SdlButton::RightPaddle1, false);

    let state = pad.snapshot(1);
    let codes: Vec<u32> = state.extras.iter().map(|item| item.code).collect();
    assert_eq!(
        codes,
        vec![
            SdlButton::RightPaddle1 as u32,
            SdlButton::LeftPaddle1 as u32
        ]
    );
    assert_eq!(state.extras[0].value, 0.0);
    assert_eq!(state.extras[1].value, 1.0);
}

/// SDL 的屏幕坐标方向要转换成原有配置的向上为正，极值不得越界
#[test]
fn sticks_keep_existing_directions_and_ranges() {
    let mut pad = Pad::empty(String::from("test pad"), None);
    for (sdl, axis, sign) in [
        (SdlAxis::LeftX, Axis::LeftStickX, 1.0),
        (SdlAxis::LeftY, Axis::LeftStickY, -1.0),
        (SdlAxis::RightX, Axis::RightStickX, 1.0),
        (SdlAxis::RightY, Axis::RightStickY, -1.0),
    ] {
        for (raw, expected) in [
            (i16::MIN, -1.0),
            (0, 0.0),
            (i16::MAX, 1.0),
            (16384, 16384.0 / 32767.0),
        ] {
            pad.set_axis(sdl, raw);
            assert_eq!(pad.axes[axis_index(axis).unwrap()], expected * sign);
        }
    }
}

/// 肩键与模拟扳机不能互相覆盖，扳机释放必须同时清空按钮和轴
#[test]
fn triggers_update_both_axis_and_button() {
    let mut pad = Pad::empty(String::from("test pad"), None);
    for (sdl, axis, button, shoulder, shoulder_button) in [
        (
            SdlAxis::TriggerLeft,
            Axis::LeftZ,
            Button::LeftTrigger,
            SdlButton::LeftShoulder,
            Button::LeftTrigger2,
        ),
        (
            SdlAxis::TriggerRight,
            Axis::RightZ,
            Button::RightTrigger,
            SdlButton::RightShoulder,
            Button::RightTrigger2,
        ),
    ] {
        pad.set_button(shoulder, true);
        for (raw, expected) in [
            (i16::MAX, 1.0),
            (16384, 16384.0 / 32767.0),
            (0, 0.0),
            (i16::MIN, 0.0),
        ] {
            pad.set_axis(sdl, raw);
            assert_eq!(pad.axes[axis_index(axis).unwrap()], expected);
            assert_eq!(pad.buttons[button_index(button).unwrap()], expected);
            assert_eq!(pad.buttons[button_index(shoulder_button).unwrap()], 1.0);
        }
    }
}

/// 十字键要保留原来的轴绑定能力，松开一侧时仍反映另一侧的状态
#[test]
fn dpad_buttons_synthesize_axes() {
    let mut pad = Pad::empty(String::from("test pad"), None);
    for (positive, negative, axis) in [
        (SdlButton::DPadRight, SdlButton::DPadLeft, Axis::DPadX),
        (SdlButton::DPadUp, SdlButton::DPadDown, Axis::DPadY),
    ] {
        let index = axis_index(axis).unwrap();
        pad.set_button(positive, true);
        assert_eq!(pad.axes[index], 1.0);
        pad.set_button(negative, true);
        assert_eq!(pad.axes[index], 0.0);
        pad.set_button(positive, false);
        assert_eq!(pad.axes[index], -1.0);
        pad.set_button(negative, false);
        assert_eq!(pad.axes[index], 0.0);
    }
}
