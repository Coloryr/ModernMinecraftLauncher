//! 卡住的请求在 `abort_all()` 后一秒内全部以中断错误返回
//!
//! 用户环境里出现过"广播成功、接收者数=4、通道值已变，但请求侧一条唤醒日志都没有"，
//! 所以中断不能只靠通道通知。
//!
//! 本测试用一个"只收不回"的本地服务制造真正卡住的请求：服务端收下请求后睡 60 秒不回响应，
//! 三个请求各在独立的 current_thread runtime 里，中途调用 `abort_all()`，
//! 要求它们都在一秒内返回中断错误；唤醒要跨 runtime、跨线程送达。
//!
//! 注意：`abort_all()` 的世代通道广播与 `AbortHandle` 强制取消都能让本用例通过，
//! 所以它证明的是"卡住的请求能被整体中断"这个结果，区分不出自轮询定时器有没有生效
//! ——定时器只是通道通知丢失时的兜底，见 `mml-net/src/lib.rs` 的 `abortable`。

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
    println!(
        "中断后统一耗时：{:?}（自轮询间隔上限 100ms）",
        abort_at.elapsed()
    );
    assert!(ok, "所有卡住的请求都应被中断");
    assert!(
        abort_at.elapsed() < Duration::from_millis(1000),
        "自轮询响应不够快：{:?}",
        abort_at.elapsed()
    );
}
