//! 复现生产环境的真实形态：四个请求同时卡在"读 body"，中断从**同步线程**发出
//!
//! 与旧测试的差别在分布方式：请求跑在 4 个独立线程加独立 current_thread runtime，
//! 中断从一个没有 runtime 上下文的线程发出，对应 Tauri 的同步命令。

use std::io::Read;
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use mml_config::config_obj::ProxyState;

/// "只收不回"的服务：响应头都不回，制造卡死
fn silent_server() -> u16 {
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

/// 初始化日志、配置与 HTTP 客户端，进程内只跑一次
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

/// 四个并发卡住的请求应被一次中断全部唤醒
#[test]
fn four_concurrent_stuck_requests_all_abort() {
    init_once();
    let port = silent_server();

    // 四个请求：各自独立线程加独立 current_thread runtime，模拟真实分布
    let mut receivers = Vec::new();
    for i in 0..4 {
        let (tx, rx) = mpsc::channel();
        receivers.push(rx);
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async move {
                let started = Instant::now();
                let client = mml_net::Client::new(ProxyState::Auto);
                let url = format!("http://127.0.0.1:{port}/stuck{i}");
                let res = client.get_text(&url).await;
                let aborted = match &res {
                    Ok(_) => false,
                    Err(e) => mml_net::is_aborted(e),
                };
                let _ = tx.send((started.elapsed(), aborted));
            });
        });
    }

    // 让四个请求都进入卡住状态
    std::thread::sleep(Duration::from_millis(600));

    // 从中断发起方：一个**没有 runtime 上下文**的普通线程，对应 Tauri 同步命令
    let abort_at = Instant::now();
    std::thread::spawn(move || {
        mml_net::abort_all();
    })
    .join()
    .unwrap();

    let mut all_ok = true;
    for (i, rx) in receivers.iter().enumerate() {
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok((elapsed, aborted)) => {
                println!("请求 {i}：总耗时 {elapsed:?}，aborted={aborted}");
                if !aborted {
                    all_ok = false;
                }
            }
            Err(_) => {
                println!("请求 {i}：中断后仍未返回 ✗");
                all_ok = false;
            }
        }
    }
    println!("中断后统一耗时：{:?}", abort_at.elapsed());
    assert!(all_ok, "四个卡住的请求应全部被中断返回");
}
