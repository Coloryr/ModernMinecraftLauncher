//! 通过真实 TCP 验证映射、抓取切换、断线重连与按键释放，不发送系统输入。

use mml_gamepad::{Action, Button, Mapper, Output, Settings, Snapshot};
use std::{
    io::{Read, Write},
    net::TcpStream,
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

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

fn connect(port: u16, uuid: Uuid) -> TcpStream {
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
    report(&mut stream, 1, uuid, Some(true));
    wait_state(uuid, true);
    stream
}

fn wait_state(uuid: Uuid, grabbed: bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while mml_ipc::get_mouse_state(uuid) != Some(grabbed) {
        assert!(Instant::now() < deadline, "game state report timed out");
        thread::sleep(Duration::from_millis(5));
    }
}

fn receive(stream: &mut TcpStream) -> Output {
    match integer(stream) {
        5 => {
            let mut bytes = [0; 16];
            stream.read_exact(&mut bytes).unwrap();
            Output::Move {
                dx: f64::from_be_bytes(bytes[..8].try_into().unwrap()),
                dy: f64::from_be_bytes(bytes[8..].try_into().unwrap()),
            }
        }
        kind @ (6 | 7) => {
            let code = integer(stream);
            let mut down = [0];
            stream.read_exact(&mut down).unwrap();
            Output::Button {
                action: if kind == 7 {
                    Action::Key(code)
                } else {
                    Action::Mouse(code)
                },
                down: down[0] != 0,
            }
        }
        8 => Output::Scroll(integer(stream)),
        kind => panic!("unexpected message type {kind}"),
    }
}

fn send_and_check(stream: &mut TcpStream, uuid: Uuid, messages: Vec<Output>) {
    for output in messages {
        assert!(output.send(uuid));
        assert_eq!(receive(stream), output);
    }
}

#[test]
fn mapped_input_stays_isolated_and_releases_on_mode_change_and_reconnect() {
    let log = mml_testutil::temp_path(format!("gamepad-bridge-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&log).unwrap();
    mml_log::start(log).unwrap();
    let port = mml_ipc::init(None).unwrap().unwrap();
    let uuid = Uuid::new_v4();
    let mut stream = connect(port, uuid);
    assert!(mml_ipc::is_game_connected(uuid));
    let mut other = connect(port, Uuid::new_v4());
    let settings = Settings::default();
    let mut mapper = Mapper::default();
    let mut input = Snapshot::default();
    assert!(mapper.update(&input, true, 0.016, &settings).is_empty());
    input.left_y = 1.0;
    input.right_x = 1.0;
    input
        .buttons
        .extend([Button::South, Button::RightTrigger, Button::LeftShoulder]);
    let messages = mapper.update(&input, true, 0.016, &settings);
    assert!(messages.contains(&Output::Button {
        action: Action::Key(87),
        down: true
    }));
    assert!(messages.contains(&Output::Button {
        action: Action::Key(32),
        down: true
    }));
    assert!(messages.contains(&Output::Button {
        action: Action::Mouse(0),
        down: true
    }));
    assert!(messages.contains(&Output::Scroll(1)));
    assert!(
        messages
            .iter()
            .any(|output| matches!(output, Output::Move { dx, .. } if *dx > 0.0))
    );
    send_and_check(&mut stream, uuid, messages);

    report(&mut stream, 1, uuid, Some(false));
    wait_state(uuid, false);
    input.left_y = 0.0;
    let released = mapper.update(
        &input,
        mml_ipc::get_mouse_state(uuid).unwrap(),
        0.016,
        &settings,
    );
    assert_eq!(released.len(), 3);
    assert!(
        released
            .iter()
            .all(|output| matches!(output, Output::Button { down: false, .. }))
    );
    send_and_check(&mut stream, uuid, released);
    input.buttons.clear();
    mapper.update(&input, false, 0.016, &settings);
    input.buttons.insert(Button::South);
    send_and_check(
        &mut stream,
        uuid,
        mapper.update(&input, false, 0.016, &settings),
    );
    let released = mapper.reset();
    assert_eq!(
        released,
        vec![Output::Button {
            action: Action::Mouse(0),
            down: false
        }]
    );
    send_and_check(&mut stream, uuid, released);
    assert!(mapper.reset().is_empty());
    assert!(!Output::Scroll(1).send(Uuid::new_v4()));

    report(&mut stream, 1, uuid, Some(true));
    wait_state(uuid, true);
    input = Snapshot::default();
    assert!(mapper.update(&input, true, 0.016, &settings).is_empty());
    input.left_y = 1.0;
    input.buttons.insert(Button::RightTrigger);
    let held = mapper.update(&input, true, 0.016, &settings);
    assert_eq!(held.len(), 2);
    send_and_check(&mut stream, uuid, held);
    drop(stream);
    let deadline = Instant::now() + Duration::from_secs(3);
    while mml_ipc::is_game_connected(uuid) {
        assert!(
            Instant::now() < deadline,
            "closed game channel still appears connected"
        );
        thread::sleep(Duration::from_millis(5));
    }
    assert!(!Output::Scroll(1).send(uuid));
    let released = mapper.reset();
    assert_eq!(released.len(), 2);
    for output in released {
        assert!(matches!(output, Output::Button { down: false, .. }));
        assert!(!output.send(uuid));
    }

    let mut reconnected = connect(port, uuid);
    assert!(mml_ipc::is_game_connected(uuid));
    // 重连时重发方向键，仍按着的扳机须松开后再攻击。
    let resumed = mapper.update(&input, true, 0.016, &settings);
    assert_eq!(
        resumed,
        vec![Output::Button {
            action: Action::Key(87),
            down: true
        }]
    );
    send_and_check(&mut reconnected, uuid, resumed);
    input = Snapshot::default();
    send_and_check(
        &mut reconnected,
        uuid,
        mapper.update(&input, true, 0.016, &settings),
    );
    input.buttons.insert(Button::RightTrigger);
    let attack = mapper.update(&input, true, 0.016, &settings);
    assert_eq!(
        attack,
        vec![Output::Button {
            action: Action::Mouse(0),
            down: true
        }]
    );
    send_and_check(&mut reconnected, uuid, attack);
    send_and_check(&mut reconnected, uuid, mapper.reset());
    assert!(mapper.reset().is_empty());

    other
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    let mut byte = [0];
    let error = other.read(&mut byte).unwrap_err();
    assert!(matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    ));
}
