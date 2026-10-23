use super::*;

#[tokio::test]
async fn rejected_room_requests_preserve_external_room_ownership() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        for status in ["400 Bad Request", "200 OK", "400 Bad Request", "200 OK"] {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 1024];
            assert!(stream.read(&mut request).await.unwrap() > 0);
            stream
                .write_all(
                    format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                        .as_bytes(),
                )
                .await
                .unwrap();
        }
    });
    let session = Session {
        client: reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap(),
        base: format!("http://{address}"),
        owned: false,
        room_owned: AtomicBool::new(false),
        worker: None,
    };
    assert!(session.action("/state/guesting", &[]).await.is_err());
    assert!(!session.room_owned.load(Ordering::Acquire));
    session.action("/state/guesting", &[]).await.unwrap();
    assert!(session.room_owned.load(Ordering::Acquire));
    assert!(session.action("/state/ide", &[]).await.is_err());
    assert!(session.room_owned.load(Ordering::Acquire));
    session.action("/state/ide", &[]).await.unwrap();
    assert!(!session.room_owned.load(Ordering::Acquire));
    server.await.unwrap();
}

#[tokio::test]
async fn reopening_waits_for_previous_room_cleanup() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (seen, accepted) = tokio::sync::oneshot::channel();
    let (finish, response) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0; 1024];
        let count = stream.read(&mut request).await.unwrap();
        assert!(String::from_utf8_lossy(&request[..count]).starts_with("GET /state/ide "));
        seen.send(()).unwrap();
        response.await.unwrap();
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
    });
    let gate = Arc::new(Mutex::new(()));
    let cancel = CancellationToken::new();
    let old = Session {
        client: reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap(),
        base: format!("http://{address}"),
        owned: false,
        room_owned: AtomicBool::new(true),
        worker: Some(WorkerLease::acquire(gate.clone(), &cancel).await.unwrap()),
    };
    drop(old);
    tokio::time::timeout(Duration::from_secs(3), accepted)
        .await
        .unwrap()
        .unwrap();
    let next = WorkerLease::acquire(gate.clone(), &cancel);
    tokio::pin!(next);
    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut next)
            .await
            .is_err()
    );
    finish.send(()).unwrap();
    let lease = tokio::time::timeout(Duration::from_secs(3), next)
        .await
        .unwrap()
        .unwrap();
    server.await.unwrap();

    // 解包线程持有的副本也须先结束，取消等待本身不能释放别人的租约。
    let unpack = lease.clone();
    drop(lease);
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(
        WorkerLease::acquire(gate.clone(), &cancelled)
            .await
            .is_err()
    );
    assert!(gate.try_lock().is_err());
    drop(unpack);
    assert!(gate.try_lock().is_ok());
}

#[test]
fn room_state_and_local_join_address_follow_protocol() {
    let state: State = serde_json::from_str(r#"{"state":"guest-ok","index":7,"url":"127.0.0.1:30000","profiles":[{"machine_id":"one","name":"player","kind":"LOCAL","vendor":"Terracotta"}]}"#).unwrap();
    assert_eq!(state.index, 7);
    assert_eq!(
        join_server(state.url.as_deref().unwrap()).unwrap().port,
        Some(30000)
    );
    assert_eq!(join_server("127.0.0.1").unwrap().port, Some(25565));
    for address in [
        "example.com",
        "127.0.0.1:0",
        "127.0.0.1:65536",
        "127.0.0.1:22/path",
    ] {
        assert!(join_server(address).is_err());
    }
}
#[tokio::test]
async fn http_adapter_encodes_room_nodes_checks_status_and_closes_owned_worker() {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let worker = std::thread::spawn(move || {
        for step in 0..4 {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(10))
                    }
                    Err(error) => panic!("HTTP request {step} missing: {error}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let target = line.split_whitespace().nth(1).unwrap();
            let url = reqwest::Url::parse(&format!("http://localhost{target}")).unwrap();
            let (status, body) = match step {
                0 => {
                    assert_eq!(url.path(), "/state");
                    (
                        "200 OK",
                        r#"{"state":"guest-ok","index":8,"url":"127.0.0.1:30000","profiles":[]}"#,
                    )
                }
                1 => {
                    assert_eq!(url.path(), "/state/guesting");
                    let pairs = url.query_pairs().collect::<Vec<_>>();
                    assert!(pairs.iter().any(|(k, v)| k == "room" && v == "U/TEST+CODE"));
                    assert!(pairs.iter().any(|(k, v)| k == "player" && v == "玩家 name"));
                    assert_eq!(pairs.iter().filter(|(k, _)| k == "public_nodes").count(), 2);
                    ("200 OK", "")
                }
                2 => ("400 Bad Request", "invalid room"),
                _ => {
                    assert_eq!(target, "/panic?peaceful=true");
                    ("200 OK", "")
                }
            };
            write!(stream, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        }
    });
    let session = Session {
        client: reqwest::Client::builder().no_proxy().build().unwrap(),
        base: format!("http://{address}"),
        owned: true,
        room_owned: AtomicBool::new(false),
        worker: None,
    };
    assert_eq!(
        session.state().await.unwrap().address.as_deref(),
        Some("127.0.0.1:30000")
    );
    session
        .action(
            "/state/guesting",
            &[
                ("room".into(), "U/TEST+CODE".into()),
                ("player".into(), "玩家 name".into()),
                ("public_nodes".into(), "tcp://one:1234".into()),
                ("public_nodes".into(), "wss://two/ws".into()),
            ],
        )
        .await
        .unwrap();
    assert!(session.action("/state/guesting", &[]).await.is_err());
    drop(session);
    tokio::task::spawn_blocking(move || worker.join().unwrap())
        .await
        .unwrap();
}
