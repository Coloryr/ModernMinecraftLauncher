use mml_cloud::{
    Manifest, archive,
    client::{Client, Settings},
};
use mml_cloud_server::Config;
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::PathBuf, time::Duration};
use uuid::Uuid;

const SERVER: &str = "server-secret-123456789";
const CLIENT: &str = "client-secret-123456789";
const OTHER: &str = "other-client-1234567890";

/// 持有隔离数据目录与本机 HTTP 服务的测试句柄。
struct Server {
    /// 本用例的临时数据目录。
    root: PathBuf,
    /// 随机端口上的服务地址。
    url: String,
    /// 正常停止监听并等待现有请求结束。
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    /// 用例中断时取消服务，重启时等待数据目录锁释放。
    task: Option<tokio::task::JoinHandle<()>>,
}
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}
impl Server {
    async fn start(quota: u64) -> Self {
        let root = mml_testutil::temp_path(format!("cloud-http-{}", Uuid::new_v4()));
        Self::start_at(root, quota).await
    }
    async fn start_at(root: PathBuf, quota: u64) -> Self {
        fs::create_dir_all(&root).unwrap();
        let router = mml_cloud_server::router(Config {
            listen: String::new(),
            data_dir: root.join("data"),
            server_key: SERVER.into(),
            clients: BTreeMap::from([(CLIENT.into(), quota), (OTHER.into(), quota)]),
        })
        .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
                .unwrap();
        });
        Self {
            root,
            url,
            stop: Some(stop),
            task: Some(task),
        }
    }
    async fn restart(mut self, quota: u64) -> Self {
        self.stop.take().unwrap().send(()).unwrap();
        // 等待连接任务结束，避免旧服务尚未释放数据目录锁。
        tokio::time::timeout(Duration::from_secs(5), self.task.take().unwrap())
            .await
            .expect("server shutdown timed out")
            .unwrap();
        Self::start_at(self.root.clone(), quota).await
    }
    fn client(&self, key: &str) -> Client {
        Client::new(Settings {
            server: self.url.clone(),
            server_key: SERVER.into(),
            client_key: key.into(),
        })
        .unwrap()
    }
    fn post(&self, endpoint: &str, uuid: &str, name: &str) -> reqwest::RequestBuilder {
        reqwest::Client::new()
            .post(format!("{}/{}", self.url, endpoint))
            .timeout(Duration::from_secs(5))
            .header("serverkey", SERVER)
            .header("clientkey", CLIENT)
            .header("uuid", uuid)
            .header("name", name)
    }
}

#[tokio::test]
async fn real_http_roundtrip_delete_no_change_and_client_isolation() {
    let server = Server::start(1024 * 1024).await;
    let uuid = Uuid::new_v4().to_string();
    let client = server.client(CLIENT);
    assert!(!client.state(&uuid).await.unwrap().have);
    client.set_enabled(&uuid, "测试实例", true).await.unwrap();
    let source = server.root.join("source");
    fs::create_dir_all(source.join("region")).unwrap();
    fs::write(source.join("level.dat"), b"first").unwrap();
    fs::write(source.join("region/deleted.mca"), b"old").unwrap();
    assert!(client.upload(&uuid, "新的世界", &source).await.unwrap());
    assert!(!client.upload(&uuid, "新的世界", &source).await.unwrap());
    assert_eq!(client.worlds(&uuid).await.unwrap()[0].name, "新的世界");
    let target = server.root.join("target");
    let backups = server.root.join("backups");
    assert!(
        client
            .download(&uuid, "新的世界", &target, &backups)
            .await
            .unwrap()
    );
    assert!(
        !client
            .download(&uuid, "新的世界", &target, &backups)
            .await
            .unwrap()
    );
    fs::remove_file(source.join("region/deleted.mca")).unwrap();
    fs::write(source.join("level.dat"), b"second").unwrap();
    client.upload(&uuid, "新的世界", &source).await.unwrap();
    assert!(
        client
            .download(&uuid, "新的世界", &target, &backups)
            .await
            .unwrap()
    );
    assert_eq!(
        archive::scan(&target).unwrap(),
        archive::scan(&source).unwrap()
    );
    assert!(!target.join("region/deleted.mca").exists());
    assert_eq!(
        fs::read(
            fs::read_dir(&backups)
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path()
                .join("level.dat")
        )
        .unwrap(),
        b"first"
    );
    assert!(!server.client(OTHER).state(&uuid).await.unwrap().have);
    assert_eq!(
        server.client(OTHER).worlds(&uuid).await.unwrap_err().key,
        "err.cloudNotEnabled"
    );
    client.delete(&uuid, "新的世界").await.unwrap();
    assert!(client.worlds(&uuid).await.unwrap().is_empty());
}

