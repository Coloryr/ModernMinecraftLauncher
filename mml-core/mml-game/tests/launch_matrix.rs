//! 启动矩阵测试：原版 / Forge / Fabric / NeoForge / Cleanroom / LiteLoader × 26.3 / 1.12.2 / 1.7.10
//!
//! Cleanroom 只有 1.12.2、LiteLoader 只有 1.7.10 与 1.12.2，其余格子记「不适用」，不算失败。
//!
//! 每一格都走完整链路：
//!
//! 1. **下载游戏** —— 版本 json、客户端 jar、运行库、加载器文件（Forge / NeoForge 的安装器与库）；
//! 2. **检查资源** —— 资源索引必查；资源对象默认跳过（一个版本上百 MB），
//!    想看完整检查就设 `MML_TEST_ASSETS=1`；
//! 3. **启动游戏** —— 真起进程。主类换成探针（javac 现编译），所以不会真开游戏窗口，
//!    但类路径、参数、Java、下载、日志、退出事件全是真的。
//!
//! # 想单跑一格 / 打断点
//!
//! 链路都在 [`launch`] 里，直接调它就行（`launch_one` 就是这么一个入口，把里面的参数
//! 改成你要的那一格，在 `launch` 里下断点）：
//!
//! ```text
//! launch("1.12.2", LoaderType::Forge, "14.23.5.2864").await
//! launch_auto("26.3", LoaderType::NeoForge).await   // 加载器版本自己去上游查
//! ```
//!
//! 记三种结果：
//!
//! - `不适用`：上游没有该组合（如 1.7.10 没有 Fabric），或查不到加载器版本；
//! - `缺 Java`：本机没有该版本要的 Java（1.7.10 / 1.12.2 要 Java 8）——下载与检查照跑，
//!   只有最后起进程这一步跳过；
//! - `失败`：其它错误，算真失败，会汇总在末尾并让用例不通过。
//!
//! `#[ignore]`：联网 + 首次要下几百 MB，缓存落在 `target/temp/mml-launch-matrix`。
//! 走本机代理（默认 `127.0.0.1:7890`），`MML_TEST_PROXY=off` 关掉。
//!
//! ```text
//! cargo test -p mml-game --test launch_matrix -- --ignored --nocapture
//! $env:MML_MATRIX = "normal:26.3,forge:1.12.2"   # 只跑指定几格
//! ```

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, Once, OnceLock};
use std::time::{Duration, Instant};

use mml_auth::LoginObj;
use mml_config::config_obj::{GCType, ProxyState, ProxyType, RunArgObj};
use mml_downloader::{DownloadTaskState, IDownloadGui, download_item::DownloadItem};
use mml_game::game_launch::{AutoJoinType, GameLaunchArg};
use mml_game::launcher::instance_setting_obj::{AdvanceJvmObj, InstanceSettingObj};
use mml_game::loader::LoaderType;
use mml_game::loader::loader_versions::get_loader_versions;
use mml_names::i18_items::error_type::ErrorType;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// 要覆盖的游戏版本
const VERSIONS: [&str; 3] = ["26.3", "1.12.2", "1.7.10"];

/// 要覆盖的加载器
const LOADERS: [LoaderType; 6] = [
    LoaderType::Normal,
    LoaderType::Forge,
    LoaderType::Fabric,
    LoaderType::NeoForge,
    LoaderType::Cleanroom,
    LoaderType::LiteLoader,
];

/// 探针：打印类路径与游戏参数，并同样写进工作目录
const PROBE: &str = r#"import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;

public class MmlLaunchProbe {
    public static void main(String[] args) throws Exception {
        String line = "mml-probe-ok java=" + System.getProperty("java.version") + " cp=" + System.getProperty("java.class.path") + " args=" + String.join(" ", args);
        System.out.println(line);
        Files.write(Paths.get("probe-args.txt"), line.getBytes(StandardCharsets.UTF_8));
    }
}
"#;

/// 一格的结论
pub struct Outcome {
    /// 加载器:版本
    pub combo: String,
    /// 加载器版本，原版为空
    pub loader_version: String,
    /// 下载与检查的结论
    pub check: String,
    /// 启动的结论
    pub launch: String,
    /// 是否算真失败
    pub failed: bool,
}

/// 每进程一次的公共准备：内核、探针、退出事件收集
struct Ctx {
    /// 探针主类名
    main: String,
    /// 探针类路径目录
    classes: PathBuf,
    /// 已退出的实例 uuid
    exited: Arc<Mutex<HashSet<Uuid>>>,
}

