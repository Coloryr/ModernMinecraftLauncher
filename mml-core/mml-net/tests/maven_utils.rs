//! `version_name_to_path` 的纯函数测试，以及 `try_get_hash` 的本地服务测试。
//!
//! 转换规则：
//! - `"group:artifact:version"` → `"group/artifact/version/artifact-version.jar"`
//! - `"group:artifact:version:ext"` → `"group/artifact/version/artifact-version-ext.jar"`
//! - 少于 3 段：`"name.name"` → `"name/name.jar"`
//!
//! `try_get_hash` 那几条起本地服务模拟 Maven 仓库：Forge 的 `1.12.2-14.23.5.2864` 只发
//! `.sha1`，缺的 `.sha256` 回 404 加一页 HTML。曾经这页 HTML 会被当成"预期校验值"收下，
//! 于是明明能下的文件被判校验失败。

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Once};

use mml_base::file_item::FileHash;
use mml_config::config_obj::ProxyState;
use mml_net::maven_utils::{try_get_hash, version_name_to_path};

/// 标准三段坐标。
#[test]
fn three_part_coordinate() {
    assert_eq!(
        version_name_to_path("com.example:artifact:1.0"),
        "com/example/artifact/1.0/artifact-1.0.jar"
    );
    assert_eq!(
        version_name_to_path("net.minecraftforge:forge:1.20.4"),
        "net/minecraftforge/forge/1.20.4/forge-1.20.4.jar"
    );
    assert_eq!(
        version_name_to_path("org.lwjgl:lwjgl:3.3.1"),
        "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1.jar"
    );
}

/// 四段坐标，带 classifier
#[test]
fn four_part_coordinate() {
    assert_eq!(
        version_name_to_path("com.example:artifact:1.0:sources"),
        "com/example/artifact/1.0/artifact-1.0-sources.jar"
    );
    assert_eq!(
        version_name_to_path("org.lwjgl:lwjgl:3.3.1:natives-windows"),
        "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows.jar"
    );
}

/// 少于 3 段的非坐标名称
#[test]
fn non_coordinate_name() {
    assert_eq!(version_name_to_path("plain.name"), "plain/name.jar");
    assert_eq!(version_name_to_path("solo"), "solo.jar");
    assert_eq!(version_name_to_path("a.b.c"), "a/b/c.jar");
    assert_eq!(version_name_to_path(""), ".jar");
}

// ============================================================================
// try_get_hash：起本地服务当 Maven 仓库
// ============================================================================

/// 一个合法的 SHA1 校验值，故意写成大写以顺带验证归一化
const SHA1_HASH: &str = "0123456789ABCDEF0123456789ABCDEF01234567";

/// 一个合法的 SHA256 校验值
const SHA256_HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// 模拟仓库回错误时那页 HTML
const ERROR_PAGE: &str = "<html><body><h1>404 Not Found</h1></body></html>";

/// 本地服务对一条请求的响应
struct FakeResponse {
    /// HTTP 状态码
    status: u16,
    /// 响应正文
    body: &'static str,
}

/// 成功响应
fn ok(body: &'static str) -> FakeResponse {
    FakeResponse { status: 200, body }
}

/// 404 加一页 HTML 错误页，即仓库缺这个文件时的回法
fn missing() -> FakeResponse {
    FakeResponse {
        status: 404,
        body: ERROR_PAGE,
    }
}

/// 起一个本地服务，按请求路径决定回什么
///
/// - `respond`: 收请求路径，返回响应
///
/// # 返回值
///
/// 返回监听端口
fn spawn_server<F>(respond: F) -> u16
where
    F: Fn(&str) -> FakeResponse + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定失败");
    let port = listener.local_addr().unwrap().port();
    let respond = Arc::new(respond);

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let respond = Arc::clone(&respond);
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let Ok(n) = stream.read(&mut buf) else { return };
                let head = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = head.split_whitespace().nth(1).unwrap_or("/").to_string();
                let FakeResponse { status, body } = respond(&path);
                let reason = if status == 200 { "OK" } else { "Not Found" };
                let resp = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes());
                let _ = stream.flush();
            });
        }
    });

    port
}

/// 初始化内核配置与 HTTP 客户端，每进程一次
///
/// 内核那几个系统是进程级单例，重复 init 会 panic；同一个测试二进制里各用例共用这一份
fn init_once() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let dir =
            mml_testutil::temp_dir().join(format!("mml-net-maven-hash-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("创建配置临时目录失败");
        // 日志系统要在 `mml_net::init` 之前起：后者会打一条客户端初始化日志
        let _ = mml_log::start(&dir);
        mml_config::init(&dir).expect("配置初始化失败");
        // 显式禁代理：默认是跟随系统代理，本机开着代理时本地服务的请求会连不上
        mml_config::write_config().http.work_proxy = ProxyState::None;
        mml_net::init();
    });
}

/// 在单线程运行时里跑一次 `try_get_hash`
///
/// - `url`: 文件 URL
///
/// # 返回值
///
/// 见 [`FileHash`]
fn try_hash(url: &str) -> FileHash {
    init_once();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(try_get_hash(url))
}

/// `.sha256` 缺失时，仓库回 404 加一页 HTML，必须回退到存在的 `.sha1`
#[test]
fn falls_back_when_sha256_missing() {
    let port = spawn_server(|path| {
        if path.ends_with(".sha1") {
            ok(SHA1_HASH)
        } else {
            missing()
        }
    });

    match try_hash(&format!("http://127.0.0.1:{port}/a.jar")) {
        FileHash::Sha1(value) => assert_eq!(value, SHA1_HASH.to_lowercase()),
        other => panic!("应回退到 SHA1，实际 {other:?}"),
    }
}

/// 仓库用 200 回错误页时也不能当成校验值，同样要继续回退
#[test]
fn rejects_error_page_with_success_status() {
    let port = spawn_server(|path| {
        if path.ends_with(".sha256") {
            ok(ERROR_PAGE)
        } else if path.ends_with(".sha1") {
            ok(SHA1_HASH)
        } else {
            missing()
        }
    });

    match try_hash(&format!("http://127.0.0.1:{port}/a.jar")) {
        FileHash::Sha1(value) => assert_eq!(value, SHA1_HASH.to_lowercase()),
        other => panic!("错误页不该被当成校验值，实际 {other:?}"),
    }
}

/// 三种校验文件都拿不到时不报错，按"没有可校验的值"处理
#[test]
fn none_when_all_hash_files_missing() {
    let port = spawn_server(|_| missing());

    let hash = try_hash(&format!("http://127.0.0.1:{port}/a.jar"));
    assert!(matches!(hash, FileHash::None), "实际 {hash:?}");
}

/// 有 `.sha256` 时优先用它
#[test]
fn prefers_sha256() {
    let port = spawn_server(|path| {
        if path.ends_with(".sha256") {
            ok(SHA256_HASH)
        } else {
            ok(SHA1_HASH)
        }
    });

    match try_hash(&format!("http://127.0.0.1:{port}/a.jar")) {
        FileHash::Sha256(value) => assert_eq!(value, SHA256_HASH),
        other => panic!("应优先用 SHA256，实际 {other:?}"),
    }
}
