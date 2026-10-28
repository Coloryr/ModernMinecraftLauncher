//! 真实下载与启动测试：文件真下、进程真起
//!
//! 两条用例都走生产链路，且都要联网、都会真的往磁盘写文件：
//!
//! - `real_download_prepares_game_files`：只用 `InstanceSettingObj::check_game_file`，
//!   把版本 json、客户端 jar、运行库、资源索引**真的下载**下来，然后核对落盘；
//!   不起进程，也不需要本机有 Java。
//! - `real_launch_downloads_and_starts`：走 `start_game` 整条 —— 检查文件、拼命令行、
//!   起进程、读输出、发退出事件。唯一"假"的是主类：用 `AdvanceJvmObj.main_class` 换成
//!   探针（源文件模式的小 Java 程序），于是不必真开游戏窗口，也能断言参数与类路径都是真的。
//!
//! **两条都带 `#[ignore]`**：首次跑要下载几十 MB（客户端 jar + 运行库）。下载结果留在
//! `target/temp/mml-real-launch`，重复跑基本只补缺的文件。显式运行：
//!
//! ```text
//! cargo test -p mml-game --test real_launch -- --ignored --nocapture
//! ```
//!
//! 启动那条需要本机有带 `jdk.compiler` 的 JDK（探针走源文件模式），没有就跳过。

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, Once};
use std::time::{Duration, Instant};

use mml_auth::LoginObj;
use mml_config::config_obj::{GCType, ProxyState, ProxyType, RunArgObj};
use mml_downloader::{DownloadTaskState, IDownloadGui, download_item::DownloadItem};
use mml_game::game_launch::{AutoJoinType, GameLaunchArg};
use mml_game::game_log::GameLog;
use mml_game::launcher::instance_setting_obj::{AdvanceJvmObj, InstanceSettingObj};
use mml_game::{InstanceLog, InstanceLogType};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// 测试用的游戏版本：客户端 jar 二十几 MB、运行库几十个文件，够真又不算太重
const VERSION: &str = "1.20.1";

/// 探针输出里的标记
const MARK: &str = "mml-real-launch-ok";

/// 下载与实例文件都留在这个目录里，重复跑不必再下一遍
fn run_dir() -> PathBuf {
    mml_testutil::temp_dir().join("mml-real-launch")
}

/// 下载回调：测试里不关心进度，全部吞掉
struct QuietDownload {}

impl IDownloadGui for QuietDownload {
    /// 单文件进度：不输出
    fn update(&self, _thread: u32, _file: &Arc<DownloadItem>) {}

    /// 任务级状态：不输出
    fn update_task(&self, _state: DownloadTaskState) {}
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

        mml_base::init(&dir);
        mml_names::init(mml_base::get_base_dir()).unwrap();
        mml_log::start(mml_base::get_base_dir()).unwrap();
        mml_config::init(mml_base::get_base_dir()).unwrap();
        mml_config::config_save::start();
        apply_proxy();
        mml_net::init();
        mml_downloader::set_gui_handel(Box::new(QuietDownload {}));
        mml_downloader::init(mml_base::get_base_dir()).unwrap();
        mml_downloader::start();
        mml_game::init(mml_base::get_base_dir()).unwrap();
        // 退出轮询线程由 load() 起，实例退出事件就是它发的
        mml_game::load().unwrap();

        // 资源对象是几千个小文件、上百 MB，与"版本 json + 客户端 + 运行库"这条链无关，跳过；
        // 客户端与运行库保持开启，本用例要验的正是它们真的被下载下来
        let mut config = mml_config::write_config();
        config.check.assets = false;
        config.check.core = true;
        config.check.lib = true;
    });
}

/// 本机能跑的 Java：先 `JAVA_HOME`，再顺着 `java` 自己报的 `java.home` 找
///
/// 必须给**绝对路径**：启动器的 `jvm_local` 是个路径，填裸名字（`java`）会被当成
/// "这个路径不存在"，于是回退去扫系统 Java —— 扫出来的往往是 `javaw.exe`。
/// 探针要现编译，所以还要求是 JDK 而不是纯 JRE（用 `--list-modules` 认 `jdk.compiler`）。
///
/// # 返回值
///
/// 返回（java, javac）可执行文件；没有可用 JDK 时返回 `None`
fn find_jdk() -> Option<(PathBuf, PathBuf)> {
    let exe = if cfg!(windows) { ".exe" } else { "" };

    let mut homes = Vec::new();
    if let Ok(home) = std::env::var("JAVA_HOME") {
        homes.push(PathBuf::from(home));
    }
    // `java.home` 是 JVM 自己报的安装根目录，比猜 PATH 靠谱
    if let Ok(out) = Command::new("java")
        .args(["-XshowSettings:properties", "-version"])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stderr);
        for line in text.lines() {
            let line = line.trim();
            if let Some(path) = line.strip_prefix("java.home = ") {
                homes.push(PathBuf::from(path));
            }
        }
    }

    homes.into_iter().find_map(|home| {
        let java = home.join("bin").join(format!("java{exe}"));
        let javac = home.join("bin").join(format!("javac{exe}"));
        if !java.is_file() || !javac.is_file() {
            return None;
        }
        let has_compiler = Command::new(&java)
            .arg("--list-modules")
            .output()
            .map(|out| {
                out.status.success()
                    && String::from_utf8_lossy(&out.stdout).contains("jdk.compiler")
            })
            .unwrap_or(false);
        has_compiler.then_some((java, javac))
    })
}

