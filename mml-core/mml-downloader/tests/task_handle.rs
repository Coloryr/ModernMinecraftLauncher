//! 下载任务句柄：查状态、取结果、取消
//!
//! 句柄是给"别处要显示进度 / 要能叫停"的调用方用的：拿到就能查，结束后还能读最后一组计数。
//! 与 download_offline 一样只打本机服务，不碰外网。

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use mml_base::file_item::{FileHash, FileItemObj, LaterRun};
use mml_config::config_obj::ProxyState;
use tokio_util::sync::CancellationToken;

/// 测试运行根目录
static RUN_DIR: OnceLock<PathBuf> = OnceLock::new();
/// 初始化闸门：两个用例并行跑，先到先建，后来的直接复用
static INIT: std::sync::Once = std::sync::Once::new();

/// 初始化下载器运行环境，进程内只执行一次
fn setup() -> PathBuf {
    INIT.call_once(|| {
    let dir = mml_testutil::temp_dir().join(format!("mml-dl-handle-{}", uuid::Uuid::new_v4()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    RUN_DIR.set(dir.clone()).unwrap();

    mml_base::init(&dir);
    mml_log::start(&dir).expect("日志系统启动失败");
    mml_config::init(&dir).expect("配置系统初始化失败");
    {
        let mut config = mml_config::write_config();
        config.http.work_proxy = ProxyState::None;
        config.http.login_proxy = ProxyState::None;
    }
    mml_net::init();
    mml_downloader::init(&dir).expect("下载器初始化失败");
    mml_downloader::start();
    });

    RUN_DIR.get().unwrap().clone()
}

/// 本机服务：每个连接都回同一份内容，`delay` 让"取消"那条用例有机会插进来
fn serve(body: Vec<u8>, delay: Duration) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定本机端口失败");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let body = body.clone();
            std::thread::spawn(move || {
                read_head(&mut stream);
                if !delay.is_zero() {
                    std::thread::sleep(delay);
                }
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&body);
                let _ = stream.flush();
            });
        }
    });
    port
}

/// 读到请求头结束就返回，避免客户端写阻塞
fn read_head(stream: &mut TcpStream) {
    let mut buf = [0u8; 1024];
    let mut total = 0;
    while total < buf.len() {
        match stream.read(&mut buf[total..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                total += n;
                if buf[..total].windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
        }
    }
}

/// 一条待下载的文件
fn item(dir: &Path, port: u16, name: &str) -> FileItemObj {
    FileItemObj {
        name: name.to_string(),
        file: dir.join(name),
        url: format!("http://127.0.0.1:{port}/{name}"),
        hash: FileHash::None,
        later: LaterRun::None,
    }
}

/// 拿句柄能查状态、能取结果；结束后状态仍读得到，任务也已从任务表里摘掉
#[tokio::test]
async fn handle_reports_status_and_result() {
    let dir = setup();
    let port = serve(b"handle body".to_vec(), Duration::ZERO);
    let items = vec![item(&dir, port, "h1.bin"), item(&dir, port, "h2.bin")];

    let handle = mml_downloader::start_download_task_handle(items, CancellationToken::new())
        .expect("下载器应已启动");
    assert!(handle.id() > 0, "句柄应带任务编号");
    assert_eq!(handle.status().total, 2);
    assert!(!handle.is_finished());

    let outcome = handle.wait().await;
    assert_eq!(outcome, mml_downloader::TaskOutcome::Done);
    assert!(handle.is_finished(), "任务结束后应从任务表里摘掉");
    assert_eq!(handle.status().completed, 2);
    assert_eq!(fs::read(dir.join("h1.bin")).unwrap(), b"handle body");
}

/// 句柄上取消：立刻返回取消结果，目标文件不留
#[tokio::test]
async fn handle_cancel_stops_task() {
    let dir = setup();
    // 响应慢 1.5 秒，保证取消时还在下
    let port = serve(b"slow body".to_vec(), Duration::from_millis(1500));
    let items = vec![item(&dir, port, "c1.bin")];

    let handle = mml_downloader::start_download_task_handle(items, CancellationToken::new())
        .expect("下载器应已启动");
    handle.cancel();

    let outcome = handle.wait().await;
    assert_eq!(outcome, mml_downloader::TaskOutcome::Cancelled);
    assert!(handle.is_finished());
    assert!(!dir.join("c1.bin").exists(), "取消后不应留下目标文件");
}
