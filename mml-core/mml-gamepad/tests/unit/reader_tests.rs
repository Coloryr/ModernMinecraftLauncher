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
    let mut pad = Pad::empty(String::from("test pad"), None);
    pad.set_button(GilrsButton::South, 0x130, 1.0);
    pad.set_axis(GilrsAxis::LeftStickX, -0.5);

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
        Some(-0.5)
    );
}

/// 固定表以外的按键按第一次出现的顺序往后追加，松开后仍留在列表里
#[test]
fn extras_append_in_first_seen_order() {
    let mut pad = Pad::empty(String::from("test pad"), None);
    pad.set_button(GilrsButton::Unknown, 0x130, 1.0);
    pad.set_button(GilrsButton::Unknown, 0x131, 1.0);
    // 松开第一颗：值归零，但它仍在原来的位置
    pad.set_button(GilrsButton::Unknown, 0x130, 0.0);

    let state = pad.snapshot(1);
    let codes: Vec<u32> = state.extras.iter().map(|item| item.code).collect();
    assert_eq!(codes, vec![0x130, 0x131]);
    assert_eq!(state.extras[0].value, 0.0);
    assert_eq!(state.extras[1].value, 1.0);
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

