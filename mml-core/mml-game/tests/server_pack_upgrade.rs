//! 在线服务器包升级
//!
//! 本机起一个按路径回字节的服务，先导入版本 1 的包，再把服务上的内容换成版本 2，
//! 走 [upgrade_server_pack] 完整跑一遍升级：新增 / 删除 / 改内容的文件、清单版本、
//! 实例的加载器版本与在线信息都该跟上。

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex, Once};
use std::time::Duration;

use mml_base::hash_helper::{HashType, gen_hash};
use mml_game::launcher::ModPackType;
use mml_game::loader::LoaderType;
use mml_game::serverpack::serverpack_obj::{ServerItemObj, ServerPackObj};
use tokio_util::sync::CancellationToken;

/// 初始化链：运行目录、路径、日志、配置、网络客户端、下载器与实例表
fn ensure_init() -> std::path::PathBuf {
    static INIT: Once = Once::new();
    let dir = mml_testutil::temp_dir().join(format!("mml-sp-upgrade-{}", std::process::id()));
    INIT.call_once(|| {
        std::fs::create_dir_all(&dir).unwrap();
        mml_base::init(&dir);
        mml_names::init(mml_base::get_base_dir()).unwrap();
        mml_log::start(mml_base::get_base_dir()).unwrap();
        mml_config::init(mml_base::get_base_dir()).unwrap();
        mml_config::config_save::start();
        mml_net::init();
        mml_downloader::init(&dir).unwrap();
        mml_downloader::start();
        mml_game::init(mml_base::get_base_dir()).unwrap();
    });
    dir
}

/// 按请求路径回字节的本机服务，路径表可以中途换掉，用来模拟发布新版
#[derive(Clone)]
struct Routes(Arc<Mutex<HashMap<String, Vec<u8>>>>);

impl Routes {
    /// 起服务并返回它的路径表
    fn start() -> (Self, u16) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("绑定本机端口失败");
        let port = listener.local_addr().unwrap().port();
        let routes = Routes(Arc::new(Mutex::new(HashMap::new())));
        let shared = routes.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let routes = shared.clone();
                std::thread::spawn(move || {
                    let mut buf = [0u8; 4096];
                    let n = stream.read(&mut buf).unwrap_or(0);
                    let request = String::from_utf8_lossy(&buf[..n]).to_string();
                    let path = request
                        .lines()
                        .next()
                        .and_then(|line| line.split_whitespace().nth(1))
                        .unwrap_or("/")
                        .to_string();
                    let body = routes.0.lock().unwrap().get(&path).cloned();
                    let (body, status) = match body {
                        Some(body) => (body, "200 OK"),
                        None => (Vec::new(), "404 Not Found"),
                    };
                    let head = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(head.as_bytes());
                    let _ = stream.write_all(&body);
                    let _ = stream.flush();
                });
            }
        });
        (routes, port)
    }

    /// 换掉整张路径表
    fn set(&self, files: Vec<(&str, Vec<u8>)>) {
        let mut map = self.0.lock().unwrap();
        map.clear();
        for (path, body) in files {
            map.insert(path.to_string(), body);
        }
    }
}

/// 一条清单文件项
fn item(port: u16, file: &str, path: &str, body: &[u8]) -> ServerItemObj {
    ServerItemObj {
        file: file.to_string(),
        url: Some(format!("http://127.0.0.1:{port}/pack/{path}")),
        sha1: Some(gen_hash(HashType::Sha1, body)),
        ..Default::default()
    }
}

/// 只带下载地址的清单文件项，没有项目号与文件号
fn plain(port: u16, file: &str, path: &str, body: &[u8]) -> ServerItemObj {
    item(port, file, path, body)
}

/// 造一份清单
fn manifest(version: &str, loader_version: &str, files: Vec<ServerItemObj>) -> ServerPackObj {
    ServerPackObj {
        name: "升级测试实例".to_string(),
        version: "1.21.1".to_string(),
        loader: LoaderType::NeoForge,
        loader_version: Some(loader_version.to_string()),
        pack_version: version.to_string(),
        online_list: files,
        ..Default::default()
    }
}

/// 清单的 JSON 字节
fn manifest_bytes(pack: &ServerPackObj) -> Vec<u8> {
    serde_json::to_vec(pack).unwrap()
}

/// 在单线程运行时上跑一个 future
fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(future)
}