/// 下载回调：什么都不做
struct Quiet {}

impl IDownloadGui for Quiet {
    fn update(&self, _thread: u32, _file: &Arc<DownloadItem>) {}
    fn update_task(&self, _state: DownloadTaskState) {}
}

/// 矩阵与实例文件都留在这个目录里，重复跑不必再下一遍
fn run_dir() -> PathBuf {
    mml_testutil::temp_dir().join("mml-launch-matrix")
}

/// 公共准备，每进程一次
///
/// # 返回值
///
/// 没有可用 JDK 时返回 `None`，用例据此跳过
fn ctx() -> Option<&'static Ctx> {
    static CTX: OnceLock<Option<Ctx>> = OnceLock::new();
    CTX.get_or_init(|| {
        ensure_init();
        let (_, javac) = find_jdk()?;
        let (main, classes) = build_probe(&javac);

        let exited: Arc<Mutex<HashSet<Uuid>>> = Arc::new(Mutex::new(HashSet::new()));
        let sink = exited.clone();
        mml_game::add_exit(move |event| {
            sink.lock().unwrap().insert(event.uuid);
        });

        Some(Ctx {
            main,
            classes,
            exited,
        })
    })
    .as_ref()
}

/// 走本机代理：默认 `127.0.0.1:7890`
///
/// `MML_TEST_PROXY=off` 关掉，`MML_TEST_PROXY=host:port` 换地址。必须在 `mml_net::init()`
/// 之前设：客户端是拿配置建出来的，建完再改就不生效了。
fn apply_proxy() {
    let proxy = std::env::var("MML_TEST_PROXY").unwrap_or_else(|_| "127.0.0.1:7890".to_string());
    if proxy.eq_ignore_ascii_case("off") {
        return;
    }
    let (ip, port) = proxy.split_once(':').unwrap_or(("127.0.0.1", "7890"));
    let port = port.parse().unwrap_or(7890);

    let mut config = mml_config::write_config();
    config.http.work_proxy = ProxyState::User;
    config.http.work_proxy_type = ProxyType::Http;
    config.http.login_proxy = ProxyState::User;
    config.http.login_proxy_type = ProxyType::Http;
    config.http.proxy_ip = ip.to_string();
    config.http.proxy_port = port;
}

/// 初始化链，与 `mml_core::init` 相同，每进程一次
fn ensure_init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let dir = run_dir();
        std::fs::create_dir_all(&dir).unwrap();

        // 启动流程会走 auth_login → LoginObj::save()，账户落到 inner_path
        // （LOCALAPPDATA 或 HOME）；不重定向就会把真实用户的 auth.json 覆盖成测试账户
        let dir_str = dir.to_string_lossy().to_string();
        unsafe { std::env::set_var("LOCALAPPDATA", &dir_str) };
        unsafe { std::env::set_var("HOME", &dir_str) };

        mml_base::init(&dir);
        mml_names::init(mml_base::get_base_dir()).unwrap();
        mml_log::start(mml_base::get_base_dir()).unwrap();
        mml_config::init(mml_base::get_base_dir()).unwrap();
        mml_config::config_save::start();
        apply_proxy();
        mml_net::init();
        mml_downloader::set_gui_handel(Box::new(Quiet {}));
        mml_downloader::init(mml_base::get_base_dir()).unwrap();
        mml_downloader::start();
        mml_game::init(mml_base::get_base_dir()).unwrap();
        // 退出轮询线程由 load() 起，实例退出事件就是它发的
        mml_game::load().unwrap();

        // 资源对象默认不查：一个版本上千个小文件、上百 MB，与"能不能起进程"无关
        let mut config = mml_config::write_config();
        config.check.assets = std::env::var("MML_TEST_ASSETS").is_ok();
        config.check.core = true;
        config.check.lib = true;
    });
}

/// 编译探针，返回主类名与类路径目录
fn build_probe(javac: &Path) -> (String, PathBuf) {
    let base = mml_base::get_base_dir();
    let src_dir = base.join("probe");
    std::fs::create_dir_all(&src_dir).unwrap();
    let src = src_dir.join("MmlLaunchProbe.java");
    std::fs::write(&src, PROBE).unwrap();

    let classes = base.join("probe-classes");
    std::fs::create_dir_all(&classes).unwrap();
    let out = Command::new(javac)
        // 探针要能跟着 1.7.10 / 1.12.2 在 Java 8 上跑，所以按 Java 8 字节码编译
        .args(["-source", "8", "-target", "8"])
        .arg("-d")
        .arg(&classes)
        .arg(&src)
        .output()
        .expect("javac 应能运行");
    assert!(
        out.status.success(),
        "探针编译失败：{}",
        String::from_utf8_lossy(&out.stderr)
    );

    ("MmlLaunchProbe".to_string(), classes)
}