#[tokio::test]
async fn wrong_keys_and_disabled_instance_are_rejected() {
    let server = Server::start(1024 * 1024).await;
    assert_eq!(
        server
            .client("unregistered-secret")
            .storage()
            .await
            .unwrap_err()
            .key,
        "err.cloudAuth"
    );
    let response = reqwest::Client::new()
        .post(format!("{}/check", server.url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.json::<Value>().await.unwrap()["res"], 300);
    let uuid = Uuid::new_v4().to_string();
    let client = server.client(CLIENT);
    client.set_enabled(&uuid, "test", true).await.unwrap();
    client.set_enabled(&uuid, "test", false).await.unwrap();
    assert!(!client.state(&uuid).await.unwrap().have);
    assert_eq!(
        client.worlds(&uuid).await.unwrap_err().key,
        "err.cloudNotEnabled"
    );
}

#[tokio::test]
async fn quota_rejection_keeps_last_good_snapshot_and_stale_revision_conflicts() {
    let server = Server::start(2048).await;
    let uuid = Uuid::new_v4().to_string();
    let client = server.client(CLIENT);
    client.set_enabled(&uuid, "test", true).await.unwrap();
    let source = server.root.join("world");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("level.dat"), b"small").unwrap();
    client.upload(&uuid, "world", &source).await.unwrap();
    let before = client.worlds(&uuid).await.unwrap()[0].files.clone();
    fs::write(source.join("large"), vec![7u8; 4096]).unwrap();
    assert_eq!(
        client
            .upload(&uuid, "world", &source)
            .await
            .unwrap_err()
            .key,
        "err.cloudQuota"
    );
    assert_eq!(client.worlds(&uuid).await.unwrap()[0].files, before);
    let body = archive::delta(&source, &archive::scan(&source).unwrap(), &Manifest::new())
        .unwrap()
        .unwrap();
    let response = server
        .post("putworld", &uuid, "world")
        .header("if-match", "stale-revision")
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.json::<Value>().await.unwrap()["res"], 105);
    assert!(client.storage().await.unwrap().used < 2048);
}

#[tokio::test]
async fn config_endpoints_and_instance_list_use_colormc_wire_format() {
    let server = Server::start(1024 * 1024).await;
    let uuid = Uuid::new_v4().to_string();
    server
        .client(CLIENT)
        .set_enabled(&uuid, "config-test", true)
        .await
        .unwrap();
    let source = server.root.join("config");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("options.txt"), b"guiScale:2").unwrap();
    let bytes = archive::delta(&source, &archive::scan(&source).unwrap(), &Manifest::new())
        .unwrap()
        .unwrap();
    let put = server
        .post("putconfig", &uuid, "")
        .body(bytes)
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(put["res"], 100);
    assert!(put["time"].is_string());
    let downloaded = server.post("pullconfig", &uuid, "").send().await.unwrap();
    assert_eq!(downloaded.headers()["content-type"], "application/zip");
    let target = server.root.join("config-copy");
    archive::apply(
        &target,
        &downloaded.bytes().await.unwrap(),
        &server.root.join("backup"),
    )
    .unwrap();
    assert_eq!(
        archive::scan(&source).unwrap(),
        archive::scan(&target).unwrap()
    );
    let list = server
        .post("getlist", "", "")
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(list["list"][0], json!({"UUID":uuid,"Name":"config-test"}));
}

