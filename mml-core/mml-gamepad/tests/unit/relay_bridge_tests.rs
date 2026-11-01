//! 真实 TCP 转发验证；直接提供设备快照，不启动驱动或发送系统输入。

use super::*;
use crate::map::{Axis, Button, GamepadConfig};
use crate::reader::{AXIS_COUNT, axis_index, button_index};
use std::{
    io::{Read, Write},
    net::TcpStream,
};

#[derive(Debug, PartialEq)]
enum Message {
    Key(i32, bool),
    Mouse(i32, bool),
    Scroll(i32),
    Move(f64, f64),
}

fn integer(stream: &mut TcpStream) -> i32 {
    let mut bytes = [0; 4];
    stream.read_exact(&mut bytes).unwrap();
    i32::from_be_bytes(bytes)
}

fn report(stream: &mut TcpStream, kind: i32, uuid: Uuid, grabbed: Option<bool>) {
    let id = uuid.to_string();
    stream.write_all(&kind.to_be_bytes()).unwrap();
    stream.write_all(&(id.len() as i32).to_be_bytes()).unwrap();
    stream.write_all(id.as_bytes()).unwrap();
    if let Some(value) = grabbed {
        stream.write_all(&[u8::from(value)]).unwrap();
    }
}

fn wait_state(uuid: Uuid, grabbed: bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while mml_ipc::get_mouse_state(uuid) != Some(grabbed) {
        assert!(Instant::now() < deadline, "game state report timed out");
        thread::sleep(Duration::from_millis(5));
    }
}

fn connect(port: u16, uuid: Uuid, grabbed: bool) -> TcpStream {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    assert_eq!(integer(&mut stream), 12);
    let length = usize::try_from(integer(&mut stream)).unwrap();
    assert!(length < 1024);
    let mut key = vec![0; length];
    stream.read_exact(&mut key).unwrap();
    assert_eq!(
        String::from_utf8(key).unwrap(),
        mml_ipc::get_auth_key().unwrap()
    );
    report(&mut stream, 9, uuid, None);
    report(&mut stream, 1, uuid, Some(grabbed));
    wait_state(uuid, grabbed);
    stream
}

fn receive(stream: &mut TcpStream) -> Message {
    match integer(stream) {
        5 => {
            let mut bytes = [0; 16];
            stream.read_exact(&mut bytes).unwrap();
            Message::Move(
                f64::from_be_bytes(bytes[..8].try_into().unwrap()),
                f64::from_be_bytes(bytes[8..].try_into().unwrap()),
            )
        }
        kind @ (6 | 7) => {
            let code = integer(stream);
            let mut down = [0];
            stream.read_exact(&mut down).unwrap();
            if kind == 7 {
                Message::Key(code, down[0] != 0)
            } else {
                Message::Mouse(code, down[0] != 0)
            }
        }
        8 => Message::Scroll(integer(stream)),
        kind => panic!("unexpected message type {kind}"),
    }
}

fn quiet(stream: &mut TcpStream) {
    stream
        .set_read_timeout(Some(Duration::from_millis(60)))
        .unwrap();
    let mut byte = [0];
    let error = stream.read(&mut byte).unwrap_err();
    assert!(matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    ));
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
}

fn frame(relay: &mut Relay, values: &PadValues, map: &CompiledMap) {
    relay.forward(target_instance(), Some(1), Some(values), map, 0.016);
}