/// 找带 `jdk.compiler` 的 JDK（探针要现编译）
fn find_jdk() -> Option<(PathBuf, PathBuf)> {
    let exe = if cfg!(windows) { ".exe" } else { "" };

    let mut homes = Vec::new();
    if let Ok(home) = std::env::var("JAVA_HOME") {
        homes.push(PathBuf::from(home));
    }
    if let Ok(out) = Command::new("java")
        .args(["-XshowSettings:properties", "-version"])
        .output()
    {
        for line in String::from_utf8_lossy(&out.stderr).lines() {
            if let Some(path) = line.trim().strip_prefix("java.home = ") {
                homes.push(PathBuf::from(path));
            }
        }
    }

    homes.into_iter().find_map(|home| {
        let java = home.join("bin").join(format!("java{exe}"));
        let javac = home.join("bin").join(format!("javac{exe}"));
        (java.is_file() && javac.is_file()).then_some((java, javac))
    })
}

/// 目录下所有文件
fn list_files(dir: &Path, acc: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            list_files(&path, acc);
        } else {
            acc.push(path);
        }
    }
}

/// 跑一格：下载 → 检查资源 → 启动
///
/// 想在某一格下断点，直接调它即可；实例目录按 `加载器-游戏版本` 复用，重复跑很快。
///
/// - `version`: 游戏版本，如 `1.12.2`
/// - `loader`: 加载器；原版传 [`LoaderType::Normal`]
/// - `loader_version`: 加载器版本，如 `14.23.5.2864`；原版传空串
///
/// # 返回值
///
/// 见 [`Outcome`]
pub async fn launch(version: &str, loader: LoaderType, loader_version: &str) -> Outcome {
    let Some(ctx) = ctx() else {
        return Outcome {
            combo: format!("{}:{version}", loader.id()),
            loader_version: loader_version.to_string(),
            check: "跳过".into(),
            launch: "跳过（本机没有可用的 JDK）".into(),
            failed: false,
        };
    };

    let base = mml_base::get_base_dir();
    let uuid = Uuid::new_v4();
    let dir = format!("{}-{version}", loader.id());
    let instance = InstanceSettingObj {
        uuid,
        name: dir.clone(),
        dir: dir.clone(),
        version: version.to_string(),
        loader,
        loader_version: (!loader_version.is_empty()).then(|| loader_version.to_string()),
        // 主类换探针：参数与类路径照旧，只是不真跑游戏
        advance_jvm: Some(AdvanceJvmObj {
            main_class: Some(ctx.main.clone()),
            class_path: Some(ctx.classes.to_string_lossy().to_string()),
        }),
        jvm_arg: Some(RunArgObj {
            min_memory: Some(256),
            max_memory: Some(512),
            gc_mode: Some(GCType::None),
            ..Default::default()
        }),
        ..Default::default()
    };

    // ---- 下载游戏 + 检查资源 ----
    let mut arg = GameLaunchArg {
        auth: LoginObj::default(),
        auto: AutoJoinType::None,
        admin: false,
        mixin: None,
        lang: None,
    };
    let check_started = Instant::now();
    let check = match instance
        .check_game_file(&mut arg, None, CancellationToken::new())
        .await
    {
        Ok(obj) => {
            let mut files = Vec::new();
            list_files(&base, &mut files);
            let client = files.iter().any(|path| {
                path.to_string_lossy()
                    .contains(&format!("client-{version}.jar"))
            });
            let index = files
                .iter()
                .any(|path| path.components().any(|part| part.as_os_str() == "indexes"));
            let objects = files
                .iter()
                .filter(|path| path.components().any(|part| part.as_os_str() == "objects"))
                .count();
            let jars = files
                .iter()
                .filter(|path| path.extension().is_some_and(|ext| ext == "jar"))
                .count();

            format!(
                "ok（主类 {}，客户端 {}，索引 {}，jar {jars}，资源对象 {objects}，{:?}）",
                obj.main_class,
                if client { "在" } else { "缺" },
                if index { "在" } else { "缺" },
                check_started.elapsed()
            )
        }
        Err(err) => {
            let text = format!("失败 {err:?}");
            return Outcome {
                combo: format!("{}:{version}", loader.id()),
                loader_version: loader_version.to_string(),
                check: text,
                launch: "未跑".into(),
                failed: true,
            };
        }
    };

    // ---- 启动游戏 ----
    let launched = Instant::now();
    // 上一次运行的凭据不能当本次结果：否则进程根本没跑起来也会被旧文件蒙混过关
    let receipt = base
        .join("minecraft/instances")
        .join(&dir)
        .join(".minecraft/probe-args.txt");
    let _ = std::fs::remove_file(&receipt);
    let (launch, failed) = match instance
        .start_game(&mut arg, None, CancellationToken::new())
        .await
    {
        Ok(()) => {
            // 等这一格的进程退出：探针跑完就退，给 5 分钟
            let deadline = Instant::now() + Duration::from_secs(300);
            while !ctx.exited.lock().unwrap().contains(&uuid) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(200));
            }

            match std::fs::read_to_string(&receipt) {
                Ok(text) => {
                    let loader_mark = match loader {
                        LoaderType::Forge => "forge",
                        LoaderType::NeoForge => "neoforge",
                        LoaderType::Fabric => "fabric",
                        LoaderType::Cleanroom => "cleanroom",
                        // LiteLoader 的类路径里不一定带本体，但 --tweakClass 一定带包名
                        LoaderType::LiteLoader => "liteloader",
                        _ => "",
                    };
                    let loader_ok =
                        loader_mark.is_empty() || text.to_lowercase().contains(loader_mark);
                    // `--version` 的值：原版就是游戏版本，加载器是"加载器-游戏版本-加载器版本"
                    // 一类的名字，所以只要求它带着游戏版本
                    let version_ok =
                        version_arg(&text).is_some_and(|value| value.contains(version));
                    let exited_ok = ctx.exited.lock().unwrap().contains(&uuid);
                    let ok = loader_ok && version_ok && exited_ok;
                    (
                        format!(
                            "{}（版本参数 {}，加载器类路径 {}，退出事件 {}，{:?}）",
                            if ok { "ok" } else { "参数不对" },
                            if version_ok { "对" } else { "错" },
                            if loader_ok { "有" } else { "无" },
                            if exited_ok { "有" } else { "无" },
                            launched.elapsed()
                        ),
                        !ok,
                    )
                }
                Err(err) => (format!("未读到凭据：{err}"), true),
            }
        }
        Err(ErrorType::JavaNotFound) => (
            format!("缺 Java（该版本要 Java {}）", java_hint(version, loader)),
            false,
        ),
        Err(err) => (format!("失败 {err:?}"), true),
    };

    Outcome {
        combo: format!("{}:{version}", loader.id()),
        loader_version: loader_version.to_string(),
        check,
        launch,
        failed,
    }
}

