//! 下载器离线集成测试
//!
//! 使用本机 `127.0.0.1` 上的临时 TCP HTTP 服务模拟下载源，
//! 不访问任何外部网络。覆盖以下离线逻辑：
//!
//! - 下载器初始化，`init` / `get_download_path`
//! - 临时文件生成，`gen_temp_file` 的 UUID 唯一且位于下载目录内
//! - 端到端下载流程：任务入队 → 工作线程下载 → 大小/哈希校验 → 移动到目标路径
//! - GUI 回调事件，AddTask / UpdateTask / RemoveTask
//! - 任务快照与取消接口，`get_tasks` / `cancel_task`
//!
//! 注意：进程级全局状态，日志/配置/GUI 回调/下载线程池，只能初始化一次，
//! 因此所有依赖初始化的用例集中在同一个测试函数内顺序执行。

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;

use mml_base::file_item::{FileHash, FileItemObj, LaterRun};
use mml_base::hash_helper::{self, HashType};
use mml_config::config_obj::ProxyState;
use mml_downloader::{DownloadTaskState, IDownloadGui, download_item::DownloadItem};

/// GUI 回调收到的事件列表
static GUI_EVENTS: OnceLock<Arc<Mutex<Vec<String>>>> = OnceLock::new();

/// 测试运行根目录
static RUN_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 记录事件的 GUI 回调实现，事件以字符串形式存入 `GUI_EVENTS`
struct GuiRecorder;

impl IDownloadGui for GuiRecorder {
    fn update(&self, thread: u32, file: &Arc<DownloadItem>) {
        let mut events = GUI_EVENTS.get().unwrap().lock().unwrap();
        events.push(format!(
            "file:{}:{}:{:.1}",
            thread,
            file.base.name,
            file.progress()
        ));
    }

    fn update_task(&self, state: DownloadTaskState) {
        let mut events = GUI_EVENTS.get().unwrap().lock().unwrap();
        match state {
            DownloadTaskState::AddTask(id) => events.push(format!("AddTask:{id}")),
            DownloadTaskState::RemoveTask(id) => events.push(format!("RemoveTask:{id}")),
            DownloadTaskState::UpdateTask(obj) => {
                events.push(format!("UpdateTask:{}:{:.0}", obj.id, obj.progress))
            }
        }
    }
}

/// 初始化下载器运行环境，进程内只执行一次
///
/// # 返回值
///
/// 返回运行根目录，重复调用返回同一目录
fn setup() -> PathBuf {
    if let Some(dir) = RUN_DIR.get() {
        return dir.clone();
    }

    let dir = mml_testutil::temp_dir().join(format!("mml-downloader-it-{}", uuid::Uuid::new_v4()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    RUN_DIR.set(dir.clone()).unwrap();

    GUI_EVENTS
        .set(Arc::new(Mutex::new(Vec::new())))
        .expect("GUI_EVENTS 只能初始化一次");

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
    mml_downloader::set_gui_handel(Box::new(GuiRecorder));
    mml_downloader::start();

    dir
}

/// 启动一个只响应单次 GET 请求的本地 HTTP 服务
///
/// - `body`: 响应体内容，Content-Length 按它的字节数写入
///
/// # 返回值
///
/// 返回监听端口号和服务线程句柄，测试结束后 join 以确认正常收尾
fn spawn_http_server(body: Vec<u8>) -> (u16, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定本机端口失败");
    let port = listener.local_addr().unwrap().port();

    let handle = std::thread::spawn(move || {
        // 只需处理一次请求
        if let Ok((mut stream, _)) = listener.accept() {
            read_request_head(&mut stream);
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
            let _ = stream.flush();
        }
        // 离开作用域时自动关闭监听器
    });

    (port, handle)
}

/// 读取并丢弃请求头，读到空行即止，避免客户端写阻塞
fn read_request_head(stream: &mut TcpStream) {
    let mut buf = [0u8; 1024];
    let mut total = 0;
    while total < buf.len() {
        match stream.read(&mut buf[total..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                total += n;
                // HTTP 头以空行 \r\n\r\n 结束
                if let Some(pos) = buf[..total].windows(4).position(|w| w == b"\r\n\r\n") {
                    let _ = pos;
                    break;
                }
            }
        }
    }
}

/// 端到端离线下载流程
#[tokio::test]
async fn offline_download_pipeline() {
    let dir = setup();

    // ---- gen_temp_file：路径唯一、位于下载目录内、文件不存在 ----
    let temp1 = mml_downloader::gen_temp_file();
    let temp2 = mml_downloader::gen_temp_file();
    assert_ne!(temp1, temp2, "临时文件路径应唯一");
    let download_dir = mml_downloader::get_download_path();
    assert!(temp1.starts_with(&download_dir));
    assert!(temp2.starts_with(&download_dir));
    assert!(!temp1.exists(), "生成的临时文件不应已存在");
    assert!(download_dir.exists(), "下载目录应已创建");

    // ---- 端到端下载，走本机 HTTP 服务 ----
    let body: Vec<u8> = b"hello mml downloader offline test body 1234567890".to_vec();
    let (port, server) = spawn_http_server(body.clone());

    // 期望哈希故意用大写，验证校验值大小写不敏感
    let sha256 = hash_helper::gen_hash(HashType::Sha256, &body).to_uppercase();

    let target = dir.join("target.bin");
    let item = FileItemObj {
        name: String::from("target.bin"),
        file: target.clone(),
        url: format!("http://127.0.0.1:{port}/target.bin"),
        hash: FileHash::Sha256(sha256),
        later: LaterRun::None,
    };

    let ok = mml_downloader::start_download_task(vec![item]).await;
    assert!(ok, "本机下载任务应全部成功");

    server.join().unwrap();

    // 文件内容与哈希一致，含大小写校验
    let downloaded = fs::read(&target).expect("下载后的文件应存在");
    assert_eq!(downloaded, body);

    // ---- 任务快照：完成后任务已移除 ----
    assert!(
        mml_downloader::get_tasks().is_empty(),
        "完成的任务应从队列移除"
    );

    // ---- GUI 回调事件 ----
    let events = GUI_EVENTS.get().unwrap().lock().unwrap().clone();
    let add_id = events.iter().find_map(|e| {
        e.strip_prefix("AddTask:")
            .and_then(|id| id.parse::<u64>().ok())
    });
    let add_id = add_id.expect("应收到 AddTask 事件");
    assert!(
        events.contains(&format!("RemoveTask:{add_id}")),
        "任务完成应收到 RemoveTask 事件，实际: {:?}",
        events
    );
    assert!(
        events.contains(&format!("UpdateTask:{add_id}:100")),
        "任务完成应收到 100% 进度事件，实际: {:?}",
        events
    );
    assert!(
        events
            .iter()
            .any(|e| e.starts_with("file:") && e.ends_with("target.bin:100.0")),
        "文件级进度应上报到 100%，实际: {:?}",
        events
    );

    // ---- 取消接口：不存在的任务返回 false ----
    assert!(!mml_downloader::cancel_task(0));
    assert!(!mml_downloader::cancel_task(u64::MAX));

    // ---- 清理，下载线程仍持有空闲句柄，尽力清理即可 ----
    let _ = fs::remove_file(&target);
    let _ = fs::remove_dir_all(&dir);
}
