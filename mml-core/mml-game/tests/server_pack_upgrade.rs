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
use mml_game::launcher::instance_setting_obj::InstanceSettingObj;
use mml_game::launcher::ModPackType;
use mml_game::loader::LoaderType;
use mml_game::serverpack::serverpack_obj::{
    ServerArchiveItemObj, ServerBaseObj, ServerItemObj, ServerPackObj,
};
use tokio_util::sync::CancellationToken;

/// 这些用例共享同一份运行目录与全局实例表，串行执行
static TEST_LOCK: Mutex<()> = Mutex::new(());

/// 初始化链：运行目录、路径、日志、配置、网络客户端、下载器与实例表
fn ensure_init() -> std::path::PathBuf {
    static INIT: Once = Once::new();
    let dir = mml_testutil::temp_dir().join(format!("mml-sp-upgrade-{}", std::process::id()));
    INIT.call_once(|| {
        let _ = std::fs::remove_dir_all(&dir);
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

/// 一条配置压缩包清单项
fn archive_item(port: u16, file: &str, dir: &str, body: &[u8]) -> ServerArchiveItemObj {
    ServerArchiveItemObj {
        file: file.to_string(),
        dir: dir.to_string(),
        // 测试环境把目录挪回收站会失败（与其它用例同样的环境限制），这里用覆盖解压
        delete_old: false,
        url: format!("http://127.0.0.1:{port}/pack/{file}"),
        sha1: Some(gen_hash(HashType::Sha1, body)),
        ..Default::default()
    }
}

/// 造一个只含若干文件的 zip，条目名即包内路径
fn zip_bytes(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buffer = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buffer);
        let options = zip::write::SimpleFileOptions::default();
        for (name, body) in files {
            zip.start_file(*name, options).unwrap();
            zip.write_all(body).unwrap();
        }
        zip.finish().unwrap();
    }
    buffer.into_inner()
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
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _run = ensure_init();
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
        &[],
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
    let game = game.read().unwrap();
    assert_eq!(game.loader_version.as_deref(), Some("21.1.300"));
    assert_eq!(game.modpack_type, ModPackType::ServerPack);

    drop(game);

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
}

/// 清单版本号没变时，远端清单换了内容也只恢复本地清单，不动文件
#[test]
fn same_pack_version_skips_file_replacement() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _run = ensure_init();
    let (routes, port) = Routes::start();

    let v1 = manifest(
        "1.0.0",
        "21.1.259",
        vec![plain(port, "mods/a.jar", "a1.jar", A_JAR)],
    );
    routes.set(vec![
        ("/pack/server.json", manifest_bytes(&v1)),
        ("/pack/a1.jar", A_JAR.to_vec()),
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
    let base = game.read().unwrap().get_game_path();
    assert_eq!(std::fs::read(base.join("mods/a.jar")).unwrap(), A_JAR);

    let snapshot = game.read().unwrap().clone();
    let local = snapshot.get_server_pack_file();
    for _ in 0..100 {
        if local.is_file() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(local.is_file(), "旧清单应已落盘");

    // 版本号不变，但文件内容与加载器版本都改了
    let v1b = manifest(
        "1.0.0",
        "21.1.999",
        vec![plain(port, "mods/a.jar", "a2.jar", B_JAR)],
    );
    routes.set(vec![
        ("/pack/server.json", manifest_bytes(&v1b)),
        ("/pack/a2.jar", B_JAR.to_vec()),
    ]);

    block_on(mml_game::add_game::upgrade_server_pack(
        &snapshot,
        None,
        CancellationToken::new(),
        &[],
    ))
    .expect("同版本应直接返回");

    assert_eq!(
        std::fs::read(base.join("mods/a.jar")).unwrap(),
        A_JAR,
        "版本号没变时文件不该被替换"
    );
    let pack: ServerPackObj =
        serde_json::from_slice(&std::fs::read(snapshot.get_server_pack_file()).unwrap()).unwrap();
    assert_eq!(pack.pack_version, "1.0.0");
    let expected = gen_hash(HashType::Sha1, A_JAR);
    assert_eq!(pack.online_list[0].sha1.as_deref(), Some(expected.as_str()));
    let game = game.read().unwrap();
    assert_eq!(
        game.loader_version.as_deref(),
        Some("21.1.259"),
        "版本号没变时加载器设置也不该跟着换"
    );
}

/// 版本号变了也只补缺失 / 不一致的文件：已有且哈希一致的不再下载
#[test]
fn upgrade_only_downloads_missing_or_changed_files() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _run = ensure_init();
    let (routes, port) = Routes::start();

    let v1 = manifest(
        "1.0.0",
        "21.1.259",
        vec![
            plain(port, "mods/a.jar", "a1.jar", A_JAR),
            plain(port, "kubejs/x.js", "x1.js", X_JS_V1),
        ],
    );
    routes.set(vec![
        ("/pack/server.json", manifest_bytes(&v1)),
        ("/pack/a1.jar", A_JAR.to_vec()),
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
    let base = game.read().unwrap().get_game_path();
    assert_eq!(std::fs::read(base.join("mods/a.jar")).unwrap(), A_JAR);

    let snapshot = game.read().unwrap().clone();
    let local = snapshot.get_server_pack_file();
    for _ in 0..100 {
        if local.is_file() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(local.is_file(), "旧清单应已落盘");

    // 版本 2：a.jar 原样保留（服务端故意不提供这个地址），删掉 x.js，新增 b.jar
    let v2 = manifest(
        "1.0.1",
        "21.1.259",
        vec![
            plain(port, "mods/a.jar", "a1.jar", A_JAR),
            plain(port, "mods/b.jar", "b2.jar", B_JAR),
        ],
    );
    routes.set(vec![
        ("/pack/server.json", manifest_bytes(&v2)),
        ("/pack/b2.jar", B_JAR.to_vec()),
    ]);

    block_on(mml_game::add_game::upgrade_server_pack(
        &snapshot,
        None,
        CancellationToken::new(),
        &[],
    ))
    .expect("只下载缺失 / 变化的文件应成功");

    assert_eq!(
        std::fs::read(base.join("mods/a.jar")).unwrap(),
        A_JAR,
        "哈希一致的文件不该重下，服务端也没给地址"
    );
    assert_eq!(std::fs::read(base.join("mods/b.jar")).unwrap(), B_JAR);
    assert!(
        !base.join("kubejs/x.js").exists(),
        "新清单删掉的文件应被删掉"
    );
}

/// 配置压缩包按哈希判断：内容没变不重下，内容变了才拉取并解压
#[test]
fn archive_hash_skips_unchanged_and_downloads_changed() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _run = ensure_init();
    let (routes, port) = Routes::start();

    let config_v1 = zip_bytes(&[("a.toml", b"a = 1")]);
    let mut v1 = manifest(
        "1.0.0",
        "21.1.259",
        vec![plain(port, "mods/a.jar", "a1.jar", A_JAR)],
    );
    v1.archive_list = vec![archive_item(port, "config.zip", "config", &config_v1)];
    routes.set(vec![
        ("/pack/server.json", manifest_bytes(&v1)),
        ("/pack/a1.jar", A_JAR.to_vec()),
        ("/pack/config.zip", config_v1.clone()),
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
    let base = game.read().unwrap().get_game_path();
    assert_eq!(std::fs::read(base.join("config/a.toml")).unwrap(), b"a = 1");

    let snapshot = game.read().unwrap().clone();
    let local = snapshot.get_server_pack_file();
    for _ in 0..100 {
        if local.is_file() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(local.is_file(), "旧清单应已落盘");

    // 版本 2：只换模组，config.zip 的哈希没变；服务端故意不提供 config.zip
    let mut v2 = manifest(
        "1.0.1",
        "21.1.259",
        vec![plain(port, "mods/b.jar", "b2.jar", B_JAR)],
    );
    v2.archive_list = vec![archive_item(port, "config.zip", "config", &config_v1)];
    routes.set(vec![
        ("/pack/server.json", manifest_bytes(&v2)),
        ("/pack/b2.jar", B_JAR.to_vec()),
    ]);

    block_on(mml_game::add_game::upgrade_server_pack(
        &snapshot,
        None,
        CancellationToken::new(),
        &[],
    ))
    .expect("压缩包没变时不该下载");
    assert_eq!(
        std::fs::read(base.join("config/a.toml")).unwrap(),
        b"a = 1",
        "没变的配置目录不该被重新解压"
    );
    assert_eq!(std::fs::read(base.join("mods/b.jar")).unwrap(), B_JAR);

    // 版本 3：config.zip 内容变了，必须重新拉取并解压；b.jar 哈希没变，不给地址
    let config_v3 = zip_bytes(&[("a.toml", b"a = 3")]);
    let mut v3 = manifest(
        "1.0.2",
        "21.1.259",
        vec![plain(port, "mods/b.jar", "b2.jar", B_JAR)],
    );
    v3.archive_list = vec![archive_item(port, "config.zip", "config", &config_v3)];
    routes.set(vec![
        ("/pack/server.json", manifest_bytes(&v3)),
        ("/pack/config.zip", config_v3),
    ]);

    block_on(mml_game::add_game::upgrade_server_pack(
        &snapshot,
        None,
        CancellationToken::new(),
        &[],
    ))
    .expect("压缩包变了应下载并解压");
    assert_eq!(
        std::fs::read(base.join("config/a.toml")).unwrap(),
        b"a = 3",
        "压缩包变了要更新配置目录"
    );
}

/// 嵌套压缩包：父包变了、子包没变时只更新父包；子包变了、父包没变时只更新子包
#[test]
fn nested_archives_skip_unchanged_parts() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _run = ensure_init();
    let (routes, port) = Routes::start();

    let parent_v1 = zip_bytes(&[("a.toml", b"a = 1")]);
    let child_v1 = zip_bytes(&[("b.toml", b"b = 1")]);
    let mut v1 = manifest("1.0.0", "21.1.259", Vec::new());
    v1.archive_list = vec![
        archive_item(port, "config.zip", "config", &parent_v1),
        archive_item(port, "config.setting.zip", "config/setting", &child_v1),
    ];
    routes.set(vec![
        ("/pack/server.json", manifest_bytes(&v1)),
        ("/pack/config.zip", parent_v1.clone()),
        ("/pack/config.setting.zip", child_v1.clone()),
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
    let base = game.read().unwrap().get_game_path();
    assert_eq!(std::fs::read(base.join("config/a.toml")).unwrap(), b"a = 1");
    assert_eq!(
        std::fs::read(base.join("config/setting/b.toml")).unwrap(),
        b"b = 1"
    );

    let snapshot = game.read().unwrap().clone();
    let local = snapshot.get_server_pack_file();
    for _ in 0..100 {
        if local.is_file() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(local.is_file(), "旧清单应已落盘");

    // 版本 2：只改父包，服务端不提供没变的子包
    let parent_v2 = zip_bytes(&[("a.toml", b"a = 2")]);
    let mut v2 = manifest("1.0.1", "21.1.259", Vec::new());
    v2.archive_list = vec![
        archive_item(port, "config.zip", "config", &parent_v2),
        archive_item(port, "config.setting.zip", "config/setting", &child_v1),
    ];
    routes.set(vec![
        ("/pack/server.json", manifest_bytes(&v2)),
        ("/pack/config.zip", parent_v2.clone()),
    ]);

    block_on(mml_game::add_game::upgrade_server_pack(
        &snapshot,
        None,
        CancellationToken::new(),
        &[],
    ))
    .expect("只更新父包应成功");
    assert_eq!(std::fs::read(base.join("config/a.toml")).unwrap(), b"a = 2");
    assert_eq!(
        std::fs::read(base.join("config/setting/b.toml")).unwrap(),
        b"b = 1",
        "没变的子包不该重下重解"
    );

    // 版本 3：只改子包，服务端不提供没变的父包
    let child_v3 = zip_bytes(&[("b.toml", b"b = 3")]);
    let mut v3 = manifest("1.0.2", "21.1.259", Vec::new());
    v3.archive_list = vec![
        archive_item(port, "config.zip", "config", &parent_v2),
        archive_item(port, "config.setting.zip", "config/setting", &child_v3),
    ];
    routes.set(vec![
        ("/pack/server.json", manifest_bytes(&v3)),
        ("/pack/config.setting.zip", child_v3),
    ]);

    block_on(mml_game::add_game::upgrade_server_pack(
        &snapshot,
        None,
        CancellationToken::new(),
        &[],
    ))
    .expect("只更新子包应成功");
    assert_eq!(std::fs::read(base.join("config/a.toml")).unwrap(), b"a = 2");
    assert_eq!(
        std::fs::read(base.join("config/setting/b.toml")).unwrap(),
        b"b = 3",
        "子包变了要更新"
    );
}


/// 自定义编号不能被当成平台编号写回在线信息
#[test]
fn custom_ids_are_not_written_to_online_info() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _run = ensure_init();
    let (routes, port) = Routes::start();

    let mut item = plain(port, "mods/local.jar", "local.jar", b"jar");
    item.pid = Some("custom-p-abc".to_string());
    item.fid = Some("custom-f-abc".to_string());
    item.custom = true;
    let v1 = manifest("1.0.0", "21.1.259", vec![item]);
    routes.set(vec![
        ("/pack/server.json", manifest_bytes(&v1)),
        ("/pack/local.jar", b"jar".to_vec()),
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
    let snapshot = game.read().unwrap().clone();
    assert!(
        snapshot.read_online_info().is_empty(),
        "自定义编号不该进在线信息"
    );
}

/// 基础文件包只在第一次同步时下载，后续版本即使清单换了也不再拉取
#[test]
fn base_archive_is_downloaded_once() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _run = ensure_init();
    let (routes, port) = Routes::start();

    let base_v1 = zip_bytes(&[
        ("server.properties", b"motd=v1"),
        (".minecraft/options.txt", b"lang:zh_cn"),
        ("libraries/foo.jar", b"lib"),
        ("patches/a.json", b"patch"),
    ]);
    let mut v1 = manifest("1.0.0", "21.1.259", Vec::new());
    v1.base = Some(ServerBaseObj {
        file: "base.zip".to_string(),
        url: format!("http://127.0.0.1:{port}/pack/base.zip"),
        sha1: Some(gen_hash(HashType::Sha1, &base_v1)),
        ..Default::default()
    });
    routes.set(vec![
        ("/pack/server.json", manifest_bytes(&v1)),
        ("/pack/base.zip", base_v1),
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
    let instance_base = game.read().unwrap().get_base_path();
    assert_eq!(
        std::fs::read(instance_base.join("server.properties")).unwrap(),
        b"motd=v1"
    );
    assert_eq!(
        std::fs::read(instance_base.join(".minecraft/options.txt")).unwrap(),
        b"lang:zh_cn"
    );
    assert_eq!(
        std::fs::read(instance_base.join("libraries/foo.jar")).unwrap(),
        b"lib"
    );
    assert_eq!(
        std::fs::read(instance_base.join("patches/a.json")).unwrap(),
        b"patch"
    );

    let snapshot = game.read().unwrap().clone();
    let local = snapshot.get_server_pack_file();
    for _ in 0..100 {
        if local.is_file() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(local.is_file(), "旧清单应已落盘");

    // 版本 2：基础包换了内容，但旧清单已经有基础包，服务端也不提供地址
    let base_v2 = zip_bytes(&[("server.properties", b"motd=v2")]);
    let mut v2 = manifest("1.0.1", "21.1.259", Vec::new());
    v2.base = Some(ServerBaseObj {
        file: "base.zip".to_string(),
        url: format!("http://127.0.0.1:{port}/pack/base.zip"),
        sha1: Some(gen_hash(HashType::Sha1, &base_v2)),
        ..Default::default()
    });
    routes.set(vec![("/pack/server.json", manifest_bytes(&v2))]);

    block_on(mml_game::add_game::upgrade_server_pack(
        &snapshot,
        None,
        CancellationToken::new(),
        &[],
    ))
    .expect("基础包已下载过，不该再拉取");
    assert_eq!(
        std::fs::read(instance_base.join("server.properties")).unwrap(),
        b"motd=v1",
        "后续版本不该覆盖已经解压的基础文件"
    );
}

/// 等后台线程把本地清单落盘：升级要先看到旧清单，否则算不出该删什么
fn wait_local_manifest(snapshot: &InstanceSettingObj) {
    let file = snapshot.get_server_pack_file();
    for _ in 0..100 {
        if file.is_file() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("旧清单应已落盘");
}

/// 可选模组换升级换文件名时，启用过的保持启用、停用着的保持停用
#[test]
fn optional_mod_keeps_enabled_state_when_renamed() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _run = ensure_init();
    let (routes, port) = Routes::start();

    let a1 = b"opt a v1".to_vec();
    let b1 = b"opt b v1".to_vec();
    let mut a = indexed(port, "mods/a-1.jar", "a1.jar", &a1, "901", "9011");
    a.optional = true;
    let mut b = indexed(port, "mods/b-1.jar", "b1.jar", &b1, "902", "9021");
    b.optional = true;
    let v1 = manifest("1.0.0", "21.1.259", vec![a, b]);
    routes.set(vec![
        ("/pack/server.json", manifest_bytes(&v1)),
        ("/pack/a1.jar", a1.clone()),
        ("/pack/b1.jar", b1.clone()),
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
    let base = game.read().unwrap().get_game_path();

    assert!(base.join("mods/a-1.jar.disabled").is_file());
    assert!(base.join("mods/b-1.jar.disabled").is_file());

    // 玩家启用 a，b 保持停用
    std::fs::rename(
        base.join("mods/a-1.jar.disabled"),
        base.join("mods/a-1.jar"),
    )
    .unwrap();

    // 版本 2：两个模组都换内容、换文件名，项目号不变
    let a2 = b"opt a v2".to_vec();
    let b2 = b"opt b v2".to_vec();
    let mut a2_item = indexed(port, "mods/a-2.jar", "a2.jar", &a2, "901", "9012");
    a2_item.optional = true;
    let mut b2_item = indexed(port, "mods/b-2.jar", "b2.jar", &b2, "902", "9022");
    b2_item.optional = true;
    let v2 = manifest("1.0.1", "21.1.259", vec![a2_item, b2_item]);
    routes.set(vec![
        ("/pack/server.json", manifest_bytes(&v2)),
        ("/pack/a2.jar", a2.clone()),
        ("/pack/b2.jar", b2.clone()),
    ]);

    let snapshot = game.read().unwrap().clone();
    wait_local_manifest(&snapshot);
    block_on(mml_game::add_game::upgrade_server_pack(
        &snapshot,
        None,
        CancellationToken::new(),
        &[],
    ))
    .expect("升级应成功");

    assert_eq!(
        std::fs::read(base.join("mods/a-2.jar")).unwrap(),
        a2,
        "启用过的可选模组换了文件名也要保持启用"
    );
    assert!(!base.join("mods/a-2.jar.disabled").exists());
    assert!(!base.join("mods/a-1.jar").exists(), "旧文件名要删掉");

    assert!(
        base.join("mods/b-2.jar.disabled").is_file(),
        "一直停用着的可选模组换了文件名仍要停用"
    );
    assert!(!base.join("mods/b-2.jar").exists());
    assert_eq!(std::fs::read(base.join("mods/b-2.jar.disabled")).unwrap(), b2);
}

/// 可选模组首次装成禁用；玩家启用后升级保持启用；清单删掉后连禁用文件一起删
#[test]
fn optional_mod_installs_disabled_and_removes_with_pack() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _run = ensure_init();
    let (routes, port) = Routes::start();

    let opt_v1 = b"optional v1".to_vec();
    let keep = b"keep".to_vec();
    let mut opt = plain(port, "mods/opt.jar", "opt.jar", &opt_v1);
    opt.optional = true;
    let v1 = manifest(
        "1.0.0",
        "21.1.259",
        vec![opt, plain(port, "mods/keep.jar", "keep.jar", &keep)],
    );
    routes.set(vec![
        ("/pack/server.json", manifest_bytes(&v1)),
        ("/pack/opt.jar", opt_v1.clone()),
        ("/pack/keep.jar", keep.clone()),
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
    let base = game.read().unwrap().get_game_path();

    assert!(
        !base.join("mods/opt.jar").exists(),
        "可选模组首次装好应是禁用状态"
    );
    assert!(
        base.join("mods/opt.jar.disabled").is_file(),
        "禁用后缀与 MMC / HMCL 一致，用 .disabled"
    );
    assert_eq!(
        std::fs::read(base.join("mods/opt.jar.disabled")).unwrap(),
        opt_v1
    );
    assert!(base.join("mods/keep.jar").is_file(), "普通模组照常启用");

    // 玩家自己启用：去掉禁用后缀
    std::fs::rename(base.join("mods/opt.jar.disabled"), base.join("mods/opt.jar")).unwrap();

    // 版本 2：同一条可选模组换了内容，玩家的启用状态要保留
    let opt_v2 = b"optional v2".to_vec();
    let mut opt_v2_item = plain(port, "mods/opt.jar", "opt2.jar", &opt_v2);
    opt_v2_item.optional = true;
    let v2 = manifest(
        "1.0.1",
        "21.1.259",
        vec![
            opt_v2_item,
            plain(port, "mods/keep.jar", "keep.jar", &keep),
        ],
    );
    routes.set(vec![
        ("/pack/server.json", manifest_bytes(&v2)),
        ("/pack/opt2.jar", opt_v2.clone()),
        ("/pack/keep.jar", keep.clone()),
    ]);

    let snapshot = game.read().unwrap().clone();
    wait_local_manifest(&snapshot);
    block_on(mml_game::add_game::upgrade_server_pack(
        &snapshot,
        None,
        CancellationToken::new(),
        &[],
    ))
    .expect("升级应成功");

    assert_eq!(
        std::fs::read(base.join("mods/opt.jar")).unwrap(),
        opt_v2,
        "玩家启用的可选模组升级后仍是启用状态"
    );
    assert!(!base.join("mods/opt.jar.disable").exists());

    // 版本 3：可选模组从清单删掉，启用 / 禁用两种文件名都要清掉
    let v3 = manifest(
        "1.0.2",
        "21.1.259",
        vec![plain(port, "mods/keep.jar", "keep.jar", &keep)],
    );
    routes.set(vec![
        ("/pack/server.json", manifest_bytes(&v3)),
        ("/pack/keep.jar", keep.clone()),
    ]);

    let snapshot = game.read().unwrap().clone();
    wait_local_manifest(&snapshot);
    block_on(mml_game::add_game::upgrade_server_pack(
        &snapshot,
        None,
        CancellationToken::new(),
        &[],
    ))
    .expect("升级应成功");

    assert!(!base.join("mods/opt.jar").exists(), "启用的可选模组要删掉");
    assert!(
        !base.join("mods/opt.jar.disable").exists(),
        "禁用后缀的那份也要删掉"
    );
    assert!(!base.join("mods/opt.jar.disabled").exists());
    assert!(base.join("mods/keep.jar").is_file());
}

/// 玩家在确认窗里去掉的可选模组：这次不下载，本地旧文件留着；下次重新勾上时旧文件被清掉
#[test]
fn optional_mod_skipped_keeps_local_copy_until_next_update() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _run = ensure_init();
    let (routes, port) = Routes::start();

    let a1 = b"opt a v1".to_vec();
    let keep = b"keep".to_vec();
    let mut a = indexed(port, "mods/a-1.jar", "a1.jar", &a1, "911", "9111");
    a.optional = true;
    let v1 = manifest(
        "1.0.0",
        "21.1.259",
        vec![a, plain(port, "mods/keep.jar", "keep.jar", &keep)],
    );
    routes.set(vec![
        ("/pack/server.json", manifest_bytes(&v1)),
        ("/pack/a1.jar", a1.clone()),
        ("/pack/keep.jar", keep.clone()),
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
    let base = game.read().unwrap().get_game_path();
    assert!(base.join("mods/a-1.jar.disabled").is_file());
    // 玩家自己启用
    std::fs::rename(
        base.join("mods/a-1.jar.disabled"),
        base.join("mods/a-1.jar"),
    )
    .unwrap();

    // 版本 2：可选模组换了内容与文件名，但玩家在弹窗里没勾它
    let a2 = b"opt a v2".to_vec();
    let mut a2_item = indexed(port, "mods/a-2.jar", "a2.jar", &a2, "911", "9112");
    a2_item.optional = true;
    let v2 = manifest(
        "1.0.1",
        "21.1.259",
        vec![a2_item, plain(port, "mods/keep.jar", "keep.jar", &keep)],
    );
    routes.set(vec![
        ("/pack/server.json", manifest_bytes(&v2)),
        ("/pack/a2.jar", a2.clone()),
        ("/pack/keep.jar", keep.clone()),
    ]);

    let snapshot = game.read().unwrap().clone();
    wait_local_manifest(&snapshot);
    block_on(mml_game::add_game::upgrade_server_pack(
        &snapshot,
        None,
        CancellationToken::new(),
        &["mods/a-2.jar".to_string()],
    ))
    .expect("升级应成功");

    assert_eq!(
        std::fs::read(base.join("mods/a-1.jar")).unwrap(),
        a1,
        "没勾的可选模组这次不该被更新"
    );
    assert!(!base.join("mods/a-2.jar").exists(), "没勾的条目不该下载");
    assert!(!base.join("mods/a-2.jar.disabled").exists());

    // 本地清单记下这次跳过，并记着留下的是哪个文件
    let pack = wait_pack_version(&snapshot, "1.0.1");
    assert_eq!(pack.skipped.len(), 1, "跳过记录应写进本地清单");
    assert_eq!(pack.skipped[0].file, "mods/a-2.jar");
    assert_eq!(pack.skipped[0].kept, "mods/a-1.jar");

    // 版本 3：同一模组又换了一版，这次勾上：新文件落地、上次留下的旧文件被清掉
    let a3 = b"opt a v3".to_vec();
    let mut a3_item = indexed(port, "mods/a-3.jar", "a3.jar", &a3, "911", "9113");
    a3_item.optional = true;
    let v3 = manifest(
        "1.0.2",
        "21.1.259",
        vec![a3_item, plain(port, "mods/keep.jar", "keep.jar", &keep)],
    );
    routes.set(vec![
        ("/pack/server.json", manifest_bytes(&v3)),
        ("/pack/a3.jar", a3.clone()),
        ("/pack/keep.jar", keep.clone()),
    ]);

    let snapshot = game.read().unwrap().clone();
    block_on(mml_game::add_game::upgrade_server_pack(
        &snapshot,
        None,
        CancellationToken::new(),
        &[],
    ))
    .expect("升级应成功");

    assert_eq!(
        std::fs::read(base.join("mods/a-3.jar")).unwrap(),
        a3,
        "勾上的可选模组要更新；玩家本来启用着，装好仍是启用"
    );
    assert!(!base.join("mods/a-1.jar").exists(), "上次留下的旧文件要清掉");
    assert!(!base.join("mods/a-2.jar").exists());
    let pack = wait_pack_version(&snapshot, "1.0.2");
    assert!(pack.skipped.is_empty(), "更新后跳过记录应清空");
}

/// 等本地清单落到指定包版本
///
/// - `snapshot`: 实例设置快照，取清单路径用
/// - `version`: 期望的包版本
///
/// # 返回值
///
/// 该版本的清单
fn wait_pack_version(snapshot: &InstanceSettingObj, version: &str) -> ServerPackObj {
    let file = snapshot.get_server_pack_file();
    for _ in 0..100 {
        let pack = std::fs::read(&file)
            .ok()
            .and_then(|data| serde_json::from_slice::<ServerPackObj>(&data).ok());
        if let Some(pack) = pack
            && pack.pack_version == version
        {
            return pack;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("本地清单没有落到版本 {version}");
}
