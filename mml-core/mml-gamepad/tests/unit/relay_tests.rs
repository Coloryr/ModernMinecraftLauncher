use super::*;
use crate::map::{Axis, Button};
use crate::reader::{AXIS_COUNT, BUTTON_COUNT, axis_index, button_index};

/// 空的手柄值
fn empty_values() -> PadValues {
    PadValues {
        buttons: [0.0; BUTTON_COUNT],
        axes: [0.0; AXIS_COUNT],
    }
}

/// 按钮按下映射成对应按键
#[test]
fn button_press_maps_to_key() {
    let map = map::GamepadConfig::default().compile().unwrap();
    let mut values = empty_values();
    values.buttons[button_index(Button::South).unwrap()] = 1.0;

    assert!(desired_actions(&map, &values, true).contains(&Action::Key(32)));
}

/// 光标模式下左摇杆不触发 WASD，转动模式下触发
#[test]
fn cursor_mode_skips_stick_keys() {
    let map = map::GamepadConfig::default().compile().unwrap();
    let mut values = empty_values();
    values.axes[axis_index(Axis::LeftStickY).unwrap()] = 1.0;

    // 向上推是 W，光标模式下不触发
    assert!(desired_actions(&map, &values, true).contains(&Action::Key(87)));
    assert!(!desired_actions(&map, &values, true).contains(&Action::Key(83)));
    assert!(!desired_actions(&map, &values, false).contains(&Action::Key(87)));
}

/// 左摇杆向下推是 S
#[test]
fn stick_down_maps_to_back() {
    let map = map::GamepadConfig::default().compile().unwrap();
    let mut values = empty_values();
    values.axes[axis_index(Axis::LeftStickY).unwrap()] = -1.0;

    let actions = desired_actions(&map, &values, true);
    assert!(actions.contains(&Action::Key(83)));
    assert!(!actions.contains(&Action::Key(87)));
}

/// 按住修饰键时组合键生效，同按钮的普通绑定被压住
#[test]
fn modifier_combo_overrides_plain_binding() {
    let mut config = map::GamepadConfig::default();
    // 默认已有 South → Space，再加一条 LeftTrigger + South → F
    config.buttons.push(map::ButtonRule {
        button: Button::South,
        modifier: Some(Button::LeftTrigger),
        action: String::from("F"),
    });
    let map = config.compile().unwrap();

    let mut values = empty_values();
    values.buttons[button_index(Button::South).unwrap()] = 1.0;

    // 只按 A：空格
    let plain = desired_actions(&map, &values, true);
    assert!(plain.contains(&Action::Key(32)));
    assert!(!plain.contains(&Action::Key(70)));

    // 按住 LT 再按 A：F 生效，空格被压住
    values.buttons[button_index(Button::LeftTrigger).unwrap()] = 1.0;
    let combo = desired_actions(&map, &values, true);
    assert!(combo.contains(&Action::Key(70)));
    assert!(!combo.contains(&Action::Key(32)));
}

/// 十字键上映射成滚轮向上
#[test]
fn dpad_maps_to_scroll() {
    let map = map::GamepadConfig::default().compile().unwrap();
    let mut values = empty_values();
    values.buttons[button_index(Button::DPadUp).unwrap()] = 1.0;

    assert!(desired_actions(&map, &values, true).contains(&Action::Scroll(1)));
}

