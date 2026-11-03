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

    read_process_stream(data, uuid, LogEncoding::GBK, Arc::new(AtomicBool::new(false)));

    assert!(log_has(&uuid, "中文"), "GBK 输出应解成中文");
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
