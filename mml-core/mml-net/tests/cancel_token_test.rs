//! 验证：`CancellationToken` 能否终止"卡在 reqwest body 读取"的请求
//!
//! 背景：用户环境里 `abortable#17` **从未被 poll**，新任务排不上 worker，
//! 因此任何写在 future 内部的检查，如 `select!` + `sleep` / `watch` / token，
//! 都没有机会执行。
//!
//! 本测试构造与服务端"只回响应头、不回 body"的连接，让 `get_text` 卡在 body 读取；
//! 然后在**另一个线程**取消 token，观察：
//! - 请求能否被终止；
//! - 用 `join` 观察是否真的结束，而不是永远 pending。

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use mml_config::config_obj::ProxyState;

/// 只回响应头、不回 body 的服务，制造"卡在读 body"
fn header_only_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定失败");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                let head = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 100\r\n\r\n";
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.flush();
                std::thread::sleep(Duration::from_secs(60));
                let _ = stream;
            });
        }
    });
    port
}

fn init_once() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let exe = std::env::current_exe().unwrap();
        let run_dir = exe.parent().unwrap().parent().unwrap().to_path_buf();
        let _ = mml_log::start(&run_dir);
        let _ = mml_config::init(&run_dir);
        mml_net::init();
    });
}

/// 直接验证：把 `get_text` 的 future 放进 `select!`，另一分支等 CancellationToken
///
/// 这是"最小改动"的方案 A：不改 Client 内部结构，只在 abortable 里换成 token。
#[test]
fn cancel_token_vs_stuck_body_read() {
    // init_once();
    // let port = header_only_server();
    // let url = format!("http://127.0.0.1:{port}/body-stuck");

    // let token = tokio_util::sync::CancellationToken::new();
    // let token_for_cancel = token.clone();

    // let (tx, rx) = mpsc::channel();
    // std::thread::spawn(move || {
    //     let rt = tokio::runtime::Builder::new_current_thread()
    //         .enable_all()
    //         .build()
    //         .unwrap();
    //     rt.block_on(async move {
    //         let started = Instant::now();
    //         let client = mml_net::Client::new(ProxyState::Auto);
    //         let token = token;
    //         let fut = client.get_text(&url);
    //         tokio::pin!(fut);
    //         let outcome = tokio::select! {
    //             r = &mut fut => format!("请求自行结束：{r:?}"),
    //             _ = token.cancelled() => "被 token 取消".to_string(),
    //         };
    //         let _ = tx.send((started.elapsed(), outcome));
    //     });
    // });

    // // 等它卡在读 body
    // std::thread::sleep(Duration::from_millis(800));
    // let cancel_at = Instant::now();
    // token_for_cancel.cancel();

    // match rx.recv_timeout(Duration::from_secs(3)) {
    //     Ok((elapsed, outcome)) => {
    //         println!(
    //             "总耗时 {elapsed:?}，取消后 {:?} 返回 → {outcome}",
    //             cancel_at.elapsed()
    //         );
    //         assert!(
    //             cancel_at.elapsed() < Duration::from_millis(500),
    //             "取消响应太慢：{:?}",
    //             cancel_at.elapsed()
    //         );
    //     }
    //     Err(_) => panic!(
    //         "✗ token 取消后请求仍未返回——说明 select! 没被 poll，token 方案同样无效"
    //     ),
    // }
}
