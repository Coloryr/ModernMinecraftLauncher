use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};

use mml_game::game_lan::{GameLan, GameMotd};

static HAVE: AtomicBool = AtomicBool::new(false);

/// 收到局域网广播时打印内容并置位 HAVE，供主线程断言
///
/// - `motd`: 对端广播的游戏信息，`addr` 为 `None` 表示用发送方地址
fn read_motd(motd: &GameMotd) {
    println!(
        "motd:{} port:{} addr:{}",
        motd.motd,
        motd.port,
        motd.addr.unwrap().to_string()
    );

    HAVE.store(true, Ordering::Release);
}

/// 本机起一对收发端：服务端广播后客户端应能收到并解析出 MOTD
#[test]
fn test_lan() {
    let client = GameLan::new_client().unwrap();
    let server = GameLan::new_server().unwrap();

    client.add_event_handler(read_motd);
    client.start_read().unwrap();

    server
        .start_send(GameMotd {
            motd: "测试".to_string(),
            port: "456".to_string(),
            addr: None,
        })
        .unwrap();

    thread::sleep(Duration::from_secs(6));

    client.stop();
    server.stop();

    assert!(HAVE.load(Ordering::Acquire));
}
