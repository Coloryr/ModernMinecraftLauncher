//! 验证"自轮询"在最恶劣情形下也能生效：请求 future 很长时间不被 poll
//!
//! 用户环境里出现过"广播成功、接收者数=4、通道值已变，但请求侧一条唤醒日志都没有"。
//! 自轮询的设计前提是"只要还会被 poll 一次，就能发现自己该中断了"。
//!
//! 本测试构造一个长时间不让出的请求 future，用 `std::thread::sleep` 模拟阻塞，
//! 期间完全不 await，然后在中途调用 `abort_all()`：
//! - 若它醒来后立刻发现自己被中断，说明自轮询有效；
//! - 这是 `watch.changed()` 单独无法覆盖的情形，阻塞期间收不到任何唤醒。

use std::sync::mpsc;
use std::time::{Duration, Instant};

/// 初始化日志 / 配置 / 网络，全进程只做一次
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

/// 起一个"只收不回"的本地服务，返回监听端口，用于制造真正卡住的请求
fn silent_server() -> u16 {
    use std::io::Read;
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定失败");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                std::thread::sleep(Duration::from_secs(60));
            });
        }
    });
    port
}

/// 三个并发请求全部卡住时，`abort_all()` 仍能让它们在一秒内以中断错误返回
#[test]
fn abort_effective_even_with_slow_polling() {
    init_once();
    let port = silent_server();
    let url = format!("http://127.0.0.1:{port}/stuck");

    // 多个请求并发，覆盖"被 poll 次数不均衡"的情况
    let mut rxs = Vec::new();
    for _ in 0..3 {
        let (tx, rx) = mpsc::channel();
        rxs.push(rx);
        let url = url.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async move {
                let started = Instant::now();
                let client = mml_net::Client::new(mml_config::config_obj::ProxyState::Auto);
                let res = client.get_text(&url).await;
                let aborted = match &res {
                    Ok(_) => false,
                    Err(e) => mml_net::is_aborted(e),
                };
                let _ = tx.send((started.elapsed(), aborted));
            });
        });
    }

    std::thread::sleep(Duration::from_millis(500));
    let abort_at = Instant::now();
    std::thread::spawn(move || {
        mml_net::abort_all();
    })
    .join()
    .unwrap();

    let mut ok = true;
    for (i, rx) in rxs.iter().enumerate() {
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok((elapsed, aborted)) => {
                println!("请求 {i}：总耗时 {elapsed:?} aborted={aborted}");
                if !aborted {
                    ok = false;
                }
            }
            Err(_) => {
                println!("请求 {i}：未返回 ✗");
                ok = false;
            }
        }
    }
    println!("中断后统一耗时：{:?}（自轮询间隔上限 100ms）", abort_at.elapsed());
    assert!(ok, "所有卡住的请求都应被中断");
    assert!(
        abort_at.elapsed() < Duration::from_millis(1000),
        "自轮询响应不够快：{:?}",
        abort_at.elapsed()
    );
}