#[test]
fn tcp_relay_releases_on_mode_device_target_changes_and_reconnect() {
    let log = mml_testutil::temp_path(format!("gamepad-bridge-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&log).unwrap();
    mml_log::start(log).unwrap();
    let port = mml_ipc::init(None).unwrap().unwrap();
    let uuid = Uuid::from_u128(1);
    let other_uuid = Uuid::from_u128(2);
    let mut stream = connect(port, uuid, true);
    let mut other = connect(port, other_uuid, false);
    assert!(mml_ipc::is_game_connected(uuid));
    assert_eq!(target_instance(), Some(uuid));
    let map = GamepadConfig::default().compile().unwrap();
    let mut relay = Relay::new();
    let mut values = PadValues {
        buttons: [0.0; BUTTON_COUNT],
        axes: [0.0; AXIS_COUNT],
    };
    frame(&mut relay, &values, &map);
    values.axes[axis_index(Axis::LeftStickY).unwrap()] = 1.0;
    values.axes[axis_index(Axis::RightStickX).unwrap()] = 1.0;
    for button in [Button::South, Button::LeftTrigger, Button::DPadUp] {
        values.buttons[button_index(button).unwrap()] = 1.0;
    }
    frame(&mut relay, &values, &map);
    let messages: Vec<_> = (0..5).map(|_| receive(&mut stream)).collect();
    for expected in [
        Message::Key(87, true),
        Message::Key(32, true),
        Message::Mouse(0, true),
        Message::Scroll(1),
        Message::Move(14.0, 0.0),
    ] {
        assert!(messages.contains(&expected), "{messages:?}");
    }
    frame(&mut relay, &values, &map);
    assert_eq!(receive(&mut stream), Message::Move(14.0, 0.0));
    quiet(&mut stream);
    quiet(&mut other);

    report(&mut stream, 1, uuid, Some(false));
    wait_state(uuid, false);
    values.axes.fill(0.0);
    frame(&mut relay, &values, &map);
    let released: Vec<_> = (0..3).map(|_| receive(&mut stream)).collect();
    for expected in [
        Message::Key(87, false),
        Message::Key(32, false),
        Message::Mouse(0, false),
    ] {
        assert!(released.contains(&expected), "{released:?}");
    }
    frame(&mut relay, &values, &map);
    quiet(&mut stream);
    values.buttons.fill(0.0);
    frame(&mut relay, &values, &map);
    values.buttons[button_index(Button::LeftTrigger).unwrap()] = 1.0;
    frame(&mut relay, &values, &map);
    assert_eq!(receive(&mut stream), Message::Mouse(0, true));

    relay.forward(Some(uuid), None, None, &map, 0.016);
    assert_eq!(receive(&mut stream), Message::Mouse(0, false));
    frame(&mut relay, &values, &map);
    quiet(&mut stream);
    values.buttons.fill(0.0);
    frame(&mut relay, &values, &map);
    values.buttons[button_index(Button::LeftTrigger).unwrap()] = 1.0;
    frame(&mut relay, &values, &map);
    assert_eq!(receive(&mut stream), Message::Mouse(0, true));

    drop(stream);
    let deadline = Instant::now() + Duration::from_secs(3);
    while mml_ipc::is_game_connected(uuid) {
        assert!(
            Instant::now() < deadline,
            "closed game channel still appears connected"
        );
        thread::sleep(Duration::from_millis(5));
    }
    assert!(!mml_ipc::send_mouse_button(uuid, 0, false));
    relay.forward(None, None, None, &map, 0.016);
    assert!(relay.held.is_empty());
    let mut stream = connect(port, uuid, true);
    values.axes[axis_index(Axis::LeftStickY).unwrap()] = 1.0;
    frame(&mut relay, &values, &map);
    assert_eq!(receive(&mut stream), Message::Key(87, true));
    quiet(&mut stream);
    values.axes.fill(0.0);
    values.buttons.fill(0.0);
    frame(&mut relay, &values, &map);
    assert_eq!(receive(&mut stream), Message::Key(87, false));
    values.buttons[button_index(Button::LeftTrigger).unwrap()] = 1.0;
    frame(&mut relay, &values, &map);
    assert_eq!(receive(&mut stream), Message::Mouse(0, true));
    quiet(&mut other);

    report(&mut other, 1, other_uuid, Some(true));
    wait_state(other_uuid, true);
    report(&mut stream, 1, uuid, Some(false));
    wait_state(uuid, false);
    assert_eq!(target_instance(), Some(other_uuid));
    frame(&mut relay, &values, &map);
    assert_eq!(receive(&mut stream), Message::Mouse(0, false));
    quiet(&mut other);
    values.buttons.fill(0.0);
    frame(&mut relay, &values, &map);
    values.buttons[button_index(Button::South).unwrap()] = 1.0;
    frame(&mut relay, &values, &map);
    assert_eq!(receive(&mut other), Message::Key(32, true));
    relay.release_all();
    assert_eq!(receive(&mut other), Message::Key(32, false));
    quiet(&mut stream);
    quiet(&mut other);
}
