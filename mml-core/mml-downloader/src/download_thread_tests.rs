//! 用本机 HTTP 响应验证下载重试和终态，不启动下载线程池。

use super::*;
use mml_base::file_item::FileItemObj;
use std::{
    fs,
    net::TcpListener,
    sync::Once,
    time::{Duration, Instant},
};

fn setup() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let root = crate::test_util::ensure_env();
        {
            let mut config = mml_config::write_config();
            config.http.work_proxy = mml_config::config_obj::ProxyState::None;
            config.http.check_file = true;
        }
        mml_net::init();
        crate::init(&root).unwrap();
    });
}

fn server(responses: Vec<String>) -> (String, JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}/file", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let mut requests = Vec::new();
        for response in responses {
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut stream = loop {
                if let Ok((stream, _)) = listener.accept() {
                    break stream;
                }
                assert!(Instant::now() < deadline, "下载器没有发出预期请求");
                thread::sleep(Duration::from_millis(10));
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut head = Vec::new();
            while !head.ends_with(b"\r\n\r\n") {
                let mut byte = [0u8];
                stream.read_exact(&mut byte).unwrap();
                head.push(byte[0]);
            }
            requests.push(String::from_utf8(head).unwrap().to_lowercase());
            stream.write_all(response.as_bytes()).unwrap();
        }
        requests
    });
    (url, handle)
}

fn run_download(
    url: String,
    name: &str,
    hash: FileHash,
    later: LaterRun,
) -> (
    PathBuf,
    Arc<DownloadTask>,
    Arc<crate::download_item::DownloadItem>,
) {
    let target = crate::test_util::make_temp_dir(name).join("download.bin");
    let task = Arc::new(DownloadTask::new(vec![FileItemObj {
        name: name.into(),
        file: target.clone(),
        url,
        hash,
        later,
    }]));
    let item = task.get_item().unwrap();
    download(
        0,
        DownloadObj {
            task: task.clone(),
            item: item.clone(),
        },
    );
    (target, task, item)
}

#[test]
fn resume_keeps_prefix_and_restarts_when_range_is_ignored() {
    setup();
    for resumed in [
        "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 3-9/10\r\nContent-Length: 7\r\nConnection: close\r\n\r\ndefghij",
        "HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nabcdefghij",
    ] {
        let (url, handle) = server(vec![
            "HTTP/1.1 200 OK\r\nAccept-Ranges: bytes\r\nContent-Length: 10\r\nConnection: close\r\n\r\nabc".into(),
            resumed.into(),
        ]);
        let hash = FileHash::Sha256(hash_helper::gen_hash(HashType::Sha256, b"abcdefghij"));
        let (target, task, item) = run_download(url, "resume", hash, LaterRun::None);
        let requests = handle.join().unwrap();
        assert!(requests[1].contains("range: bytes=3-"));
        assert_eq!(item.get_state(), DownloadItemState::Done);
        assert_eq!(task.completed_count.load(Ordering::SeqCst), 1);
        assert_eq!(fs::read(target).unwrap(), b"abcdefghij");
    }
}

#[test]
fn resume_rejects_wrong_range() {
    setup();
    let (url, handle) = server(vec![
        "HTTP/1.1 200 OK\r\nAccept-Ranges: bytes\r\nContent-Length: 10\r\nConnection: close\r\n\r\nabc".into(),
        "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 2-9/10\r\nContent-Length: 8\r\nConnection: close\r\n\r\ncdefghij".into(),
        "HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nabcdefghij".into(),
    ]);
    let (target, _, item) = run_download(url, "wrong-range", FileHash::None, LaterRun::None);
    let requests = handle.join().unwrap();
    assert!(requests[1].contains("range: bytes=3-"));
    assert!(!requests[2].contains("range:"));
    assert_eq!(item.get_state(), DownloadItemState::Done);
    assert_eq!(fs::read(target).unwrap(), b"abcdefghij");
}

#[test]
fn http_error_bodies_never_complete_a_download() {
    setup();
    for status in ["404 Not Found", "500 Internal Server Error"] {
        let response =
            format!("HTTP/1.1 {status}\r\nContent-Length: 5\r\nConnection: close\r\n\r\nerror");
        let (url, handle) = server(vec![response; 6]);
        let (target, task, item) = run_download(url, "http-error", FileHash::None, LaterRun::None);
        assert_eq!(handle.join().unwrap().len(), 6);
        assert_eq!(item.get_state(), DownloadItemState::Error);
        assert_eq!(task.failed_count.load(Ordering::SeqCst), 1);
        assert_eq!(task.completed_count.load(Ordering::SeqCst), 0);
        assert!(!target.exists());
    }
}

#[test]
fn http_range_error_retries_as_a_full_download() {
    setup();
    let (url, handle) = server(vec![
        "HTTP/1.1 200 OK\r\nAccept-Ranges: bytes\r\nContent-Length: 10\r\nConnection: close\r\n\r\nabc".into(),
        "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Length: 5\r\nConnection: close\r\n\r\nerror".into(),
        "HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nabcdefghij".into(),
    ]);
    let (target, task, item) = run_download(url, "range-error", FileHash::None, LaterRun::None);
    let requests = handle.join().unwrap();
    assert!(requests[1].contains("range: bytes=3-"));
    assert!(!requests[2].contains("range:"));
    assert_eq!(item.get_state(), DownloadItemState::Done);
    assert_eq!(task.completed_count.load(Ordering::SeqCst), 1);
    assert_eq!(fs::read(target).unwrap(), b"abcdefghij");
}
