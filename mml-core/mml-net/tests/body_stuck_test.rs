//! 复现用户日志里的**精确形态**：请求已拿到响应头，正卡在"读 body"
//!
//! 用户日志里多次出现：
//! ```text
//! [TEMP] get_text 头部已回 url=https://maven.minecraftforge.net/...，开始读 body 世代=0
//! [TEMP] abortable#8 开始等待：进入世代=0 通道当前值=0 线程=ThreadId(42)
//! (中断后 #8 再无任何输出)
//! ```
//!
//! 关键差别：**服务器先回响应头，含 Content-Length，然后不发送 body**。
//! 这样 `get_with_retry` 在头部阶段正常返回，`abortable` 在**读 body**阶段卡住。
//! 之前所有测试用的都是"连接后连头都不回"，走的不是同一条路径。

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use mml_config::config_obj::ProxyState;

/// "只回头、不回 body"的服务：声明 Content-Length 后挂住
fn header_only_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定失败");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                // 回响应头：声明 100 字节 body，但一个字节都不发
                let head = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 100\r\nConnection: keep-alive\r\n\r\n";
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.flush();
                // 挂住，不发 body
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

/// 验证服务器回了响应头却不发 body 时，卡在读 body 阶段的请求仍能被整体中断，
/// 并以中断错误返回，且中断到返回不超过 500ms
#[test]
fn abort_while_reading_body() {
    init_once();
    let port = header_only_server();

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let started = Instant::now();
            let client = mml_net::Client::new(ProxyState::Auto);
            let url = format!("http://127.0.0.1:{port}/body-stuck");
            let res = client.get_text(&url).await;
            let aborted = match &res {
                Ok(_) => false,
                Err(e) => mml_net::is_aborted(e),
            };
            let _ = tx.send((started.elapsed(), aborted, format!("{res:?}")));
        });
    });

    // 等它进入读 body 阶段
    std::thread::sleep(Duration::from_millis(800));

    let abort_at = Instant::now();
    std::thread::spawn(move || {
        mml_net::abort_all();
    })
    .join()
    .unwrap();

    match rx.recv_timeout(Duration::from_secs(5)) {
        Ok((elapsed, aborted, detail)) => {
            println!(
                "读 body 阶段中断：总耗时 {elapsed:?}，中断后 {:?} 返回，aborted={aborted}",
                abort_at.elapsed()
            );
            println!("结果：{detail}");
            assert!(aborted, "读 body 阶段也应能被中断");
            assert!(abort_at.elapsed() < Duration::from_millis(500), "中断太慢");
        }
        Err(_) => panic!("卡在读 body 的请求在中断后仍未返回 ✗ —— 复现了用户的问题"),
    }
}
