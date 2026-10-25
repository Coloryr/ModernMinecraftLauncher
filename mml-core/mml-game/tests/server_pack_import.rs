//! 在线实例（server.json）导入
//!
//! 本机起一个只回清单的 HTTP 服务，走 [`install_server_pack_from_url`] 的完整链路：
//! 地址补齐 → 下载清单 → 解析 → 登记实例。
//!
//! 关键口径：用户填的地址结尾必须是 `server.json`（只填目录会自动补上），
//! 而实例里存的 `server_url` 是**发布目录** —— 启动同步时自己拼 `server.json` 与 `.sha1`。

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex, Once};
use std::time::Duration;

use mml_game::gui_hook::{AddModPackState, IAddModPackGui};
use mml_game::launcher::ModPackType;
use mml_game::loader::LoaderType;
use mml_game::serverpack::serverpack_obj::{ServerItemObj, ServerPackObj};
use tokio_util::sync::CancellationToken;

/// 初始化链：运行目录、路径、日志、配置、网络客户端与实例表
fn ensure_init() -> std::path::PathBuf {
    static INIT: Once = Once::new();
    let dir = mml_testutil::temp_dir().join(format!("mml-sp-import-{}", std::process::id()));
    INIT.call_once(|| {
        std::fs::create_dir_all(&dir).unwrap();
        mml_base::init(&dir);
        mml_names::init(mml_base::get_base_dir()).unwrap();
        mml_log::start(mml_base::get_base_dir()).unwrap();
        mml_config::init(mml_base::get_base_dir()).unwrap();
        mml_config::config_save::start();
        mml_net::init();
        // 导入会真的去下清单里的文件，下载器必须先起来，否则任务永远等着
        mml_downloader::init(&dir).unwrap();
        mml_downloader::start();
        mml_game::init(mml_base::get_base_dir()).unwrap();
    });
    dir
}

/// 本机服务：`icon.png` 与 `tiny.jar` 各有一份固定内容，其余路径回当前清单
///
/// 清单里有下载地址，得先拿到端口才能写，所以拆成"先监听、后填清单"两步。
fn json_server_slot() -> (u16, Arc<Mutex<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定本机端口失败");
    let port = listener.local_addr().unwrap().port();
    let slot = Arc::new(Mutex::new(String::new()));
    let shared = slot.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let shared = shared.clone();
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]).to_string();
                let (body, kind) = if request.contains("icon.png") {
                    (ICON_PNG.to_vec(), "image/png")
                } else if request.contains("tiny.jar") {
                    (TINY_JAR.to_vec(), "application/java-archive")
                } else if request.contains("slow.jar") {
                    // 慢一点，让轮询能抓到"下到一半"的那一帧
                    std::thread::sleep(Duration::from_millis(1200));
                    (TINY_JAR.to_vec(), "application/java-archive")
                } else {
                    (shared.lock().unwrap().clone().into_bytes(), "application/json")
                };
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&body);
                let _ = stream.flush();
            });
        }
    });
    (port, slot)
}

/// 只回同一份清单的服务
fn json_server(body: String) -> u16 {
    let (port, slot) = json_server_slot();
    *slot.lock().unwrap() = body;
    port
}

/// 清单里那个假模组的字节
const TINY_JAR: &[u8] = b"tiny mod bytes";

/// 1×1 的透明 PNG，够验证"文件被原样写进实例目录"
const ICON_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

/// 造一份最小清单
fn manifest() -> String {
    let pack = ServerPackObj {
        name: "ATM10 测试实例".to_string(),
        version: "1.21.1".to_string(),
        loader: LoaderType::NeoForge,
        loader_version: Some("21.1.259".to_string()),
        pack_version: "1.0.0".to_string(),
        ..Default::default()
    };
    serde_json::to_string(&pack).unwrap()
}

/// 把 manifest() 解析回来，方便按用例改字段
fn decode_manifest() -> ServerPackObj {
    serde_json::from_str(&manifest()).unwrap()
}

/// 只填目录也能导入：地址补齐成 server.json，实例里存目录
#[test]
fn import_from_directory_url() {
    let run = ensure_init();
    let port = json_server(manifest());
    let base = format!("http://127.0.0.1:{port}/pack");

    let uuid = block_on(mml_game::add_game::install_server_pack_from_url(
        &base,
        None,
        None,
        None,
        None,
        CancellationToken::new(),
    ))
    .expect("导入应成功");

    let instance = mml_game::get_instance(&uuid).expect("实例应已登记");
    let instance = instance.read().unwrap();
    assert_eq!(instance.name, "ATM10 测试实例");
    assert_eq!(instance.version, "1.21.1");
    assert_eq!(instance.loader, LoaderType::NeoForge);
    assert_eq!(instance.loader_version.as_deref(), Some("21.1.259"));
    assert!(instance.is_modpack);
    assert_eq!(instance.modpack_type, ModPackType::ServerPack);
    assert_eq!(instance.server_url.as_deref(), Some(base.as_str()));
    assert!(instance.get_base_path().starts_with(&run));
}

