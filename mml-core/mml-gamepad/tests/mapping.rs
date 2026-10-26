use mml_gamepad::{Action, Button, Mapper, Output, Settings, Snapshot};

fn pressed(action: Action, down: bool) -> Output {
    Output::Button { action, down }
}

#[test]
fn movement_deadzone_and_release_are_not_repeated() {
    let mut mapper = Mapper::default();
    let mut input = Snapshot {
        left_y: 0.1,
        ..Default::default()
    };
    let settings = Settings::default();
    assert!(mapper.update(&input, true, 0.016, &settings).is_empty());
    input.left_y = 1.0;
    assert_eq!(
        mapper.update(&input, true, 0.016, &settings),
        vec![pressed(Action::Key(87), true)]
    );
    assert!(mapper.update(&input, true, 0.016, &settings).is_empty());
    input.left_y = 0.0;
    assert_eq!(
        mapper.update(&input, true, 0.016, &settings),
        vec![pressed(Action::Key(87), false)]
    );
}

#[test]
fn capture_change_releases_game_key_without_clicking_menu() {
    let mut mapper = Mapper::default();
    let mut input = Snapshot::default();
    let settings = Settings::default();
    mapper.update(&input, true, 0.016, &settings);
    input.buttons.insert(Button::South);
    assert_eq!(
        mapper.update(&input, true, 0.016, &settings),
        vec![pressed(Action::Key(32), true)]
    );
    assert_eq!(
        mapper.update(&input, false, 0.016, &settings),
        vec![pressed(Action::Key(32), false)]
    );
    assert!(mapper.update(&input, false, 0.016, &settings).is_empty());
    input.buttons.clear();
    mapper.update(&input, false, 0.016, &settings);
    input.buttons.insert(Button::South);
    assert_eq!(
        mapper.update(&input, false, 0.016, &settings),
        vec![pressed(Action::Mouse(0), true)]
    );
    assert_eq!(mapper.reset(), vec![pressed(Action::Mouse(0), false)]);
    assert!(mapper.reset().is_empty());
}

#[test]
fn shoulder_scroll_is_edge_triggered_and_shared_keys_release_once() {
    let mut mapper = Mapper::default();
    let settings = Settings::default();
    let mut input = Snapshot::default();
    mapper.update(&input, false, 0.016, &settings);
    input
        .buttons
        .extend([Button::East, Button::Start, Button::RightShoulder]);
    assert_eq!(
        mapper.update(&input, false, 0.016, &settings),
        vec![pressed(Action::Key(256), true), Output::Scroll(-1)]
    );
    assert!(mapper.update(&input, false, 0.016, &settings).is_empty());
    input.buttons.remove(&Button::East);
    assert!(mapper.update(&input, false, 0.016, &settings).is_empty());
    input.buttons.clear();
    assert_eq!(
        mapper.update(&input, false, 0.016, &settings),
        vec![pressed(Action::Key(256), false)]
    );
}

#[test]
fn sensitivity_is_time_based_and_stalls_are_clamped() {
    let settings = Settings::default();
    let mut mapper = Mapper::default();
    let input = Snapshot {
        right_x: 1.0,
        right_y: 1.0,
        ..Default::default()
    };
    let frame = mapper.update(&input, true, 0.01, &settings);
    let Output::Move { dx, dy } = frame[0] else {
        panic!()
    };
    assert!(dx > 0.0 && dy < 0.0);
    assert!((dx.hypot(dy) - 9.0).abs() < 0.00001);
    let stalled = mapper.update(&input, true, 10.0, &settings);
    let Output::Move { dx, dy } = stalled[0] else {
        panic!()
    };
    assert!((dx.hypot(dy) - 45.0).abs() < 0.00001);
    let invalid = Snapshot {
        right_x: f64::NAN,
        ..input
    };
    assert!(
        mapper
            .update(&invalid, true, f64::INFINITY, &settings)
            .is_empty()
    );
}

#[test]
fn enable_with_held_trigger_does_not_attack_until_released() {
    let mut mapper = Mapper::default();
    let settings = Settings::default();
    let mut input = Snapshot::default();
    input.buttons.insert(Button::RightTrigger);
    assert!(mapper.update(&input, true, 0.01, &settings).is_empty());
    input.buttons.clear();
    mapper.update(&input, true, 0.01, &settings);
    input.buttons.insert(Button::RightTrigger);
    assert_eq!(
        mapper.update(&input, true, 0.01, &settings),
        vec![pressed(Action::Mouse(0), true)]
    );
    assert_eq!(mapper.reset(), vec![pressed(Action::Mouse(0), false)]);
}
