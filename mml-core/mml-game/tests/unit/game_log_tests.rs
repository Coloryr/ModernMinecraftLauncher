use super::*;
use std::io::Write;

/// .log.gz 要先解压再逐行解析：把压缩字节当文本读时一行都解析不出来
#[test]
fn reads_gzip_log() {
    let dir = mml_testutil::temp_dir().join(format!("mml-log-gz-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("2026-01-01-1.log.gz");
    let file = fs::File::create(&path).unwrap();
    let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    encoder
        .write_all(b"[12:00:00] [main/INFO]: hello gz\n")
        .unwrap();
    encoder.finish().unwrap();

    let log = InstanceRuntimeLog::from_file(&path, LogEncoding::UTF8);
    let logs = log.logs.read().unwrap();
    assert_eq!(logs.len(), 1);
    // 存的是整行原文，解析成哪种变体不重要；关键是不能是压缩字节
    let text = match &logs[0].log {
        GameLog::GameLog(obj) => &obj.log,
        GameLog::Text(s) => s,
        _ => panic!("gzip 日志不应解析成启动器消息"),
    };
    assert_eq!(text, "[12:00:00] [main/INFO]: hello gz");

    drop(logs);
    let _ = std::fs::remove_dir_all(&dir);
}

/// 没有 `/级别` 的行：可选组不参与匹配，按下标取会 panic
#[test]
fn parses_line_without_level() {
    let line = "[12:00:00] [main]: hello";
    let parsed = InstanceRuntimeLog::parse_game_log_line(line);
    assert_eq!(parsed.thread, "main");
    assert_eq!(parsed.time, "12:00:00");
    assert_eq!(parsed.log, line);
    assert!(matches!(parsed.level, LogLevel::None));
}