/// 直接贴 server.json 也认，名字可以由调用方覆盖
#[test]
fn import_from_manifest_url_with_name() {
    ensure_init();
    let port = json_server(manifest());

    let uuid = block_on(mml_game::add_game::install_server_pack_from_url(
        &format!("http://127.0.0.1:{port}/pack/server.json"),
        Some("我自己起的名字".to_string()),
        None,
        None,
        None,
        CancellationToken::new(),
    ))
    .expect("导入应成功");

    let instance = mml_game::get_instance(&uuid).expect("实例应已登记");
    let instance = instance.read().unwrap();
    assert_eq!(instance.name, "我自己起的名字");
    assert_eq!(
        instance.server_url.as_deref(),
        Some(format!("http://127.0.0.1:{port}/pack").as_str())
    );
}

/// 探测接口：一次拿回清单摘要，名字 / 版本 / 加载器 / 条目数都在里面
#[test]
fn probe_reads_manifest_summary() {
    ensure_init();
    let port = json_server(manifest());

    let (url, pack) = block_on(mml_game::add_game::fetch_server_pack(&format!(
        "http://127.0.0.1:{port}"
    )))
    .expect("探测应成功");

    assert_eq!(url, format!("http://127.0.0.1:{port}/server.json"));
    assert_eq!(pack.name, "ATM10 测试实例");
    assert_eq!(pack.version, "1.21.1");
    assert_eq!(pack.loader, LoaderType::NeoForge);
    assert_eq!(pack.loader_version.as_deref(), Some("21.1.259"));
    assert_eq!(pack.pack_version, "1.0.0");
    assert!(pack.online_list.is_empty());
    assert!(pack.archive_list.is_empty());
}

/// 进度从下载器的任务表同步：下到一半就能看到 1/2，不是等全部下完才跳一下
#[test]
fn progress_follows_download_task() {
    ensure_init();
    let (port, slot) = json_server_slot();
    *slot.lock().unwrap() = manifest_with_slow_file(port);

    let sink = Arc::new(ProgressSink::default());
    let _uuid = block_on(mml_game::add_game::install_server_pack_from_url(
        &format!("http://127.0.0.1:{port}/pack"),
        None,
        None,
        None,
        Some(sink.clone() as Arc<dyn IAddModPackGui>),
        CancellationToken::new(),
    ))
    .expect("导入应成功");

    let steps = sink.0.lock().unwrap().clone();
    assert!(
        steps.iter().any(|(done, all)| *done == 1 && *all == Some(2)),
        "下到一半应报 1/2：{steps:?}"
    );
    assert_eq!(steps.last(), Some(&(2, Some(2))), "最后应报 2/2：{steps:?}");
}

/// 两个文件：一个秒回，一个慢 1.2 秒
fn manifest_with_slow_file(port: u16) -> String {
    let mut pack = decode_manifest();
    for name in ["tiny.jar", "slow.jar"] {
        pack.online_list.push(ServerItemObj {
            file: format!("mods/{name}"),
            url: Some(format!("http://127.0.0.1:{port}/pack/{name}")),
            ..Default::default()
        });
    }
    serde_json::to_string(&pack).unwrap()
}

/// 导入时就同步：清单里的文件在导入结束时已经落进实例目录
#[test]
fn import_downloads_files() {
    ensure_init();
    let (port, slot) = json_server_slot();
    *slot.lock().unwrap() = manifest_with_file(port);

    let sink = Arc::new(ProgressSink::default());
    let uuid = block_on(mml_game::add_game::install_server_pack_from_url(
        &format!("http://127.0.0.1:{port}/pack"),
        None,
        None,
        None,
        Some(sink.clone() as Arc<dyn IAddModPackGui>),
        CancellationToken::new(),
    ))
    .expect("导入应成功");

    let instance = mml_game::get_instance(&uuid).expect("实例应已登记");
    let instance = instance.read().unwrap();
    let jar = instance.get_game_path().join("mods").join("tiny.jar");
    assert!(jar.exists(), "清单里的文件应已下载");
    assert_eq!(std::fs::read(&jar).unwrap(), TINY_JAR);

    // 进度要真的往前走到 100%，否则界面上的条一直停在 0
    let steps = sink.0.lock().unwrap().clone();
    assert_eq!(steps.first().map(|(done, _)| *done), Some(0));
    assert_eq!(steps.last(), Some(&(1, Some(1))), "进度应走完：{steps:?}");
}

