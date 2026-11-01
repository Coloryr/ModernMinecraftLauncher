use super::*;
use std::path::PathBuf;

/// 拿到一个唯一的临时工作目录
fn temp_work_dir() -> PathBuf {
    let dir = mml_testutil::temp_dir().join(format!(
        "mml_sys_process_test_{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// is_run_as_admin 只要求不 panic 并返回布尔值，CI 环境权限不定，不做断言
#[test]
fn test_is_run_as_admin() {
    let _ = is_run_as_admin();
}

/// run_command_arg 返回按行拆分的 stdout
#[cfg(target_os = "windows")]
#[test]
fn test_run_command_arg() {
    let lines = run_command_arg("cmd", &["/C", "echo", "hello"]).unwrap();
    assert_eq!(lines, vec!["hello".to_string()]);
}

/// run_command_arg 返回按行拆分的 stdout
#[cfg(not(target_os = "windows"))]
#[test]
fn test_run_command_arg() {
    let lines = run_command_arg("echo", &["hello"]).unwrap();
    assert_eq!(lines, vec!["hello".to_string()]);
}

/// 命令执行失败时返回空列表，非零退出码即视为失败
#[cfg(target_os = "windows")]
#[test]
fn test_run_command_arg_failure_returns_empty() {
    let lines = run_command_arg("cmd", &["/C", "exit", "1"]).unwrap();
    assert!(lines.is_empty());
}

/// 命令执行失败时返回空列表，非零退出码即视为失败
#[cfg(not(target_os = "windows"))]
#[test]
fn test_run_command_arg_failure_returns_empty() {
    // false 命令固定返回非零退出码
    let lines = run_command("false").unwrap();
    assert!(lines.is_empty());
}

/// run_command：命令不存在时报错
#[test]
fn test_run_command_not_found() {
    let result = run_command("mml_no_such_command_xyz");
    assert!(result.is_err());
}

/// launch 启动普通进程并捕获 stdout
#[cfg(target_os = "windows")]
#[test]
fn test_launch_and_capture_stdout() {
    use std::io::Read;

    let work = temp_work_dir();
    let mut result = launch(
        PathBuf::from("cmd"),
        vec!["/C".to_string(), "echo".to_string(), "hi".to_string()],
        Default::default(),
        work.clone(),
        false,
    )
    .unwrap();

    assert!(!result.is_admin);
    assert!(result.pid.is_none());
    assert!(result.stdout.is_some());

    let mut stdout = result.stdout.unwrap();
    let mut buf = String::new();
    stdout.read_to_string(&mut buf).unwrap();
    assert!(buf.contains("hi"), "stdout 应包含 hi: {buf:?}");

    result.child.wait().unwrap();
    let _ = std::fs::remove_dir_all(&work);
}

/// launch 启动普通进程并捕获 stdout
#[cfg(not(target_os = "windows"))]
#[test]
fn test_launch_and_capture_stdout() {
    use std::io::Read;

    let work = temp_work_dir();
    let mut result = launch(
        PathBuf::from("echo"),
        vec!["hi".to_string()],
        Default::default(),
        work.clone(),
        false,
    )
    .unwrap();

    assert!(!result.is_admin);
    assert!(result.stdout.is_some());

    let mut stdout = result.stdout.unwrap();
    let mut buf = String::new();
    stdout.read_to_string(&mut buf).unwrap();
    assert!(buf.contains("hi"), "stdout 应包含 hi: {buf:?}");

    result.child.wait().unwrap();
    let _ = std::fs::remove_dir_all(&work);
}

/// launch 设置的环境变量对子进程可见
#[cfg(target_os = "windows")]
#[test]
fn test_launch_with_env() {
    use std::io::Read;

    let work = temp_work_dir();
    let mut env = std::collections::HashMap::new();
    env.insert("MML_TEST_VAR".to_string(), "mml_env_value".to_string());

    let mut result = launch(
        PathBuf::from("cmd"),
        vec![
            "/C".to_string(),
            "echo".to_string(),
            "%MML_TEST_VAR%".to_string(),
        ],
        env,
        work.clone(),
        false,
    )
    .unwrap();

    let mut stdout = result.stdout.unwrap();
    let mut buf = String::new();
    stdout.read_to_string(&mut buf).unwrap();
    assert!(buf.contains("mml_env_value"), "环境变量未生效: {buf:?}");

    result.child.wait().unwrap();
    let _ = std::fs::remove_dir_all(&work);
}

/// launch 设置的环境变量对子进程可见
#[cfg(not(target_os = "windows"))]
#[test]
fn test_launch_with_env() {
    use std::io::Read;

    let work = temp_work_dir();
    let mut env = std::collections::HashMap::new();
    env.insert("MML_TEST_VAR".to_string(), "mml_env_value".to_string());

    // 通过 sh 读取环境变量
    let mut result = launch(
        PathBuf::from("sh"),
        vec!["-c".to_string(), "echo $MML_TEST_VAR".to_string()],
        env,
        work.clone(),
        false,
    )
    .unwrap();

    let mut stdout = result.stdout.unwrap();
    let mut buf = String::new();
    stdout.read_to_string(&mut buf).unwrap();
    assert!(buf.contains("mml_env_value"), "环境变量未生效: {buf:?}");

    result.child.wait().unwrap();
    let _ = std::fs::remove_dir_all(&work);
}

/// launch 启动不存在的可执行文件时报 ProcessError
#[test]
fn test_launch_not_found() {
    let work = temp_work_dir();
    let result = launch(
        PathBuf::from("mml_no_such_binary_xyz"),
        Vec::new(),
        Default::default(),
        work.clone(),
        false,
    );
    assert!(result.is_err());
    let _ = std::fs::remove_dir_all(&work);
}

