//! 实验，修正版：卡死请求是否挤占调度，以及 abort() 能否恢复
//!
//! 上一版缺陷：用 current_thread runtime 却只在主线程 sleep，没人驱动 runtime，
//! 于是"0 个被 poll"是测试自身的问题，不能当结论。本版用 rt.block_on 真正驱动。

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// 只回响应头、不回 body 的服务，制造"卡在读 body"
fn header_only_server(hits: Arc<AtomicUsize>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定失败");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            hits.fetch_add(1, Ordering::SeqCst);
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                let head = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 100000\r\n\r\n";
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.flush();
                std::thread::sleep(Duration::from_secs(120));
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

/// 在真正被 block_on 驱动的 runtime 上验证：卡在读 body 的请求会占住调度，让后续任务排不上，
/// 而对这些请求 abort 之后调度能恢复
#[test]
fn stuck_requests_and_abort_under_driven_runtime() {
    init_once();
    let hits = Arc::new(AtomicUsize::new(0));
    let port = header_only_server(Arc::clone(&hits));

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let entered = Arc::new(AtomicUsize::new(0));
    let marker_polled = Arc::new(AtomicUsize::new(0));
    let after_polled = Arc::new(AtomicUsize::new(0));

    let entered_c = Arc::clone(&entered);
    let marker_c = Arc::clone(&marker_polled);
    let after_c = Arc::clone(&after_polled);

    let report = rt.block_on(async move {
        let mut handles = Vec::new();

        for i in 0..4 {
            let url = format!("http://127.0.0.1:{port}/stuck{i}");
            let entered = Arc::clone(&entered_c);
            let client = Arc::new(mml_net::Client::new(
                mml_config::config_obj::ProxyState::Auto,
            ));
            let h = tokio::spawn(async move {
                entered.fetch_add(1, Ordering::SeqCst);
                let _ = client.get_text(&url).await;
            });
            handles.push(h.abort_handle());
        }

        tokio::time::sleep(Duration::from_millis(800)).await;

        tokio::spawn({
            let m = Arc::clone(&marker_c);
            async move {
                m.fetch_add(1, Ordering::SeqCst);
            }
        });
        tokio::time::sleep(Duration::from_millis(200)).await;
        let marker_ran = marker_polled.load(Ordering::SeqCst);
        let entered_now = entered.load(Ordering::SeqCst);

        let abort_at = Instant::now();
        for h in &handles {
            h.abort();
        }

        tokio::spawn({
            let a = Arc::clone(&after_c);
            async move {
                a.fetch_add(1, Ordering::SeqCst);
            }
        });
        tokio::time::sleep(Duration::from_millis(200)).await;
        let recovered = after_polled.load(Ordering::SeqCst);

        (entered_now, marker_ran, recovered, abort_at.elapsed())
    });

    let (entered_now, marker_ran, recovered, abort_lag) = report;
    println!("服务端收到的连接数：{}", hits.load(Ordering::SeqCst));
    println!("进入 await 的请求数：{entered_now}");
    println!("卡死期间标记任务被 poll：{marker_ran}");
    println!("abort 后新任务被 poll：{recovered}（abort 耗时 {abort_lag:?}）");

    println!("\n--- 结论 ---");
    if marker_ran == 0 {
        println!("复现：卡死请求占住 runtime，后续任务排不上（对应 #17 从未被 poll）");
    } else {
        println!("未复现：卡死期间标记任务仍被调度");
    }
    if recovered == 1 {
        println!("abort() 有效：调度恢复");
    } else {
        println!("abort() 后仍无法调度");
    }
}