/// 清单里带一个要下载的文件，地址指向本机服务
fn manifest_with_file(port: u16) -> String {
    let mut pack = decode_manifest();
    pack.online_list.push(ServerItemObj {
        file: "mods/tiny.jar".to_string(),
        url: Some(format!("http://127.0.0.1:{port}/pack/tiny.jar")),
        sha1: Some(mml_base::hash_helper::gen_hash(
            mml_base::hash_helper::HashType::Sha1,
            TINY_JAR,
        )),
        ..Default::default()
    });
    serde_json::to_string(&pack).unwrap()
}

/// 清单里写了图标就顺带下下来：写进实例目录并记到 Icon
#[test]
fn import_downloads_icon() {
    ensure_init();
    let port = json_server(manifest_with_icon());

    let uuid = block_on(mml_game::add_game::install_server_pack_from_url(
        &format!("http://127.0.0.1:{port}/pack/server.json"),
        None,
        None,
        None,
        None,
        CancellationToken::new(),
    ))
    .expect("导入应成功");

    let instance = mml_game::get_instance(&uuid).expect("实例应已登记");
    let instance = instance.read().unwrap();
    assert_eq!(instance.icon.as_deref(), Some("icon.png"));
    let icon = instance.get_base_path().join("icon.png");
    assert_eq!(std::fs::read(&icon).unwrap(), ICON_PNG);
    // 同步过就有一份本地清单，下次启动不必重新判断是不是第一次；
    // 落盘走 config_save 的后台线程，等它一小会儿
    let manifest = instance.get_server_pack_file();
    for _ in 0..50 {
        if manifest.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(manifest.exists(), "清单应落进实例目录");
}

/// 带图标的清单
fn manifest_with_icon() -> String {
    let mut pack = decode_manifest();
    pack.icon = Some("icon.png".to_string());
    serde_json::to_string(&pack).unwrap()
}

/// 空地址直接报错，不去发请求
#[test]
fn empty_url_is_rejected() {
    ensure_init();
    let res = block_on(mml_game::add_game::install_server_pack_from_url(
        "   ",
        None,
        None,
        None,
        None,
        CancellationToken::new(),
    ));
    assert!(res.is_err());
}

/// 清单里带项目号的文件会写进实例的在线信息表，没有编号的不记
#[test]
fn import_records_online_info() {
    ensure_init();
    let (port, slot) = json_server_slot();
    *slot.lock().unwrap() = manifest_with_indexed_file(port);

    let uuid = block_on(mml_game::add_game::install_server_pack_from_url(
        &format!("http://127.0.0.1:{port}/pack"),
        None,
        None,
        None,
        None,
        CancellationToken::new(),
    ))
    .expect("导入应成功");

    let instance = mml_game::get_instance(&uuid).expect("实例应已登记");
    let instance = instance.read().unwrap();

    // 落盘走后台保存线程，等它写完再看
    let file = instance.get_online_info_file();
    let mut info = instance.read_online_info();
    for _ in 0..100 {
        if !info.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
        info = instance.read_online_info();
    }

    assert!(file.exists(), "在线信息应落盘");
    assert_eq!(info.len(), 1, "只有带编号的那条该被记录：{info:?}");
    let entry = info.values().next().unwrap();
    assert_eq!(entry.file, "tiny.jar");
    assert_eq!(entry.path, "mods");
    assert_eq!(entry.modid, "111");
    assert_eq!(entry.fileid, "222");
    assert_eq!(entry.sha1, sha1_of_tiny());
}

/// 一份清单：一个带项目号与文件号的文件，一个只有下载地址的文件
fn manifest_with_indexed_file(port: u16) -> String {
    let mut pack = decode_manifest();
    pack.online_list.push(ServerItemObj {
        file: "mods/tiny.jar".to_string(),
        pid: Some("111".to_string()),
        fid: Some("222".to_string()),
        url: Some(format!("http://127.0.0.1:{port}/pack/mods/tiny.jar")),
        sha1: Some(sha1_of_tiny()),
        ..Default::default()
    });
    pack.online_list.push(ServerItemObj {
        file: "mods/plain.jar".to_string(),
        url: Some(format!("http://127.0.0.1:{port}/pack/mods/plain.jar")),
        ..Default::default()
    });
    serde_json::to_string(&pack).unwrap()
}

/// 假模组的 SHA1
fn sha1_of_tiny() -> String {
    mml_base::hash_helper::gen_hash(mml_base::hash_helper::HashType::Sha1, TINY_JAR)
}

/// 只记录进度的假界面：验证导入时进度真的在往前走
#[derive(Default)]
struct ProgressSink(Mutex<Vec<(usize, Option<usize>)>>);

impl IAddModPackGui for ProgressSink {
    fn set_state(&self, _state: AddModPackState) {}
    fn set_now(&self, value: usize, all: Option<usize>) {
        self.0.lock().unwrap().push((value, all));
    }
    fn set_sub_text(&self, _text: Option<String>) {}
    fn set_sub_now(&self, _value: usize, _all: Option<usize>) {}
}

/// 在单线程运行时上跑一个 future，各用例各建一个，互不影响
fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(future)
}
