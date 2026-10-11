//! 验证：`read_timeout` 对"服务端只回响应头、不回 body"是否真的生效
//!
//! 不能加全局 `timeout()`，它会让大文件下载被误杀，所以卡死只能靠 `read_timeout`
//! 即两次读取间隔兜底；本测试把服务器造成只回头不回 body，看请求多久失败。

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use mml_config::config_obj::ProxyState;

/// 只回响应头、不回 body：Content-Length 声明很大，实际一个字节不发
fn header_only_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定失败");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                let head = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 100000\r\n\r\n";
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.flush();
                // 挂着不发 body，但保持连接 60 秒
                std::thread::sleep(Duration::from_secs(60));
                let _ = stream;
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

/// 读 body 卡住时应在 read_timeout 到期后失败
#[test]
fn read_timeout_kills_body_stall() {
    init_once();
    let port = header_only_server();
    let url = format!("http://127.0.0.1:{port}/body-stall");

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let started = Instant::now();
            let client = mml_net::Client::new(ProxyState::Auto);
            let res = client.get_text(&url).await;
            let _ = tx.send((started.elapsed(), format!("{res:?}")));
        });
    });

    // read_timeout 配置是 10 秒，给足够余量观察
    match rx.recv_timeout(Duration::from_secs(20)) {
        Ok((elapsed, detail)) => {
            println!("读 body 卡住的请求：{elapsed:?} 后结束");
            println!("结果：{detail}");
            assert!(
                detail.contains("Err"),
                "应当以错误结束（超时），实际：{detail}"
            );
            println!(
                "→ read_timeout 生效（约 {:?} 后失败）",
                elapsed
            );
        }
        Err(_) => panic!(
            "✗ 20 秒内仍未结束：read_timeout 对'读 body 卡住'**没有生效**——需要另找兜底"
        ),
    }
}
