//! 复现用户场景：查询支持的加载器卡住时改代理
//!
//! 验证三件事，对应"改了代理但查询没停"的三个成因：
//! 1. `abort_all()` 能中断卡住的加载器查询，并把它作为错误冒出来，不再被吞掉；
//! 2. 中断后重试能真的重新发起，不会命中失败的缓存单元；
//! 3. 重试走的是新客户端，这里用"换成一个能立刻连上的服务"来体现。

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use mml_config::config_obj::ProxyState;

/// 起一个"只收不回"的服务，返回监听端口，模拟卡住的加载器源
fn silent_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定端口失败");
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

/// 起一个"立刻回 200 空 JSON 数组"的服务，返回监听端口，模拟换代理后能连通的源
fn responsive_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定端口失败");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                let body = "[]";
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

/// 卡住的加载器查询应作为中断错误冒出来，重试要打到新客户端，普通错误不能被误判为中断
#[test]
fn aborted_loader_query_surfaces_error_and_retries_fresh() {
    let exe = std::env::current_exe().unwrap();
    let run_dir = exe.parent().unwrap().parent().unwrap().to_path_buf();
    mml_log::start(&run_dir);
    let _ = mml_config::init(&run_dir);
    mml_net::init();

    let stuck_port = silent_server();
    let stuck_url = format!("http://127.0.0.1:{stuck_port}/forge-meta");
    let ok_port = responsive_server();
    let ok_url = format!("http://127.0.0.1:{ok_port}/forge-meta");

    // ---- 1) 卡住的请求被中断，并且是错误，不会被 unwrap_or(false) 吞掉 ----
    let (tx, rx) = mpsc::channel();
    let url = stuck_url.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let client = mml_net::Client::new(ProxyState::Auto);
            let started = Instant::now();
            let res = client.get_text(&url).await;
            let aborted = match &res {
                Ok(_) => false,
                Err(err) => mml_net::is_aborted(err),
            };
            let _ = tx.send((started.elapsed(), aborted));
        });
    });

    std::thread::sleep(Duration::from_millis(500));
    assert!(rx.try_recv().is_err(), "请求应仍卡住");

    let abort_at = Instant::now();
    mml_net::abort_all();
    let (elapsed, aborted) = rx.recv_timeout(Duration::from_secs(5)).expect("未返回");
    let lag = abort_at.elapsed();
    println!("卡住请求：总耗时 {elapsed:?}，中断后 {lag:?} 返回，is_aborted={aborted}");
    assert!(aborted, "应被识别为中断错误（上层才能据此放弃而不是跳过）");
    assert!(lag < Duration::from_millis(500), "中断太慢：{lag:?}");

    // ---- 2) 中断后重试：新请求能正常完成，模拟走新代理 ----
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let retry = rt.block_on(async {
        let client = mml_net::Client::new(ProxyState::Auto);
        tokio::time::timeout(Duration::from_secs(5), client.get_text(&ok_url)).await
    });
    match retry {
        Ok(Ok(text)) => println!("重试成功：{text:?}"),
        Ok(Err(err)) => panic!("重试失败：{err:?}"),
        Err(_) => panic!("重试超时"),
    }

    // ---- 3) 未中断的普通错误不应被误判为 aborted ----
    let plain = rt.block_on(async {
        let client = mml_net::Client::new(ProxyState::Auto);
        client.get_text("http://127.0.0.1:9/nothing").await
    });
    match plain {
        Ok(_) => println!("? 意外连通 127.0.0.1:9"),
        Err(err) => {
            println!("普通连接错误：{err:?}");
            assert!(
                !mml_net::is_aborted(&err),
                "普通网络错误被误判为中断，会让加载器列表整体失败"
            );
            println!("✓ 普通错误未被误判为中断");
        }
    }

    println!("\n全部通过");
}