/// 目录下所有文件
///
/// - `dir`: 起始目录
/// - `acc`: 结果累加，调用方一般不传
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

/// 编译探针，返回主类名与它的类路径目录
///
/// 探针只做两件事：把 JVM 的类路径与收到的游戏参数打印出来、并同样写进工作目录下的
/// `probe-args.txt`。落文件这一手是为了不依赖日志链路 —— 它就是"进程真按这份参数跑起来了"的证据。
///
/// - `base`: 运行目录
/// - `javac`: javac 可执行文件
///
/// # 返回值
///
/// 返回（主类名, 类路径目录）
fn build_probe(base: &Path, javac: &Path) -> (String, PathBuf) {
    let src_dir = base.join("probe");
    std::fs::create_dir_all(&src_dir).unwrap();
    let src = src_dir.join("MmlLaunchProbe.java");
    std::fs::write(
        &src,
        format!(
            r#"import java.nio.file.Files;
import java.nio.file.Path;

public class MmlLaunchProbe {{
    public static void main(String[] args) throws Exception {{
        String line = "{MARK} cp=" + System.getProperty("java.class.path") + " args=" + String.join(" ", args);
        System.out.println(line);
        Files.writeString(Path.of("probe-args.txt"), line);
    }}
}}
"#
        ),
    )
    .unwrap();

    let classes = base.join("probe-classes");
    std::fs::create_dir_all(&classes).unwrap();
    let out = Command::new(javac)
        .arg("-d")
        .arg(&classes)
        .arg(&src)
        .output()
        .expect("javac 应能运行");
    assert!(
        out.status.success(),
        "探针应编译通过：{}",
        String::from_utf8_lossy(&out.stderr)
    );

    ("MmlLaunchProbe".to_string(), classes)
}