#[tokio::test]
async fn concurrent_uploads_from_one_revision_commit_only_one_complete_snapshot() {
    let server = Server::start(1024 * 1024).await;
    let uuid = Uuid::new_v4().to_string();
    let client = server.client(CLIENT);
    client.set_enabled(&uuid, "test", true).await.unwrap();
    let source = server.root.join("original");
    fs::create_dir_all(source.join("region")).unwrap();
    fs::write(source.join("level.dat"), b"original").unwrap();
    fs::write(source.join("region/old.mca"), b"remove").unwrap();
    fs::write(source.join("unchanged.dat"), b"keep").unwrap();
    client.upload(&uuid, "world", &source).await.unwrap();
    let original = archive::scan(&source).unwrap();
    let candidates = [server.root.join("left"), server.root.join("right")];
    let mut updates = Vec::new();
    for (index, path) in candidates.iter().enumerate() {
        fs::create_dir_all(path.join("region")).unwrap();
        fs::write(path.join("level.dat"), format!("candidate-{index}")).unwrap();
        fs::write(path.join(format!("region/{index}.mca")), [index as u8; 32]).unwrap();
        fs::write(path.join("unchanged.dat"), b"keep").unwrap();
        let bytes = archive::delta(path, &archive::scan(path).unwrap(), &original)
            .unwrap()
            .unwrap();
        updates.push(
            server
                .post("putworld", &uuid, "world")
                .header("if-match", mml_cloud::revision(&original))
                .body(bytes),
        );
    }
    let right = updates.pop().unwrap();
    let left = updates.pop().unwrap();
    let (left, right) = tokio::join!(left.send(), right.send());
    let mut codes = Vec::new();
    for response in [left.unwrap(), right.unwrap()] {
        codes.push(
            response.json::<Value>().await.unwrap()["res"]
                .as_u64()
                .unwrap(),
        );
    }
    assert!(
        codes == [100, 105] || codes == [105, 100],
        "unexpected concurrent upload results: {codes:?}"
    );
    let winner = usize::from(codes[1] == 100);
    let expected = archive::scan(&candidates[winner]).unwrap();
    assert_eq!(client.worlds(&uuid).await.unwrap()[0].files, expected);
    let target = server.root.join("download");
    client
        .download(&uuid, "world", &target, &server.root.join("backups"))
        .await
        .unwrap();
    assert_eq!(archive::scan(&target).unwrap(), expected);
    assert_eq!(fs::read(target.join("unchanged.dat")).unwrap(), b"keep");
    assert!(!target.join("region/old.mca").exists());
    assert!(!target.join(format!("region/{}.mca", 1 - winner)).exists());
}