/// 记一条带项目号的清单项
fn indexed(port: u16, file: &str, path: &str, body: &[u8], pid: &str, fid: &str) -> ServerItemObj {
    ServerItemObj {
        pid: Some(pid.to_string()),
        fid: Some(fid.to_string()),
        ..item(port, file, path, body)
    }
}

const A_JAR: &[u8] = b"mod a v1";
const B_JAR: &[u8] = b"mod b v2";
const X_JS_V1: &[u8] = b"// v1";
const X_JS_V2: &[u8] = b"// v2 changed";

#[test]
fn upgrade_applies_add_remove_and_change() {
    let run = ensure_init();
    let (routes, port) = Routes::start();

    // 版本 1：a.jar + x.js 的旧内容
    let v1 = manifest(
        "1.0.0",
        "21.1.259",
        vec![
            indexed(port, "mods/a.jar", "a.jar", A_JAR, "111", "1111"),
            plain(port, "kubejs/x.js", "x1.js", X_JS_V1),
        ],
    );
    routes.set(vec![
        ("/pack/server.json", manifest_bytes(&v1)),
        ("/pack/a.jar", A_JAR.to_vec()),
        ("/pack/x1.js", X_JS_V1.to_vec()),
    ]);

    let uuid = block_on(mml_game::add_game::install_server_pack_from_url(
        &format!("http://127.0.0.1:{port}/pack"),
        None,
        None,
        None,
        None,
        CancellationToken::new(),
    ))
    .expect("导入应成功");

    let game = mml_game::get_instance(&uuid).expect("实例应已登记");
    let base = {
        let game = game.read().unwrap();
        assert_eq!(game.loader_version.as_deref(), Some("21.1.259"));
        game.get_game_path()
    };
    assert!(base.join("mods/a.jar").is_file());
    assert_eq!(std::fs::read(base.join("kubejs/x.js")).unwrap(), X_JS_V1);

    // 版本 2：删掉 a.jar、加一个 b.jar、改 x.js 的内容，加载器版本一起往上走
    let v2 = manifest(
        "1.0.1",
        "21.1.300",
        vec![
            plain(port, "kubejs/x.js", "x2.js", X_JS_V2),
            indexed(port, "mods/b.jar", "b.jar", B_JAR, "222", "2222"),
        ],
    );
    routes.set(vec![
        ("/pack/server.json", manifest_bytes(&v2)),
        ("/pack/b.jar", B_JAR.to_vec()),
        ("/pack/x2.js", X_JS_V2.to_vec()),
    ]);

    let snapshot = game.read().unwrap().clone();
    // 清单是后台线程落的盘，升级要先能看到旧清单，否则算不出该删什么
    let local_manifest = snapshot.get_server_pack_file();
    for _ in 0..100 {
        if local_manifest.is_file() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(local_manifest.is_file(), "旧清单应已落盘");

    block_on(mml_game::add_game::upgrade_server_pack(
        &snapshot,
        None,
        CancellationToken::new(),
    ))
    .expect("升级应成功");

    // 文件：a 被删、b 落地、x.js 换成新内容
    assert!(
        !base.join("mods/a.jar").exists(),
        "清单里没有的文件该被删掉"
    );
    assert_eq!(std::fs::read(base.join("mods/b.jar")).unwrap(), B_JAR);
    assert_eq!(std::fs::read(base.join("kubejs/x.js")).unwrap(), X_JS_V2);

    // 清单与实例设置：包版本、加载器版本都跟着新版
    let pack: ServerPackObj =
        serde_json::from_slice(&std::fs::read(snapshot.get_server_pack_file()).unwrap()).unwrap();
    assert_eq!(pack.pack_version, "1.0.1");
    {
        let game = game.read().unwrap();
        assert_eq!(game.loader_version.as_deref(), Some("21.1.300"));
        assert_eq!(game.modpack_type, ModPackType::ServerPack);
    }

    // 在线信息：只剩 b.jar 那条，项目号 / 文件号来自新清单
    let mut info = snapshot.read_online_info();
    for _ in 0..100 {
        if info.len() == 1 {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
        info = snapshot.read_online_info();
    }
    assert_eq!(info.len(), 1, "在线信息应只剩 b.jar：{info:?}");
    let entry = info.values().next().unwrap();
    assert_eq!(entry.file, "b.jar");
    assert_eq!(entry.modid, "222");
    assert_eq!(entry.fileid, "2222");

    let _ = std::fs::remove_dir_all(&run);
}
