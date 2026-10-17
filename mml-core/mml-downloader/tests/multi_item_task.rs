//! 多文件一个下载任务的端到端测试（本机模拟 HTTP 服务，不访问外网）
//!
//! 为什么单独有这一份：下载器原有的离线测试**每个任务只放一个文件**，而「下载更新」是
//! **整批合成一个任务**（见 `resource_mods_update_apply`）。多文件任务的完成条件是
//! "每一项都被 `done` / `fail` / `mark_cancelled` 记过"（`DownloadTask::check_done`），
//! 只要有一项漏记，`wait_done` 就永远不会返回 —— 调用方跟着无限期挂住，界面表现为
//! 一直"下载中"。这条路径以前没人守，所以它必须有测试。
//!
//! 卡住的现场就在被测的那一句上，所以这里用 `recv_timeout` 把"永远不返回"变成**测试失败**，
//! 而不是让测试进程自己挂死。

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::Duration;

use mml_base::file_item::{FileHash, FileItemObj, LaterRun};
use mml_base::hash_helper::{self, HashType};
use mml_config::config_obj::ProxyState;

/// 用例之间必须串行：内核这一套是进程级单例，而 `cargo test` 默认多线程跑 `#[test]`，
/// 两个用例同时 `setup()` 会抢着初始化（`RUN_DIR.set` 会失败）。与 mml-game 的测试同一做法
static TEST_LOCK: Mutex<()> = Mutex::new(());

/// 初始化内核各系统，返回运行目录；重复调用返回同一目录
///
/// 日志 / 配置 / 下载线程池都是进程级单例，只能初始化一次，
/// 所以整个测试文件共用一份，顺序与主程序一致
///
/// # 返回值
///
/// 本次运行的临时运行目录
fn setup() -> PathBuf {
    static RUN_DIR: OnceLock<PathBuf> = OnceLock::new();
    if let Some(dir) = RUN_DIR.get() {
        return dir.clone();
    }

    let dir = mml_testutil::temp_dir().join(format!("mml-multi-item-{}", uuid::Uuid::new_v4()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    RUN_DIR.set(dir.clone()).unwrap();

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

/// 启动一个响应 `requests` 次 GET 的本地 HTTP 服务
///
/// 每次都用同一份响应体，够多文件任务把每个文件都下一遍
///
/// - `body`: 响应体内容
/// - `requests`: 最多响应多少次请求
///
/// # 返回值
///
/// 监听端口与服务线程句柄
fn spawn_http_server(body: Vec<u8>, requests: usize) -> (u16, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定本机端口失败");
    let port = listener.local_addr().unwrap().port();

    let handle = std::thread::spawn(move || {
        for _ in 0..requests {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            read_request_head(&mut stream);
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
            let _ = stream.flush();
        }
    });

    (port, handle)
}

/// 读取并丢弃请求头，读到空行即止，避免客户端写阻塞
///
/// - `stream`: 已接受的连接
fn read_request_head(stream: &mut TcpStream) {
    let mut buf = [0u8; 1024];
    let mut total = 0;
    while total < buf.len() {
        match stream.read(&mut buf[total..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                total += n;
                if let Some(pos) = buf[..total].windows(4).position(|w| w == b"\r\n\r\n") {
                    let _ = pos;
                    break;
                }
            }
        }
    }
}

/// 一个任务里放多个文件：必须在有限时间内全部下完
///
/// 任何一项漏记（panic 打死工作线程、提前 return 没记、写盘失败没兜住）都会让任务
/// 永远等不到"全部记完"，所以断言分两层：任务要能**返回**，且每个文件都真的落盘
#[test]
fn multi_item_task_completes() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = setup();
    const COUNT: usize = 5;

    let body: Vec<u8> = b"mml multi item payload 0123456789".to_vec();
    let sha = hash_helper::gen_hash(HashType::Sha256, &body);
    let (port, server) = spawn_http_server(body.clone(), COUNT);

    let mut items = Vec::new();
    for i in 0..COUNT {
        items.push(FileItemObj {
            name: format!("item-{i}.bin"),
            file: dir.join(format!("out/item-{i}.bin")),
            url: format!("http://127.0.0.1:{port}/item-{i}.bin"),
            hash: FileHash::Sha256(sha.clone()),
            later: LaterRun::None,
        });
    }

    // 下载要跑在自己的运行时上；放到独立线程里跑，主线程用 recv_timeout 等结果
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let ok = rt.block_on(mml_downloader::start_download_task(items));
        let _ = tx.send(ok);
    });

    let ok = match rx.recv_timeout(Duration::from_secs(60)) {
        Ok(ok) => ok,
        Err(_) => panic!("一个任务里 5 个文件 60 秒没结束：卡在等某一项被记完了"),
    };
    assert!(ok, "本机下载应全部成功");

    for i in 0..COUNT {
        let path = dir.join(format!("out/item-{i}.bin"));
        assert_eq!(fs::read(&path).expect("文件应已下好"), body, "第 {i} 个文件内容应一致");
    }

    server.join().unwrap();
}

