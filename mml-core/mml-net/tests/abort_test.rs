//! 全局中断的行为验证：改代理时中断所有在途请求
//!
//! 不依赖外网：本地起一个"只收不回"的 TCP 服务装作卡住的服务器，
//! 断言 `abort_all()` 能让卡住的请求**立刻**返回中断错误，
//! 且中断之后的新请求不受影响。

use std::env;
use std::io::Read;
use std::net::TcpListener;
use std::sync::{Once, mpsc};
use std::time::{Duration, Instant};

use mml_config::config_obj::ProxyState;

/// 初始化只做一次：两个用例在同一进程里跑，日志/配置都是进程级单例，
/// 重复 init 会让 mml_log 在打开同一个日志文件时 panic
static INIT: Once = Once::new();

/// 初始化日志、配置与 HTTP 客户端，进程内只跑一次
fn init() {
    INIT.call_once(|| {
        let exe_path = env::current_exe().expect("Failed to get exe path");
        let exe_dir = exe_path.parent().expect("Failed to get exe directory");
        let run_dir = exe_dir.parent().unwrap().to_path_buf();

        let _ = mml_log::start(&run_dir);
        let _ = mml_config::init(&run_dir);
        mml_net::init();
    });
}

/// 起一个"收下请求但永不响应"的服务，返回端口
fn silent_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定本地端口失败");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                // 挂着不回，直到客户端主动断开
                std::thread::sleep(Duration::from_secs(30));
            });
        }
    });
    port
}

/// 一次请求的观测结果
struct RequestOutcome {
    /// 从发起到拿到结果的耗时
    elapsed: Duration,
    /// 是否以错误结束
    is_err: bool,
    /// 结果详情：成功是 `Ok(n bytes)`，失败是错误调试文本
    detail: String,
}

/// 在独立线程 + 独立 runtime 里发一个请求，把结果回传
fn spawn_request(url: String) -> mpsc::Receiver<RequestOutcome> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let client = mml_net::Client::new(ProxyState::Auto);
            let started = Instant::now();
            let result = client.get_text(&url).await;
            let detail = match &result {
                Ok(text) => format!("Ok({} bytes)", text.len()),
                Err(err) => format!("{err:?}"),
            };
            let _ = tx.send(RequestOutcome {
                elapsed: started.elapsed(),
                is_err: result.is_err(),
                detail,
            });
        });
    });
    rx
}

/// 中断能让卡住的在途请求立刻返回中断错误
#[test]
fn abort_interrupts_inflight_request() {
    init();

    let port = silent_server();
    let url = format!("http://127.0.0.1:{port}/stuck");
    let rx = spawn_request(url);

    // 前提：请求确实卡住了，本地服务不回响应
    std::thread::sleep(Duration::from_millis(600));
    assert!(rx.try_recv().is_err(), "请求提前返回了，测试前提不成立");

    // 触发全局中断
    let abort_at = Instant::now();
    mml_net::abort_all();

    let RequestOutcome {
        elapsed,
        is_err,
        detail,
    } = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("abort 之后请求仍未返回");
    let lag = abort_at.elapsed();

    println!("请求总耗时 {elapsed:?}，abort 后 {lag:?} 返回；错误={detail}");
    assert!(is_err, "卡住的请求应该以错误结束");
    assert!(
        lag < Duration::from_millis(500),
        "中断不够快：abort 后 {lag:?} 才返回"
    );
    assert!(detail.contains("已中断"), "错误内容不是中断提示：{detail}");
}

/// 中断之后发出的新请求不受上一轮中断影响
#[test]
fn new_request_after_abort_unaffected() {
    init();

    mml_net::abort_all();

    // 打一个必然连不上的端口：只验证"新请求没被上一轮中断波及"
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let detail = rt.block_on(async {
        let client = mml_net::Client::new(ProxyState::Auto);
        match tokio::time::timeout(
            Duration::from_secs(5),
            client.get_text("http://127.0.0.1:9/nothing"),
        )
        .await
        {
            Ok(Ok(_)) => "Ok".to_string(),
            Ok(Err(err)) => format!("{err:?}"),
            Err(_) => "timeout".to_string(),
        }
    });

    println!("中断后新请求结果：{detail}");
    assert_ne!(detail, "timeout", "新请求不应卡住");
    assert!(
        !detail.contains("已中断"),
        "新请求被上一轮中断波及了：{detail}"
    );
}
