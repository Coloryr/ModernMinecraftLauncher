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
    // 默认映射里 X / north 是空格
    values.buttons[button_index(Button::North).unwrap()] = 1.0;

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
    // 默认已有 South → MouseLeft，再加一条 RightTrigger + South → F
    // 修饰键用 RT 而不是 LT：LT 自己也绑着鼠标左键，会把断言搅浑
    config.buttons.push(map::ButtonRule {
        button: Button::South,
        modifier: Some(Button::RightTrigger),
        action: String::from("F"),
        mode: map::ButtonMode::Normal,
    });
    let map = config.compile().unwrap();

    let mut values = empty_values();
    values.buttons[button_index(Button::South).unwrap()] = 1.0;

    // 只按 A：左键
    let plain = desired_actions(&map, &values, true);
    assert!(plain.contains(&Action::Mouse(0)));
    assert!(!plain.contains(&Action::Key(70)));

    // 按住 RT 再按 A：F 生效，左键被压住
    values.buttons[button_index(Button::RightTrigger).unwrap()] = 1.0;
    let combo = desired_actions(&map, &values, true);
    assert!(combo.contains(&Action::Key(70)));
    assert!(!combo.contains(&Action::Mouse(0)));
}

/// 左肩键映射成滚轮向上
#[test]
fn shoulder_maps_to_scroll() {
    let map = map::GamepadConfig::default().compile().unwrap();
    let mut values = empty_values();
    values.buttons[button_index(Button::LeftTrigger2).unwrap()] = 1.0;

    assert!(desired_actions(&map, &values, true).contains(&Action::Scroll(1)));
}

/// 组合规则命中后按书写顺序给出步骤，松开时反序
#[test]
fn combo_rule_keeps_order() {
    let mut config = map::GamepadConfig::default();
    // 只留这一条规则，免得默认绑定（LT 就是 MouseLeft）混进来
    config.buttons.clear();
    config.axes.clear();
    config.buttons.push(map::ButtonRule {
        button: Button::DPadLeft,
        modifier: Some(Button::LeftTrigger),
        action: String::from("LeftAlt+E"),
        mode: map::ButtonMode::Normal,
    });
    let map = config.compile().unwrap();

    let mut values = empty_values();
    values.buttons[button_index(Button::LeftTrigger).unwrap()] = 1.0;
    values.buttons[button_index(Button::DPadLeft).unwrap()] = 1.0;

    let rules = desired_rules(&map, &values, true);
    assert_eq!(rules.len(), 1);
    let combo = combo_of(&map, *rules.iter().next().unwrap()).unwrap();
    assert_eq!(
        combo.press().collect::<Vec<_>>(),
        vec![Action::Key(342), Action::Key(69)]
    );
    assert_eq!(
        combo.release().collect::<Vec<_>>(),
        vec![Action::Key(69), Action::Key(342)]
    );
}

/// 切换模式：按一次保持按下，再按一次松开
#[test]
fn toggle_mode_holds_until_next_press() {
    let mut config = map::GamepadConfig::default();
    config.buttons.clear();
    config.axes.clear();
    config.buttons.push(map::ButtonRule {
        button: Button::South,
        modifier: None,
        action: String::from("Space"),
        mode: ButtonMode::Toggle,
    });
    let map = config.compile().unwrap();
    let id = map.buttons[0].id;
    let mut relay = Relay::new();
    let mut down = BTreeSet::new();
    down.insert(id);

    // 第一帧按下：进入保持
    let frame = relay.step(&map, &map.buttons, Instant::now(), &down);
    assert!(frame.active.contains(&id));
    // 松开后仍然保持
    let frame = relay.step(&map, &map.buttons, Instant::now(), &BTreeSet::new());
    assert!(frame.active.contains(&id));
    // 再按一次才松开
    let frame = relay.step(&map, &map.buttons, Instant::now(), &down);
    assert!(!frame.active.contains(&id));
}