/// 与 [`launch`] 相同，只是加载器版本自己去上游查
///
/// - `version`: 游戏版本
/// - `loader`: 加载器；原版直接走 [`launch`]
///
/// # 返回值
///
/// 见 [`Outcome`]；上游没有该组合时是「不适用」，不算失败
pub async fn launch_auto(version: &str, loader: LoaderType) -> Outcome {
    // 查加载器版本要联网，所以不能等 launch() 里的 ctx() 再初始化：
    // 单跑 run_cleanroom / run_liteloader 时这里是第一条路径
    ensure_init();
    if loader == LoaderType::Normal {
        return launch(version, loader, "").await;
    }

    let combo = format!("{}:{version}", loader.id());
    let not_applicable = |reason: String| Outcome {
        combo: combo.clone(),
        loader_version: String::new(),
        check: "不适用".into(),
        launch: reason,
        failed: false,
    };

    match get_loader_versions(&loader, version).await {
        // 新版本在前，取最新的那个
        Ok(list) if !list.is_empty() => launch(version, loader, &list[0]).await,
        Ok(_) => not_applicable("上游没有该组合".into()),
        Err(err) => not_applicable(format!("查加载器版本失败：{err:?}")),
    }
}

/// 取命令行里 `--version` 后面那个值
///
/// - `text`: 探针记下的整条游戏参数
///
/// # 返回值
///
/// 有 `--version` 且后面还有值时返回该值
fn version_arg(text: &str) -> Option<&str> {
    let mut parts = text.split_whitespace();
    while let Some(part) = parts.next() {
        if part == "--version" {
            return parts.next();
        }
    }
    None
}

