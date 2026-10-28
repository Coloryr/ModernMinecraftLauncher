//! 进程管理模块
//!
//! 提供跨平台的子进程启动功能，支持：
//!
//! - **普通启动** — 标准子进程，stdout/stderr 通过管道捕获
//! - **管理员权限启动** — Windows (PowerShell + 命名管道) / macOS (osascript) / Linux (pkexec)
//! - **进程重启** — 以管理员权限重新启动启动器自身
//!
//! # 提权机制
//!
//! | 平台 | 机制 |
//! |------|------|
//! | Windows | PowerShell RunAs + 命名管道转发 stdout/stderr |
//! | macOS | `osascript do shell script with administrator privileges` |
//! | Linux | `pkexec` 直接转发 stdin/stdout/stderr |

use std::{
    collections::HashMap,
    io::{self, Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
};

use mml_names::i18_items::error_type::{CoreResult, ErrorData, ErrorType};

/// 出错原因转 ProcessError
fn process_err(err: impl std::fmt::Display) -> ErrorType {
    ErrorType::ProcessError(ErrorData::from_error(err))
}

/// 进程输出流统一封装
///
/// 统一普通子进程的输出管道和提权进程的命名管道，对外提供统一的 `Read` 接口。
pub enum OutputStream {
    /// 普通子进程的 stdout
    Stdout(std::process::ChildStdout),
    /// 普通子进程的 stderr
    Stderr(std::process::ChildStderr),
    /// 提权进程通过命名管道重定向的输出：Windows 用命名管道，macOS 用 FIFO
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    File(std::fs::File),
    /// macOS 的 FIFO 读端：写端还没连上时要先等它
    #[cfg(target_os = "macos")]
    Pipe(macos_elevation::PipeReader),
}

impl Read for OutputStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            OutputStream::Stdout(s) => s.read(buf),
            OutputStream::Stderr(s) => s.read(buf),
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            OutputStream::File(f) => f.read(buf),
            #[cfg(target_os = "macos")]
            OutputStream::Pipe(p) => p.read(buf),
        }
    }
}

/// 进程启动结果
///
/// 统一普通启动和提权启动的返回结构
pub struct LaunchResult {
    /// 进程句柄，普通启动为目标进程，提权启动为启动器进程
    pub child: Child,
    /// 是否为提权启动：通过外部启动器启动，非直接子进程
    pub is_admin: bool,
    /// Windows 提权辅助进程的 PID，其余情况为 `None`
    pub pid: Option<u32>,
    /// 通知已提权的辅助进程停止游戏，避免普通权限进程跨权限结束目标。
    pub control: Option<ElevationControl>,
    /// 目标进程 stdout，普通启动为管道，提权为命名管道或 pkexec 转发
    pub stdout: Option<OutputStream>,
    /// 目标进程 stderr，合并策略同 stdout
    pub stderr: Option<OutputStream>,
}

/// 提权辅助进程的专用停止通道。
pub struct ElevationControl {
    /// 创建启动任务时独占的控制管道。
    pipe: std::fs::File,
}

impl ElevationControl {
    /// 请求辅助进程结束目标进程树。
    /// # 返回值
    /// 请求写入成功或管道已经断开的错误。
    pub fn request_stop(&self) -> io::Result<()> {
        (&self.pipe).write_all(&[1])
    }
}

/// 检查当前进程是否以管理员/root 权限运行
///
/// # 返回值
///
/// 当前进程具有管理员 / root 权限时返回 true
pub fn is_run_as_admin() -> bool {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::Shell::IsUserAnAdmin;
        unsafe { IsUserAnAdmin().as_bool() }
    }
    #[cfg(not(target_os = "windows"))]
    {
        // 通过 `id -u` 检查是否为 root，跨 macOS / Linux
        std::process::Command::new("id")
            .arg("-u")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim() == "0")
            .unwrap_or(false)
    }
}

/// 启动进程
///
/// - `path`: 可执行文件路径
/// - `args`: 程序参数
/// - `env`: 环境变量
/// - `working_dir`: 工作目录
/// - `admin`: 是否请求管理员权限
///
/// # 返回值
///
/// 返回启动结果，进程创建失败时返回相应错误
pub fn launch<P: AsRef<Path>>(
    path: P,
    args: Vec<String>,
    env: HashMap<String, String>,
    working_dir: P,
    admin: bool,
) -> CoreResult<LaunchResult> {
    if admin && !is_run_as_admin() {
        launch_with_elevation(path, args, env, working_dir)
    } else {
        let mut cmd = Command::new(path.as_ref());
        cmd.current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in env {
            cmd.env(key, value);
        }

        for arg in args {
            cmd.arg(arg);
        }

        let mut child = cmd.spawn().map_err(process_err)?;
        let stdout = child.stdout.take().map(OutputStream::Stdout);
        let stderr = child.stderr.take().map(OutputStream::Stderr);

        Ok(LaunchResult {
            child,
            is_admin: false,
            pid: None,
            control: None,
            stdout,
            stderr,
        })
    }
}