/// 循环按下：按住期间按间隔给出一次点击
#[test]
fn repeat_mode_pulses_on_interval() {
    let mut config = map::GamepadConfig::default();
    config.buttons.clear();
    config.axes.clear();
    config.buttons.push(map::ButtonRule {
        button: Button::South,
        modifier: None,
        action: String::from("Space"),
        mode: ButtonMode::Repeat,
    });
    let map = config.compile().unwrap();
    let id = map.buttons[0].id;
    let mut relay = Relay::new();
    let mut down = BTreeSet::new();
    down.insert(id);

    let start = Instant::now();
    assert_eq!(relay.step(&map, &map.buttons, start, &down).pulses, vec![id]);
    // 间隔没到不重复触发
    assert!(relay.step(&map, &map.buttons, start + Duration::from_millis(10), &down).pulses.is_empty());
    // 到了就再来一次
    assert_eq!(relay.step(&map, &map.buttons, start + REPEAT_INTERVAL, &down).pulses, vec![id]);
}

/// 鼠标模式与游戏内模式各用一套按钮绑定
#[test]
fn cursor_mode_uses_its_own_buttons() {
    let mut config = map::GamepadConfig::default();
    config.buttons.clear();
    config.axes.clear();
    config.buttons.push(map::ButtonRule {
        button: Button::South,
        modifier: None,
        action: String::from("Space"),
        mode: map::ButtonMode::Normal,
    });
    config.cursor_buttons = Some(vec![map::ButtonRule {
        button: Button::South,
        modifier: None,
        action: String::from("MouseLeft"),
        mode: map::ButtonMode::Normal,
    }]);
    let map = config.compile().unwrap();

    let mut values = empty_values();
    values.buttons[button_index(Button::South).unwrap()] = 1.0;

    assert_eq!(desired_actions(&map, &values, true), [Action::Key(32)].into());
    assert_eq!(desired_actions(&map, &values, false), [Action::Mouse(0)].into());
}

/// 换模式：先松开旧模式按住的动作，切换时还按着的按钮要等松开才生效
#[test]
fn mode_switch_releases_and_blocks_held_buttons() {
    let mut relay = Relay::new();
    relay.target = Some(Uuid::from_u128(9));
    relay.grabbed = Some(true);
    relay.counts.insert(Action::Key(32), 1);
    relay.active.insert(0);

    let south = button_index(Button::South).unwrap();
    let mut values = empty_values();
    values.buttons[south] = 1.0;

    relay.on_mode_change(false, &values);
    assert_eq!(relay.grabbed, Some(false));
    assert!(relay.counts.is_empty(), "切换模式要松开旧模式的动作");
    assert!(relay.active.is_empty());

    // 切换瞬间还按着：新模式下这颗按钮先不发
    assert_eq!(relay.mask_blocked(&values).buttons[south], 0.0);
    // 松开一次解除，再按才生效
    values.buttons[south] = 0.0;
    let _ = relay.mask_blocked(&values);
    values.buttons[south] = 1.0;
    assert_eq!(relay.mask_blocked(&values).buttons[south], 1.0);
}

/// 映射换了版本（改过配置）也要先松开：规则编号会整体重排，按旧编号解键会解错
#[test]
fn map_reload_releases_held_actions() {
    let mut relay = Relay::new();
    relay.generation = map::generation();
    relay.target = Some(Uuid::from_u128(9));
    relay.counts.insert(Action::Key(32), 1);
    relay.active.insert(0);

    relay.sync_generation();
    assert!(!relay.counts.is_empty(), "版本没变时不该动");

    map::bump_generation();
    relay.sync_generation();
    assert!(relay.counts.is_empty());
    assert!(relay.active.is_empty());
}

/// 关掉分离时两种模式共用游戏内那套
#[test]
fn shared_buttons_when_separation_off() {
    let mut config = map::GamepadConfig::default();
    config.separate_cursor = false;
    config.buttons.clear();
    config.axes.clear();
    config.buttons.push(map::ButtonRule {
        button: Button::South,
        modifier: None,
        action: String::from("Space"),
        mode: map::ButtonMode::Normal,
    });
    config.cursor_buttons = Some(vec![map::ButtonRule {
        button: Button::South,
        modifier: None,
        action: String::from("MouseLeft"),
        mode: map::ButtonMode::Normal,
    }]);
    let map = config.compile().unwrap();

    let mut values = empty_values();
    values.buttons[button_index(Button::South).unwrap()] = 1.0;

    assert_eq!(desired_actions(&map, &values, true), [Action::Key(32)].into());
    assert_eq!(desired_actions(&map, &values, false), [Action::Key(32)].into());
}