/// 该版本要求的 Java 主版本，仅用于跳过时的提示文案
///
/// - `version`: 游戏版本
/// - `loader`: 加载器；Cleanroom 的清单要求 Java 25，覆盖 1.12.2 惯用的 8
///
/// # 返回值
///
/// 返回该版本惯用的 Java 主版本
fn java_hint(version: &str, loader: LoaderType) -> &'static str {
    if loader == LoaderType::Cleanroom {
        return "25";
    }
    match version {
        "1.6.4" | "1.7.10" | "1.12.2" => "8",
        _ => "17+",
    }
}

/// 该不该跑这一格：`MML_MATRIX=loader:version,...` 过滤，没设就全跑
///
/// - `loader`: 加载器
/// - `version`: 游戏版本
///
/// # 返回值
///
/// 该跑返回 `true`
fn wanted(loader: &LoaderType, version: &str) -> bool {
    let Ok(filter) = std::env::var("MML_MATRIX") else {
        return true;
    };
    let key = format!("{}:{version}", loader.id());
    filter.split(',').any(|item| item.trim() == key)
}

/// 原版各版的执行体，给 [`launch_one_normal`] 与 [`launch_one_auto`] 共用
async fn run_normal() {
    report(launch("26.3", LoaderType::Normal, "").await);
    report(launch("1.21.1", LoaderType::Normal, "").await);
    report(launch("1.16.5", LoaderType::Normal, "").await);
    report(launch("1.12.2", LoaderType::Normal, "").await);
    report(launch("1.7.10", LoaderType::Normal, "").await);
    report(launch("1.6.4", LoaderType::Normal, "").await);
}

/// 1.13 以上的 Forge 执行体
async fn run_forge_new() {
    report(launch("1.14.4", LoaderType::Forge, "28.2.30").await);
    report(launch("1.16.5", LoaderType::Forge, "36.2.42").await);
    report(launch("1.20.1", LoaderType::Forge, "47.4.28").await);
    report(launch("1.20.6", LoaderType::Forge, "50.2.13").await);
    report(launch("1.21.1", LoaderType::Forge, "52.1.19").await);
    report(launch("1.21.6", LoaderType::Forge, "56.0.9").await);
    report(launch("1.21.11", LoaderType::Forge, "61.2.3").await);
    report(launch("26.1.2", LoaderType::Forge, "64.1.4").await);
    report(launch("26.3", LoaderType::Forge, "66.0.11").await);
}

/// Forge 1.12.2 各版执行体
async fn run_forge_1122() {
    report(launch("1.12.2", LoaderType::Forge, "14.23.5.2864").await);
    report(launch("1.12.2", LoaderType::Forge, "14.23.5.2859").await);
    report(launch("1.12.2", LoaderType::Forge, "14.23.1.2592").await);
    report(launch("1.12.2", LoaderType::Forge, "14.23.0.2535").await);
    report(launch("1.12.2", LoaderType::Forge, "14.23.0.2486").await);
}

/// 1.12 以下的 Forge 执行体
async fn run_forge_old() {
    report(launch("1.7.10", LoaderType::Forge, "10.13.4.1614").await);
    report(launch("1.8", LoaderType::Forge, "11.14.4.1577").await);
    report(launch("1.8.9", LoaderType::Forge, "11.15.1.2318").await);
    report(launch("1.9.4", LoaderType::Forge, "12.17.0.2317").await);
    report(launch("1.10.2", LoaderType::Forge, "12.18.3.2511").await);
}

/// Fabric 执行体
async fn run_fabric() {
    report(launch("26.3", LoaderType::Fabric, "0.19.5").await);
    report(launch("26.1", LoaderType::Fabric, "0.19.5").await);
    report(launch("1.21.11", LoaderType::Fabric, "0.19.5").await);
    report(launch("1.20.1", LoaderType::Fabric, "0.19.5").await);
    report(launch("1.16.5", LoaderType::Fabric, "0.19.5").await);
}

/// NeoForge 执行体
async fn run_neoforge() {
    report(launch("26.3", LoaderType::NeoForge, "26.3.0.83").await);
    report(launch("26.1.1", LoaderType::NeoForge, "26.1.1.15-beta").await);
    report(launch("1.21.11", LoaderType::NeoForge, "21.11.48").await);
    report(launch("1.21.6", LoaderType::NeoForge, "21.6.20-beta").await);
    report(launch("1.21.1", LoaderType::NeoForge, "21.1.262").await);
    report(launch("1.20.6", LoaderType::NeoForge, "20.6.144").await);
    report(launch("1.20.4", LoaderType::NeoForge, "20.4.253").await);
    report(launch("1.20.2", LoaderType::NeoForge, "20.2.93").await);
}

