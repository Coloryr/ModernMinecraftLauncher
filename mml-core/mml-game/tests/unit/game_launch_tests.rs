//! 实例启动的进程层冒烟：真起一个进程，看输出进不进运行日志、`tick` 认不认退出。
//!
//! "游戏"由系统命令解释器扮演（Windows 是 `cmd /c echo`，其余是 `sh -c echo`）：
//! 不下载任何游戏文件、不需要 Java、不联网，只验证启动器这侧的进程与日志链路。
//!
//! 顺序上**先等日志、再 tick**：`tick` 判定退出会置退出标志，而读流线程每行都查这个标志，
//! 先 tick 可能让最后几行没读完就收工。
use std::time::Duration;

use uuid::Uuid;

use super::*;
use crate::test_support::boot;

/// 假游戏输出里的标记，用来在运行日志里找那一行
const MARK: &str = "mml-launch-ok";

/// 假游戏：打印一行标记后退出 0
///
/// # 返回值
///
/// 返回可执行文件与参数
fn fake_game() -> (PathBuf, Vec<String>) {
    #[cfg(windows)]
    {
        let exe = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());
        (
            PathBuf::from(exe),
            vec!["/c".to_string(), format!("echo {MARK}")],
        )
    }
    #[cfg(not(windows))]
    {
        (
            PathBuf::from("sh"),
            vec!["-c".to_string(), format!("echo {MARK}")],
        )
    }
}

/// 运行日志里有没有这一行
fn log_has(uuid: &Uuid, mark: &str) -> bool {
    crate::test_support::run_log_texts(uuid)
        .iter()
        .any(|line| line.contains(mark))
}

/// 起进程 → 输出进运行日志 → tick 判定退出
#[test]
fn handle_starts_process_and_forwards_output() {
    boot();
    let uuid = Uuid::new_v4();
    let path = mml_testutil::temp_path(format!("mml-launch-test-{}", uuid.simple()));
    let (java, args) = fake_game();

    let handle = InstanceHandle::new(GameRunObj {
        uuid,
        encoding: LogEncoding::UTF8,
        auth: LoginObj::default(),
        path: path.clone(),
        java,
        args,
        env: HashMap::new(),
        admin: false,
        debugger: LaunchDebugger::None,
    })
    .expect("启动假游戏进程失败");

    // 先等输出落地：此时还没 tick，读流线程不会被退出标志打断
    let deadline = Instant::now() + Duration::from_secs(10);
    while !log_has(&uuid, MARK) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(log_has(&uuid, MARK), "运行日志里应出现假游戏的输出");

    // 再等进程退出：tick 是轮询式的，调用方得自己按节奏推
    while !handle.is_exit() && Instant::now() < deadline {
        handle.tick();
        thread::sleep(Duration::from_millis(20));
    }
    assert!(handle.is_exit(), "假游戏进程应在 10 秒内退出");

    let _ = std::fs::remove_dir_all(&path);
}

/// 命令输出按实例设置的编码解：GBK 字节要读成中文
#[test]
fn read_stream_decodes_gbk() {
    boot();
    let uuid = Uuid::new_v4();
    // "中文" 的 GBK 字节
    let data: &[u8] = &[0xD6, 0xD0, 0xCE, 0xC4, b'\n'];

    read_process_stream(
        data,
        uuid,
        LogEncoding::GBK,
        Arc::new(AtomicBool::new(false)),
        false,
    );

    assert!(log_has(&uuid, "中文"), "GBK 输出应解成中文");
}

/// renderdoccmd 的命令行回显不能进日志：里面有 access token
#[test]
fn renderdoc_launch_line_is_masked() {
    boot();
    let uuid = Uuid::new_v4();
    let data = b"Launching 'java.exe' with params: --accessToken SECRET_TOKEN\n";

    read_process_stream(
        &data[..],
        uuid,
        LogEncoding::UTF8,
        Arc::new(AtomicBool::new(false)),
        true,
    );

    assert!(
        !log_has(&uuid, "SECRET_TOKEN"),
        "access token 不能进运行日志"
    );
}