#[tokio::test]
async fn concurrent_world_uploads_cannot_exceed_account_quota() {
    let quota = 2048;
    let server = Server::start(quota).await;
    let uuid = Uuid::new_v4().to_string();
    let client = server.client(CLIENT);
    client.set_enabled(&uuid, "test", true).await.unwrap();
    let candidates = [server.root.join("left"), server.root.join("right")];
    let names = ["left-world", "right-world"];
    let mut updates = Vec::new();
    for (index, path) in candidates.iter().enumerate() {
        fs::create_dir_all(path).unwrap();
        fs::write(path.join("level.dat"), vec![index as u8; 1200]).unwrap();
        let bytes = archive::delta(path, &archive::scan(path).unwrap(), &Manifest::new())
            .unwrap()
            .unwrap();
        updates.push(
            server
                .post("putworld", &uuid, names[index])
                .header("if-match", mml_cloud::revision(&Manifest::new()))
                .body(bytes),
        );
    }
    let right = updates.pop().unwrap();
    let left = updates.pop().unwrap();
    let (left, right) = tokio::join!(left.send(), right.send());
    let mut codes = Vec::new();
    for response in [left.unwrap(), right.unwrap()] {
        codes.push(
            response.json::<Value>().await.unwrap()["res"]
                .as_u64()
                .unwrap(),
        );
    }
    assert!(
        codes == [100, 104] || codes == [104, 100],
        "unexpected concurrent quota results: {codes:?}"
    );
    let winner = usize::from(codes[1] == 100);
    let worlds = client.worlds(&uuid).await.unwrap();
    assert_eq!(worlds.len(), 1);
    assert_eq!(worlds[0].name, names[winner]);
    assert_eq!(worlds[0].files, archive::scan(&candidates[winner]).unwrap());
    let storage = client.storage().await.unwrap();
    assert!(storage.used >= 1200 && storage.used <= quota);
    assert_eq!(storage.size, quota);
    assert_eq!(
        server
            .post("getworldfiles", &uuid, names[1 - winner])
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()["res"],
        102
    );
    assert_eq!(
        fs::read_dir(server.root.join("data/staging"))
            .unwrap()
            .count(),
        0
    );
    client.delete(&uuid, names[winner]).await.unwrap();
    assert!(
        client
            .upload(&uuid, names[1 - winner], &candidates[1 - winner])
            .await
            .unwrap()
    );
    assert_eq!(
        client.worlds(&uuid).await.unwrap()[0].name,
        names[1 - winner]
    );
    assert!(client.storage().await.unwrap().used <= quota);
}

#[tokio::test]
async fn restart_preserves_worlds_enabled_state_and_storage_usage() {
    let quota = 32 * 1024;
    let server = Server::start(quota).await;
    let active = Uuid::new_v4().to_string();
    let paused = Uuid::new_v4().to_string();
    let client = server.client(CLIENT);
    let source = server.root.join("world");
    fs::create_dir_all(source.join("region")).unwrap();
    fs::write(source.join("level.dat"), b"saved before restart").unwrap();
    fs::write(source.join("region/r.0.0.mca"), [7; 128]).unwrap();
    for uuid in [&active, &paused] {
        client.set_enabled(uuid, "persistent", true).await.unwrap();
        client.upload(uuid, "world", &source).await.unwrap();
    }
    let active_worlds = serde_json::to_value(client.worlds(&active).await.unwrap()).unwrap();
    let paused_worlds = serde_json::to_value(client.worlds(&paused).await.unwrap()).unwrap();
    client
        .set_enabled(&paused, "persistent", false)
        .await
        .unwrap();
    let storage_before = client.storage().await.unwrap();
    drop(client);

    let server = server.restart(quota).await;
    let client = server.client(CLIENT);
    assert!(client.state(&active).await.unwrap().have);
    assert!(!client.state(&paused).await.unwrap().have);
    assert_eq!(
        client.worlds(&paused).await.unwrap_err().key,
        "err.cloudNotEnabled"
    );
    assert_eq!(
        serde_json::to_value(client.worlds(&active).await.unwrap()).unwrap(),
        active_worlds
    );
    let storage_after = client.storage().await.unwrap();
    assert_eq!(storage_after.used, storage_before.used);
    assert_eq!(storage_after.size, quota);
    assert_eq!(server.client(OTHER).storage().await.unwrap().used, 0);
    client
        .set_enabled(&paused, "persistent", true)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(client.worlds(&paused).await.unwrap()).unwrap(),
        paused_worlds
    );
    for uuid in [&active, &paused] {
        let target = server.root.join(format!("restored-{uuid}"));
        assert!(
            client
                .download(uuid, "world", &target, &server.root.join("backups"))
                .await
                .unwrap()
        );
        assert_eq!(
            archive::scan(&target).unwrap(),
            archive::scan(&source).unwrap()
        );
    }
}