/// Cleanroom 的唯一一格，给单跑测试与总入口共用
async fn cleanroom_cell() {
    report(launch_auto("1.12.2", LoaderType::Cleanroom).await);
}

/// 只跑 Cleanroom：1.12.2
#[ignore = "联网 + 会下载 / 启动 Cleanroom 实例"]
#[tokio::test]
async fn run_cleanroom() {
    cleanroom_cell().await;
}

/// LiteLoader 的两格，给单跑测试与总入口共用
async fn liteloader_cells() {
    report(launch_auto("1.12.2", LoaderType::LiteLoader).await);
    report(launch_auto("1.7.10", LoaderType::LiteLoader).await);
}

/// 只跑 LiteLoader：1.12.2 与 1.7.10
#[ignore = "联网 + 会下载 / 启动 LiteLoader 实例"]
#[tokio::test]
async fn run_liteloader() {
    liteloader_cells().await;
}

/// 单格调试总入口：把全部单格串起来跑一遍
///
/// `#[tokio::test]` 宏把测试函数展开成同步函数，直接互相调用既不能
/// `.await`、又会在运行时里再嵌一个运行时，所以这里只能串"执行体"；
/// 想钻某一格先单独跑它的测试入口（如 `run_cleanroom` / `run_liteloader`），能省前面格子的耗时
#[ignore = "联网 + 首次要下几百 MB，串行跑全格"]
#[tokio::test]
async fn launch_one_auto() {
    run_normal().await;
    run_forge_new().await;
    run_forge_1122().await;
    run_forge_old().await;
    run_fabric().await;
    run_neoforge().await;
    cleanroom_cell().await;
    liteloader_cells().await;
}

/// 打印一格的结论；算真失败就让用例不过
///
/// - `outcome`: [`launch`] / [`launch_auto`] 的结论
fn report(outcome: Outcome) {
    eprintln!(
        "[矩阵] {}（加载器 {}）：下载/检查 {}；启动 {}",
        outcome.combo, outcome.loader_version, outcome.check, outcome.launch
    );
    assert!(!outcome.failed, "{} 失败了", outcome.combo);
}

/// 启动矩阵：下载 → 检查资源 → 启动，逐格跑完并汇总
#[ignore = "在线矩阵测试：会下载各版本 / 各加载器的文件并真起进程，用 -- --ignored 显式跑"]
#[tokio::test]
async fn launch_matrix() {
    if ctx().is_none() {
        eprintln!("跳过矩阵：本机没有可用的 JDK");
        return;
    }

    let mut outcomes: Vec<Outcome> = Vec::new();
    for loader in LOADERS {
        for version in VERSIONS {
            if !wanted(&loader, version) {
                continue;
            }
            eprintln!("[矩阵] ==== {}:{version} ====", loader.id());
            let outcome = launch_auto(version, loader).await;
            eprintln!("[矩阵]   下载 / 检查：{}", outcome.check);
            eprintln!("[矩阵]   启动：{}", outcome.launch);
            outcomes.push(outcome);
        }
    }

    // ---- 汇总 ----
    eprintln!("\n[矩阵] 结果：");
    eprintln!(
        "{:<20} {:<24} {:<18} {}",
        "组合", "加载器版本", "下载/检查", "启动"
    );
    for item in &outcomes {
        eprintln!(
            "{:<20} {:<24} {:<18} {}{}",
            item.combo,
            if item.loader_version.is_empty() {
                "-".to_string()
            } else {
                item.loader_version.clone()
            },
            if item.check.chars().count() > 16 {
                format!("{}…", item.check.chars().take(16).collect::<String>())
            } else {
                item.check.clone()
            },
            item.launch,
            if item.failed { "  ← 失败" } else { "" }
        );
    }

    let launched = outcomes
        .iter()
        .filter(|item| item.launch.starts_with("ok"))
        .count();
    let failed: Vec<&str> = outcomes
        .iter()
        .filter(|item| item.failed)
        .map(|item| item.combo.as_str())
        .collect();
    eprintln!(
        "[矩阵] 共 {} 格：启动成功 {launched}，失败 {}",
        outcomes.len(),
        failed.len()
    );

    assert!(failed.is_empty(), "这些组合失败了：{failed:?}");
    assert!(launched > 0, "至少有一格要能真启动起来（当前 0 格）");
}
