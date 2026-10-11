//! mml-log 集成测试
//!
//! 日志系统使用进程级全局状态，`STREAM` 只能初始化一次、日志线程只能启动
//! 一次，因此在同一个测试里顺序执行完整的启动 → 记录 → 校验 → 停止流程。
//!
//! 日志文件统一写入仓库 `target/temp` 下的唯一子目录，测试结束后尽力清理；
//! 日志文件句柄由全局 `STREAM` 持有，Windows 下可能删不掉，失败会忽略

use std::{
    fs,
    panic::{self, AssertUnwindSafe},
    path::PathBuf,
    thread,
    time::Duration,
};

use mml_names::{i18_items::info_type::InfoType, names};

/// 在临时目录下创建唯一的测试目录
fn temp_dir(name: &str) -> PathBuf {
    let dir = mml_testutil::temp_dir().join(format!("mml_log_test_{}_{name}", std::process::id()));
    // 清掉上次运行可能残留的目录
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 轮询等待文件内容满足条件，后台线程异步写盘，需要等待
fn wait_for_file<F: Fn(&str) -> bool>(file: &PathBuf, check: F) -> String {
    for _ in 0..100 {
        if let Ok(data) = fs::read_to_string(file) {
            if check(&data) {
                return data;
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    fs::read_to_string(file).unwrap_or_default()
}

/// 完整生命周期：启动 → 各级别记录 → 落盘校验 → 重复启动失败 → 停止
#[test]
fn log_lifecycle() {
    let dir = temp_dir("lifecycle");
    let log_file = dir.join(names::LOG_FILE);

    // 启动日志系统
    mml_log::start(&dir).unwrap();
    assert!(log_file.exists(), "启动后应创建日志文件");

    // 各级别日志
    mml_log::info(String::from("信息日志"));
    mml_log::warn(String::from("警告日志"));
    mml_log::error(String::from("错误日志"));
    mml_log::failt(String::from("严重日志"));
    mml_log::info_type(InfoType::TempFile);

    // 等待后台线程把队列中的日志全部写入文件
    let data = wait_for_file(&log_file, |d| {
        d.contains("信息日志")
            && d.contains("警告日志")
            && d.contains("错误日志")
            && d.contains("严重日志")
            && d.contains("临时文件")
    });
    assert!(data.contains("信息日志"), "缺少 info 日志: {data:?}");
    assert!(data.contains("警告日志"), "缺少 warn 日志: {data:?}");
    assert!(data.contains("错误日志"), "缺少 error 日志: {data:?}");
    assert!(data.contains("严重日志"), "缺少 fault 日志: {data:?}");
    assert!(data.contains("临时文件"), "缺少 info_type 日志: {data:?}");

    // 级别标记
    assert!(data.contains("[Info]"), "缺少 [Info] 标记: {data:?}");
    assert!(data.contains("[Warn]"), "缺少 [Warn] 标记: {data:?}");
    assert!(data.contains("[Error]"), "缺少 [Error] 标记: {data:?}");
    assert!(data.contains("[Fault]"), "缺少 [Fault] 标记: {data:?}");

    // 每条日志独占一行，换行符与平台一致
    let line_ending = mml_names::get_line_ending();
    assert!(
        data.contains(&format!("信息日志{line_ending}")),
        "日志应以平台换行符结尾: {data:?}"
    );

    // 重复 start 应失败：STREAM 是 OnceLock，第二次 set 会 panic
    let second = panic::catch_unwind(AssertUnwindSafe(|| mml_log::start(&dir)));
    assert!(second.is_err(), "第二次 start 应该失败");

    // 停止日志系统
    mml_log::stop();

    // 清理：日志文件句柄被全局 STREAM 持有，Windows 下可能删除失败，忽略
    let _ = fs::remove_dir_all(&dir);
}