/// 以管理员权限启动进程，平台特定实现
///
/// Windows: 使用命名管道捕获提权进程的 stdout/stderr，PowerShell Wait-Process 保持存活
/// macOS: 使用 osascript 提权，启动器在目标进程运行期间保持存活
/// Linux: 使用 pkexec 提权，启动器在目标进程运行期间保持存活
///
/// 参数含义同 [`launch`]。
///
/// # 返回值
///
/// 返回启动结果，提权启动失败时返回相应错误
fn launch_with_elevation<P: AsRef<Path>>(
    path: P,
    args: Vec<String>,
    env: HashMap<String, String>,
    working_dir: P,
) -> CoreResult<LaunchResult> {
    #[cfg(target_os = "windows")]
    {
        launch_with_elevation_windows(path, args, env, working_dir)
    }

    #[cfg(target_os = "macos")]
    {
        // 有 FIFO 就把输出重定向进去，日志才是实时的；
        // 建不出来就退回"输出合并进脚本返回值"，至少退出后还能看到
        let pipe = macos_elevation::create_log_pipe();
        let mut cmd = macos_elevation::command(
            path.as_ref(),
            &args,
            env,
            working_dir.as_ref(),
            pipe.as_ref().map(|item| item.path.as_path()),
        );

        cmd.stdout(Stdio::piped()).stderr(Stdio::null());
        let mut child = cmd.spawn().map_err(process_err)?;
        let stdout = match pipe {
            Some(item) => Some(OutputStream::Pipe(item.reader)),
            None => child.stdout.take().map(OutputStream::Stdout),
        };
        Ok(LaunchResult {
            child,
            is_admin: true,
            pid: None,
            control: None,
            stdout,
            stderr: None, // stderr 已经并进 stdout
        })
    }

    #[cfg(target_os = "linux")]
    {
        let mut cmd = linux_elevation::command(path.as_ref(), &args, env, working_dir.as_ref());

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(process_err)?;
        let stdout = child.stdout.take().map(OutputStream::Stdout);
        let stderr = child.stderr.take().map(OutputStream::Stderr);
        Ok(LaunchResult {
            child,
            is_admin: true,
            pid: None,
            control: None,
            stdout,
            stderr,
        })
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err(ErrorType::ProcessElevationUnsupported)
    }
}

#[cfg(windows)]
mod windows_elevation;
#[cfg(any(target_os = "linux", test))]
mod linux_elevation;
#[cfg(any(target_os = "macos", test))]
mod macos_elevation;
#[cfg(windows)]
pub use windows_elevation::{create_named_pipe, connect_named_pipe_with_timeout, launch_with_elevation_windows};

/// 以管理员权限重新启动启动器自身
///
/// 对应 C# 的 `ProcessUtils.LaunchAdmin(string[])`。
///
/// - `args`: 启动参数
///
/// # 返回值
///
/// 启动器进程创建或等待失败时返回相应错误，成功返回 `Ok(())`
pub fn launch_admin(args: &[String]) -> CoreResult<()> {
    let current_exe = std::env::current_exe().map_err(process_err)?;

    #[cfg(target_os = "windows")]
    {
        windows_elevation::restart_elevated(&current_exe, args)
    }

    #[cfg(target_os = "macos")]
    {
        // 重启自身不需要日志管道：输出留给调用方的控制台
        let status = macos_elevation::command(&current_exe, args, HashMap::new(),
            &std::env::current_dir().map_err(process_err)?, None)
            .status().map_err(process_err)?;
        if !status.success() { return Err(process_err(format!("osascript: {status}"))); }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    {
        let mut cmd = linux_elevation::command(&current_exe, args, HashMap::new(),
            &std::env::current_dir().map_err(process_err)?);
        let status = cmd.status().map_err(process_err)?;
        if !status.success() { return Err(process_err(format!("pkexec: {status}"))); }
        Ok(())
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Admin elevation not supported on this platform",
        ))
    }
}

/// 执行命令并返回输出行列表
///
/// - `command`: 命令名
///
/// # 返回值
///
/// 返回按行拆分、去空的 stdout；命令启动失败报错，非零退出码返回空列表
pub fn run_command(command: &str) -> CoreResult<Vec<String>> {
    let output = Command::new(command).output().map_err(process_err)?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

/// 执行命令并返回输出行列表
///
/// - `command`: 命令名
/// - `args`: 命令参数
///
/// # 返回值
///
/// 返回按行拆分、去空的 stdout；命令启动失败报错，非零退出码返回空列表
pub fn run_command_arg<I, S>(command: &str, args: I) -> CoreResult<Vec<String>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = Command::new(command)
        .args(args)
        .output()
        .map_err(process_err)?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

#[cfg(test)]
mod tests {
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
}
