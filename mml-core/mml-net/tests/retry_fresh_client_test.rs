//! 验证「改代理后自动重试」：中断发生后，重跑必须真的用**新客户端**发出请求
//!
//! 用户实测现象：改了代理仍拉不到加载器列表。
//! 根因：六路查询在改代理**之前**就发出了，各自绑定"发出时那份客户端"；
//! `rebuild()` 只换 `RwLock` 里的指针，不影响在途请求。
//! 所以必须**中断后重新发起**才能走新代理。
//!
//! 本测试验证这个前提成立：
//! 1. 用打不通的客户端 A 发起请求 → 卡住；
//! 2. 中断；
//! 3. 用能打通的客户端 B 重新发起 → 必须成功。

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use mml_config::config_obj::{ProxyState, ProxyType};

/// 只收不回的"坏"服务，模拟打不通的代理或源
fn blackhole_server() -> u16 {
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

/// 正常回 200 的"好"服务，模拟新代理后能连通的源
fn good_server(hits: Arc<AtomicUsize>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定失败");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            hits.fetch_add(1, Ordering::SeqCst);
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                let body = "[1,2,3]";
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(resp.as_bytes());
                let _ = stream.flush();
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

#[test]
fn retry_after_abort_uses_fresh_client() {
    init_once();
    let bad_port = blackhole_server();
    let hits = Arc::new(AtomicUsize::new(0));
    let good_port = good_server(Arc::clone(&hits));

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let report = rt.block_on(async {
        // 第一次：用"坏"客户端，直连黑洞，发起后应该卡住
        let old_client = Arc::new(mml_net::Client::new_proxy(
            ProxyType::Http,
            &"127.0.0.1".to_string(),
            bad_port,
            &String::new(),
            &String::new(),
        ));
        let bad_url = format!("http://127.0.0.1:{bad_port}/stuck");

        let first = tokio::spawn({
            let c = Arc::clone(&old_client);
            let u = bad_url.clone();
            async move { c.get_text(&u).await }
        });

        // 等它卡住
        tokio::time::sleep(Duration::from_millis(400)).await;

        // 改代理 → 中断
        let abort_at = Instant::now();
        mml_net::abort_all();
        let first_res = first.await.unwrap();
        let aborted = first_res.as_ref().err().is_some_and(mml_net::is_aborted);
        let lag = abort_at.elapsed();

        // 第二次：用"新"客户端，直连好服务，重新发起必须成功
        let new_client = Arc::new(mml_net::Client::new(ProxyState::Auto));
        let good_url = format!("http://127.0.0.1:{good_port}/ok");
        let retry = new_client.get_text(&good_url).await;

        (aborted, lag, retry.is_ok(), format!("{retry:?}"))
    });

    let (aborted, lag, retry_ok, detail) = report;
    println!("第一次请求被中断：{aborted}（中断后 {lag:?} 返回）");
    println!("重试结果：ok={retry_ok} → {detail}");
    println!("新服务收到请求数：{}", hits.load(Ordering::SeqCst));

    assert!(aborted, "第一次请求应被中断");
    assert!(retry_ok, "重试必须成功——证明重跑会用新客户端");
    assert!(
        hits.load(Ordering::SeqCst) > 0,
        "新服务没收到请求，说明重试没真正发出去"
    );
    println!("\n✓ 中断后重新发起确实走新客户端（这是「改代理后自动重试」生效的前提）");
}