/// `LaterRun::ReplaceOld`：新文件就位后，旧文件被挪走
///
/// 这是「下载更新」换旧模组的那一步。放在下载线程的后续操作里跑，才能"下完一个换一个"，
/// 而不是等整批下完之后再扫一遍 —— 也就不会出现新旧同名文件并存
#[test]
fn replace_old_runs_after_download() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = setup();

    let body: Vec<u8> = b"mml replace old payload".to_vec();
    let sha = hash_helper::gen_hash(HashType::Sha256, &body);
    let (port, server) = spawn_http_server(body.clone(), 1);

    let out_dir = dir.join("out3");
    fs::create_dir_all(&out_dir).unwrap();
    // 旧版本的文件；新文件与它不同名，走的就是"换"而不是"覆盖"
    let old_file = out_dir.join("old-version.jar");
    fs::write(&old_file, b"old jar content").unwrap();

    let item = FileItemObj {
        name: String::from("new-version.jar"),
        file: out_dir.join("new-version.jar"),
        url: format!("http://127.0.0.1:{port}/new-version.jar"),
        hash: FileHash::Sha256(sha),
        later: LaterRun::ReplaceOld(old_file.clone()),
    };

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let ok = rt.block_on(mml_downloader::start_download_task(vec![item]));
        let _ = tx.send(ok);
    });

    let ok = match rx.recv_timeout(Duration::from_secs(60)) {
        Ok(ok) => ok,
        Err(_) => panic!("带后续操作的下载 60 秒没结束"),
    };
    assert!(ok, "下载应成功");
    assert_eq!(
        fs::read(out_dir.join("new-version.jar")).expect("新文件应已下好"),
        body,
        "新文件应在最终路径上"
    );
    assert!(!old_file.exists(), "旧文件应已被换掉");

    server.join().unwrap();
}

/// 线上真正的形态：Modrinth 的 `Sha1Sha512` 双哈希 + 目标文件已经存在
///
/// 这两点才是「下载更新」实际会遇到的：CurseForge 用 `Sha1`、Modrinth 用 `Sha1Sha512`；
/// 而重试时目标路径上往往已经躺着上一次下好的那一份（上一轮卡住留下的）。
/// 单独钉一下，是因为"目标已存在"会走进另一条分支（先读盘校验再决定要不要重下）
#[test]
fn existing_target_with_dual_hash_completes() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = setup();
    const COUNT: usize = 3;

    let body: Vec<u8> = b"mml existing target payload abcdefghij".to_vec();
    let sha1 = hash_helper::gen_hash(HashType::Sha1, &body);
    let sha512 = hash_helper::gen_hash(HashType::Sha512, &body);
    let (port, server) = spawn_http_server(body.clone(), COUNT);

    let out_dir = dir.join("out2");
    fs::create_dir_all(&out_dir).unwrap();

    let mut items = Vec::new();
    for i in 0..COUNT {
        let file = out_dir.join(format!("item-{i}.jar"));
        // 目标先放一份内容不对的（等价于"上次卡住时留下的 / 旧的同名文件"）
        fs::write(&file, b"stale leftover").unwrap();
        items.push(FileItemObj {
            name: format!("item-{i}.jar"),
            file,
            url: format!("http://127.0.0.1:{port}/item-{i}.jar"),
            hash: FileHash::Sha1Sha512(sha1.clone(), sha512.clone()),
            later: LaterRun::None,
        });
    }

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let ok = rt.block_on(mml_downloader::start_download_task(items));
        let _ = tx.send(ok);
    });

    let ok = match rx.recv_timeout(Duration::from_secs(60)) {
        Ok(ok) => ok,
        Err(_) => panic!("目标已存在 + 双哈希时 60 秒没结束：卡在等某一项被记完了"),
    };
    assert!(ok, "目标已存在时也应覆盖成功");

    for i in 0..COUNT {
        let path = out_dir.join(format!("item-{i}.jar"));
        assert_eq!(fs::read(&path).expect("文件应已下好"), body, "第 {i} 个文件应被覆盖成新内容");
    }

    server.join().unwrap();
}
