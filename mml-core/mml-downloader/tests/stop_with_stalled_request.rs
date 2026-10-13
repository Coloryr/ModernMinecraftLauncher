//! 退出卡死复现：在途请求无人打断时，`stop()` 必须仍能立即返回
//!
//! 现场是「材质包下载中 → 关窗口 → 确认停止并退出」后界面卡住：`mml_core::stop()`
//! 里 `mml_downloader::stop()` 要 join 每个下载线程，而在途的 HTTP 请求不会因为
//! 取消令牌被放弃 —— 只能等服务端回包或读超时，退出就跟着一起卡住。
//!
//! 本用例不连外网：本机 TCP 服务只接受连接、不回任何字节，模拟"服务端不回包"。

use std::{
    fs,
    net::TcpListener,
    path::PathBuf,
    sync::{OnceLock, mpsc},
    time::Duration,
};

use mml_base::file_item::{FileHash, FileItemObj, LaterRun};
use mml_config::config_obj::ProxyState;

/// 测试运行根目录
static RUN_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 初始化下载器运行环境，进程内只执行一次
///
/// # 返回值
///
/// 返回运行根目录，重复调用返回同一目录
fn setup() -> PathBuf {
    if let Some(dir) = RUN_DIR.get() {
        return dir.clone();
    }

    let dir =
        mml_testutil::temp_dir().join(format!("mml-downloader-stop-{}", uuid::Uuid::new_v4()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    RUN_DIR.set(dir.clone()).unwrap();

    // 初始化全局依赖链，顺序与主程序一致
    mml_base::init(&dir);
    mml_log::start(&dir).expect("日志系统启动失败");
    mml_config::init(&dir).expect("配置系统初始化失败");

    // 显式禁用代理，保证请求直接命中本机模拟服务器
    {
        let mut config = mml_config::write_config();
        config.http.work_proxy = ProxyState::None;
        config.http.login_proxy = ProxyState::None;
    }

    mml_net::init();
    mml_downloader::init(&dir).expect("下载器初始化失败");
    mml_downloader::start();

    dir
}

/// 请求发出去之后服务端不回包时，`stop()` 应立即返回
#[test]
fn stop_returns_while_request_stalled() {
    let dir = setup();

    // 只接受连接、不回包的本机服务：连上就不管了，一直挂着直到收尾信号。
    // 刻意不读请求、不等对端关闭 —— 客户端丢下请求后不一定立刻断开连接（可能被连接池留下），
    // 等 EOF 收尾会把用例自己卡住
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定本机端口失败");
    let port = listener.local_addr().unwrap().port();
    let (accepted_tx, accepted_rx) = mpsc::channel();
    let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();
    let server = std::thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            let _ = accepted_tx.send(());
            let _ = shutdown_rx.recv_timeout(Duration::from_secs(30));
            drop(stream);
        }
    });

    // 后台发起下载：请求会一直挂住，任务不会自行结束
    let target = dir.join("stalled.bin");
    let item = FileItemObj {
        name: String::from("stalled.bin"),
        file: target.clone(),
        url: format!("http://127.0.0.1:{port}/stalled.bin"),
        hash: FileHash::None,
        later: LaterRun::None,
    };
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _ = rt.block_on(mml_downloader::start_download_task(vec![item]));
    });

    // 等到连接真的建立：此时下载线程一定卡在请求里
    accepted_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("下载线程应发出请求");

    // stop() 要立刻返回，不能陪在途请求一起等超时
    let (done_tx, done_rx) = mpsc::channel();
    std::thread::spawn(move || {
        mml_downloader::stop();
        let _ = done_tx.send(());
    });
    done_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("stop() 应立即返回：在途请求必须能被取消打断");

    let _ = shutdown_tx.send(());
    server.join().unwrap();
    let _ = fs::remove_dir_all(&dir);
}
