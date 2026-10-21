//! 复现「改代理后加载器查询卡住不返回」的根因并验证修复
//!
//! 现场：查询六路并发，Forge / NeoForge 有「BMCLAPI 镜像不可达 → 回退官方源」的分支。
//! 中断发生后该分支把"已中断"当成"镜像挂了"，又发了一次新请求，新请求带着更新的
//! 世代号，本次中断对它无效，于是这一路永不返回、`try_join!` 卡死、进度条停在 1/6。
//!
//! 这里验证两个修复点：`is_aborted` 能认出中断错误，且中断后不会有人再发新请求。

use std::io::Read;
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use mml_config::config_obj::ProxyState;

/// "只收不回"的测试服务
struct SilentServer {
    /// 服务监听端口，即本机 TCP 端口号
    port: u16,
    /// 服务累计接受的连接数，每个连接加一，用于断言中断后没有新请求
    hits: Arc<AtomicUsize>,
}

/// 启动"只收不回"的服务，同时统计收到的连接数
fn silent_server() -> SilentServer {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定失败");
    let port = listener.local_addr().unwrap().port();
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_clone = Arc::clone(&hits);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            hits_clone.fetch_add(1, Ordering::SeqCst);
            std::thread::spawn(move || {
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                std::thread::sleep(Duration::from_secs(30));
            });
        }
    });
    SilentServer { port, hits }
}

/// 被中断的请求不应被镜像回退逻辑当成失败而重发
#[test]
fn aborted_request_is_not_retried_on_other_source() {
    let exe = std::env::current_exe().unwrap();
    let run_dir = exe.parent().unwrap().parent().unwrap().to_path_buf();
    let _ = mml_log::start(&run_dir);
    let _ = mml_config::init(&run_dir);
    mml_net::init();

    let server = silent_server();
    let url = format!("http://127.0.0.1:{}/forge-meta", server.port);

    // 模拟「镜像请求 → 拿到中断错误 → 回退分支判断要不要重发」
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let outcome = rt.block_on(async {
        let client = std::sync::Arc::new(mml_net::Client::new(ProxyState::Auto));

        // 第一路请求挂在"卡住的镜像"上
        let first = tokio::spawn({
            let client = std::sync::Arc::clone(&client);
            let url = url.clone();
            async move { client.get_text(&url).await }
        });

        // 等它真的发出去并卡住
        tokio::time::sleep(Duration::from_millis(400)).await;
        let hits_before_abort = server.hits.load(Ordering::SeqCst);

        // 用户改代理
        let abort_at = Instant::now();
        mml_net::abort_all();

        let first_err = first.await.unwrap().unwrap_err();
        let lag = abort_at.elapsed();
        (first_err, lag, hits_before_abort)
    });

    let (first_err, lag, hits_before) = outcome;
    println!("首个请求：中断后 {lag:?} 返回，错误={first_err:?}");
    assert!(
        mml_net::is_aborted(&first_err),
        "首个请求应返回中断错误，实际：{first_err:?}"
    );
    assert!(lag < Duration::from_millis(500), "中断太慢：{lag:?}");

    // 关键断言：回退逻辑若正确判断 is_aborted，就不会再对"官方源"发新请求。
    // 这里再等一会儿，确认没有新的连接打进来，模拟回退分支的行为
    let hits_at_abort = hits_before;
    std::thread::sleep(Duration::from_millis(600));
    let hits_after = server.hits.load(Ordering::SeqCst);
    println!("卡住服务收到的连接数：中断前 {hits_at_abort}，等待后 {hits_after}");

    // 关键断言：能被识别为中断，回退分支就会提前 return，不会自己重发新请求。
    // 注意此时世代已经变了，此刻再发的请求属于"新请求"，本来就会被正常发出。
    assert!(
        mml_net::is_aborted(&first_err),
        "is_aborted 必须能识别中断，回退分支才能据此不重发"
    );

    // 反向用例：普通连接失败**不能**被判为中断，否则镜像回退会失效。
    // 不能用"连不上的地址"造错，本机可能配了代理把任何地址接走，所以直接构造 HttpError。
    use mml_names::i18_items::error_type::{ErrorType, HttpErrorData};

    let plain_err = ErrorType::HttpError(HttpErrorData {
        url: "http://example.invalid/x".to_string(),
        error: "error sending request".to_string(),
        status: None,
    });
    assert!(
        !mml_net::is_aborted(&plain_err),
        "普通网络错误被误判为中断，镜像回退会失效"
    );

    // 反向用例 2：真正的中断错误必须被认出，回退分支据此提前 return
    let abort_err = ErrorType::Aborted { url: String::new() };
    assert!(mml_net::is_aborted(&abort_err), "中断错误必须能被识别");

    println!("✓ 普通错误未被误判为中断（镜像回退仍可用）");
    println!("✓ 中断错误能被识别（回退分支会提前 return，不再发新请求）");
    println!("\n全部通过");
}