/// 每一行输出都要发一次日志事件
///
/// GUI 的实时日志就靠这条事件流。只在"新建日志"那一次发（日志已在表里就只入库、不发），
/// 实时日志就只剩第一行；而 `start_game` 开头会 `clear_game_log` 先把空日志建好，
/// 于是真实启动后一行都收不到。
#[test]
fn every_output_line_emits_log_event() {
    boot();
    let uuid = Uuid::new_v4();

    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = seen.clone();
    let id = crate::add_run_log(move |event: &crate::InstanceLog| {
        if let crate::InstanceLogType::AddLog(item) = &event.log
            && let GameLog::Text(text) = &item.log
        {
            sink.lock().unwrap().push(text.clone());
        }
    });

    crate::clear_game_log(&uuid);
    crate::add_game_log(&uuid, "第一行");
    crate::add_game_log(&uuid, "第二行");
    crate::remove_run_log(id);

    let seen = seen.lock().unwrap();
    assert!(
        seen.contains(&"第一行".to_string()),
        "第一行应发事件，实际：{seen:?}"
    );
    assert!(
        seen.contains(&"第二行".to_string()),
        "日志已存在时也要发事件，实际：{seen:?}"
    );
}
/// ZGC 分代开关只按实际 Java 版本补：21 / 22 要，23 起默认、26 起不认
#[test]
fn zgc_generational_only_on_java_21_22() {
    let with_zgc = || vec!["-XX:+UseZGC".to_string()];

    for major in [21, 22] {
        let mut args = with_zgc();
        fix_zgc_args(&mut args, Some(major));
        assert!(
            args.contains(&"-XX:+ZGenerational".to_string()),
            "Java {major} 要显式打开分代 ZGC"
        );
    }

    for major in [17, 23, 24, 25, 26] {
        let mut args = with_zgc();
        fix_zgc_args(&mut args, Some(major));
        assert!(
            !args.contains(&"-XX:+ZGenerational".to_string()),
            "Java {major} 不该带 ZGenerational"
        );
    }

    // 取不到实际版本时保守处理：不加，避免 Java 26 起不来
    let mut unknown = with_zgc();
    fix_zgc_args(&mut unknown, None);
    assert!(!unknown.contains(&"-XX:+ZGenerational".to_string()));

    // 非 ZGC 的旧参数里残留这条也要摘掉
    let mut stale = vec!["-XX:+UseG1GC".to_string(), "-XX:+ZGenerational".to_string()];
    fix_zgc_args(&mut stale, Some(26));
    assert!(!stale.contains(&"-XX:+ZGenerational".to_string()));
}

/// 检查游戏文件要留下可诊断的日志
///
/// 这一步以前只有一行用时，出问题时看不出查没查、缺不缺。这里用一份空启动内容：
/// 核心与运行库都有统计，资源索引读不到则记「跳过」，两条路径都覆盖。
#[tokio::test]
async fn check_game_file_writes_logs() {
    let run = boot();
    crate::init(&run).unwrap();

    let uuid = Uuid::new_v4();
    let instance = InstanceSettingObj {
        uuid,
        name: "check-log-test".to_string(),
        dir: "check-log-test".to_string(),
        version: "1.20.4".to_string(),
        ..Default::default()
    };

    instance
        .get_lost_game_file(&GameLaunchObj::default())
        .await
        .unwrap();

    let logs = {
        let logs = crate::RUNTIME_LOGS.read().unwrap();
        let log = logs.get(&uuid).expect("检查过就应当有运行日志");
        log.logs.read().unwrap().clone()
    };

    let mut core = false;
    let mut libs = false;
    let mut assets_skipped = false;
    for item in logs.iter() {
        match &item.log {
            GameLog::CheckCount { kind, total, lost }
                if *kind == crate::game_log::CheckKind::Core && *total == 1 && *lost == 0 =>
            {
                core = true;
            }
            GameLog::CheckCount { kind, total, lost }
                if *kind == crate::game_log::CheckKind::Libs && *total == 0 && *lost == 0 =>
            {
                libs = true;
            }
            GameLog::CheckSkipped(crate::game_log::CheckKind::Assets) => assets_skipped = true,
            _ => {}
        }
    }

    assert!(core, "游戏核心要有统计行");
    assert!(libs, "运行库要有统计行");
    assert!(assets_skipped, "资源索引缺失要记一行跳过");
}
