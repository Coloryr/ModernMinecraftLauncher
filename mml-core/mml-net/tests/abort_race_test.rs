//! 复现「中断信号丢失」竞态，并验证改用 `watch` 后不再丢
//!
//! 现场，来自用户日志：
//! ```text
//! [TEMP] get_text 头部已回 ...，开始读 body 世代=0
//! [TEMP] abortable 进入并已登记等待：进入世代=0     ← 旧实现自称"已登记"
//! 中断(世代 -> 1)
//! 中断(世代 -> 2)
//! (此后该请求永不返回)
//! ```
//!
//! 原因：旧实现用 `Notify`。`Notified` 是**惰性登记**的，第一次被 poll 才进等待队列，
//! 而 `notify_waiters()` **不保存许可**，只唤醒"当时已在队列里"的 waiter。
//! 于是"创建 Notified 之后、select! 首次 poll 之前"这个窗口里发生的中断会被直接丢弃。
//!
//! 本测试用**可控的时序**验证：在请求卡住期间、且确保中断先于某次等待建立，
//! 无论中断落在哪个时刻，卡住的请求都必须被唤醒并返回中断错误。

use std::io::Read;
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use mml_config::config_obj::ProxyState;

/// "只收不回"的服务：把请求挂住，制造"卡在等待中"的场景
fn silent_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定失败");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                std::thread::sleep(Duration::from_secs(30));
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

/// 卡住的请求必须在中断后返回中断错误；重复多次以覆盖不同时序
#[test]
fn abort_never_lost_regardless_of_timing() {
    init_once();
    let port = silent_server();
    let url = format!("http://127.0.0.1:{port}/stuck");

    for round in 0..5 {
        // 让每轮的"中断时刻"相对请求启动时间错开，覆盖竞态窗口
        let delay_ms = 20 + round * 60;

        let (tx, rx) = mpsc::channel();
        let url_clone = url.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async move {
                let started = Instant::now();
                let client = mml_net::Client::new(ProxyState::Auto);
                let res = client.get_text(&url_clone).await;
                let aborted = match &res {
                    Ok(_) => false,
                    Err(e) => mml_net::is_aborted(e),
                };
                let _ = tx.send((started.elapsed(), aborted));
            });
        });

        std::thread::sleep(Duration::from_millis(delay_ms));
        let abort_at = Instant::now();
        mml_net::abort_all();

        let (elapsed, aborted) = rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|_| panic!("第 {round} 轮：中断后请求仍未返回（信号丢失）"));
        let lag = abort_at.elapsed();
        println!("第 {round} 轮（中断于开始后 {delay_ms}ms）：总耗时 {elapsed:?}，中断后 {lag:?} 返回，aborted={aborted}");

        assert!(aborted, "第 {round} 轮：应返回中断错误");
        assert!(
            lag < Duration::from_millis(500),
            "第 {round} 轮：中断太慢（{lag:?}）"
        );
    }

    println!("\n所有时序下中断均未丢失");
}