/// 造一个待下载 / 待启动的实例
///
/// **不指定 Java**：`jvm_local` 留空，让启动器按版本要求的 Java 主版本自己检测，
/// 与用户点"启动游戏"时走的同一条路（这里只负责编译探针时才需要 javac）。
///
/// - `uuid`: 实例 UUID
/// - `dir`: 实例目录名
/// - `probe`: 探针的主类名与类路径目录，给了就拿它当主类，`None` 表示用版本 json 里的主类
///
/// # 返回值
///
/// 返回实例配置
fn make_instance(uuid: Uuid, dir: &str, probe: Option<(&str, &Path)>) -> InstanceSettingObj {
    InstanceSettingObj {
        uuid,
        name: dir.to_string(),
        dir: dir.to_string(),
        version: VERSION.to_string(),
        advance_jvm: probe.map(|(main, class_path)| AdvanceJvmObj {
            main_class: Some(main.to_string()),
            class_path: Some(class_path.to_string_lossy().to_string()),
        }),
        jvm_arg: Some(RunArgObj {
            min_memory: Some(256),
            max_memory: Some(512),
            gc_mode: Some(GCType::None),
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// 启动参数：离线账户、不自动进服、不提权
fn make_launch_arg() -> GameLaunchArg {
    GameLaunchArg {
        auth: LoginObj::default(),
        auto: AutoJoinType::None,
        admin: false,
        mixin: None,
        lang: None,
        skip_server_pack: false,
    }
}

/// 真下载：版本 json、客户端 jar、运行库、资源索引都要真的落到磁盘上
#[ignore = "在线下载测试：会下载版本 json / 客户端 jar / 运行库，用 -- --ignored 显式跑"]
#[tokio::test]
async fn real_download_prepares_game_files() {
    ensure_init();
    let base = mml_base::get_base_dir();
    let instance = make_instance(Uuid::new_v4(), "real-download", None);

    let mut arg = make_launch_arg();
    let obj = instance
        .check_game_file(&mut arg, None, CancellationToken::new())
        .await
        .expect("检查并补齐游戏文件应成功");

    // 版本 json 解析出了主类，说明它确实被取到并读过了
    assert!(!obj.main_class.is_empty(), "版本 json 里应解析出主类");
    assert!(!obj.jvm_args.is_empty(), "版本 json 里应解析出 JVM 参数");

    let mut files = Vec::new();
    list_files(&base, &mut files);

    // 客户端 jar：体积是"真下下来了"最直接的证据
    let client = files
        .iter()
        .find(|path| path.ends_with(format!("client-{VERSION}.jar")))
        .unwrap_or_else(|| panic!("应下载出客户端 jar，实际文件 {} 个", files.len()));
    let size = std::fs::metadata(client)
        .map(|item| item.len())
        .unwrap_or(0);
    assert!(size > 1024 * 1024, "客户端 jar 体积不对：{size} 字节");

    // 运行库与资源索引
    let libs = files
        .iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "jar"))
        .count();
    assert!(libs > 3, "应下载出多个运行库 jar，实际 {libs} 个");

    assert!(
        files
            .iter()
            .any(|path| path.ends_with(format!("{VERSION}.json"))),
        "版本 json 应已落盘"
    );
    assert!(
        files
            .iter()
            .any(|path| path.components().any(|part| part.as_os_str() == "indexes")),
        "资源索引应已落盘"
    );
}

/// 真启动：检查文件、拼命令行、起进程、读输出、发退出事件，一条都不省
#[ignore = "在线启动测试：会下载版本 json / 客户端 jar / 运行库并真起进程，用 -- --ignored 显式跑"]
#[tokio::test]
async fn real_launch_downloads_and_starts() {
    ensure_init();
    let Some((java, javac)) = find_jdk() else {
        eprintln!("跳过真实启动用例：本机没有可用的 JDK");
        return;
    };

    let base = mml_base::get_base_dir();
    let uuid = Uuid::new_v4();

    // 探针：打印 JVM 的类路径与收到的游戏参数，两样都是"下载 + 拼装"的结果
    let (main, classes) = build_probe(&base, &javac);
    let mut instance = make_instance(uuid, "real-launch", Some((&main, &classes)));
    // 测试进程的 Java 列表是空的，get_java 只能靠实例自己指路；直接用 JAVA_HOME 找到的那个
    instance.jvm_local = Some(java.to_string_lossy().to_string());

    // 运行日志：只收进程的原始输出行
    let logs = Arc::new(Mutex::new(Vec::<String>::new()));
    let logs_sink = logs.clone();
    let log_id = mml_game::add_run_log(move |event: &InstanceLog| {
        if let InstanceLogType::AddLog(item) = &event.log
            && let GameLog::Text(text) = &item.log
        {
            logs_sink.lock().unwrap().push(text.clone());
        }
    });

    // 退出事件：等它就知道进程结束了
    let exited = Arc::new(Mutex::new(false));
    let exited_flag = exited.clone();
    let exit_id = mml_game::add_exit(move |event| {
        if event.uuid == uuid {
            *exited_flag.lock().unwrap() = true;
        }
    });

    let mut arg = make_launch_arg();
    instance
        .start_game(&mut arg, None, CancellationToken::new())
        .await
        .expect("启动应成功");

    // 等进程退出：首次跑要下载，给足 10 分钟
    let deadline = Instant::now() + Duration::from_secs(600);
    while !*exited.lock().unwrap() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(200));
    }

    mml_game::remove_run_log(log_id);
    mml_game::remove_exit(exit_id);

    assert!(*exited.lock().unwrap(), "进程应在 10 分钟内退出");

    // 凭据文件：进程真按这份参数跑起来了的硬证据，不依赖日志链路
    let mut files = Vec::new();
    list_files(&base, &mut files);
    let receipt = files
        .iter()
        .find(|path| path.ends_with("probe-args.txt"))
        .unwrap_or_else(|| panic!("探针应写下凭据文件，实际文件 {} 个", files.len()));
    let line = std::fs::read_to_string(receipt).expect("凭据文件应可读");

    assert!(
        line.contains(&format!("client-{VERSION}.jar")),
        "类路径里应有下载下来的客户端 jar：{line}"
    );
    assert!(
        line.contains(&format!("--version {VERSION}")),
        "版本占位符应被替换掉并传给进程：{line}"
    );
    assert!(
        line.contains(&format!("--width")),
        "窗口参数应传给进程：{line}"
    );

    // 日志链路也看一眼：GUI 的实时日志就是靠它，头部被掩码也不影响这行断言
    let logs = logs.lock().unwrap();
    assert!(
        logs.iter().any(|item| item.contains(MARK)),
        "运行日志里也应有探针输出，实际：{logs:?}"
    );
}
