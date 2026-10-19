//! 进程管理模块
//!
//! 提供跨平台的子进程启动功能，支持：
//!
//! - **普通启动** — 标准子进程，stdout/stderr 通过管道捕获
//! - **管理员权限启动** — Windows (PowerShell + 命名管道) / macOS (osascript) / Linux (pkexec)
//! - **进程重启** — 以管理员权限重新启动 ColorMC 自身
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
    io::{self, Read},
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
    /// 提权进程通过命名管道重定向的输出，仅 Windows
    #[cfg(target_os = "windows")]
    File(std::fs::File),
}

impl Read for OutputStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            OutputStream::Stdout(s) => s.read(buf),
            OutputStream::Stderr(s) => s.read(buf),
            #[cfg(target_os = "windows")]
            OutputStream::File(f) => f.read(buf),
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
    /// 提权 helper 的 PID，Windows 提权时用于 taskkill /T 结束整棵进程树；其余情况为 `None`
    pub pid: Option<u32>,
    /// 目标进程 stdout，普通启动为管道，提权为命名管道或 pkexec 转发
    pub stdout: Option<OutputStream>,
    /// 目标进程 stderr，合并策略同 stdout
    pub stderr: Option<OutputStream>,
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
        // osascript 的 do shell script 只返回 stdout；用 2>&1 将 stderr 合并到 stdout
        let script = format!(
            "do shell script \"'{}' {} 2>&1\" with administrator privileges",
            path.as_ref().display(),
            args.iter()
                .map(|a| format!("\\\"{}\\\"", a))
                .collect::<Vec<_>>()
                .join(" ")
        );

        let mut cmd = Command::new("osascript");
        cmd.arg("-e").arg(script);
        cmd.current_dir(working_dir);

        for (key, value) in env {
            cmd.env(key, value);
        }

        cmd.stdout(Stdio::piped()).stderr(Stdio::null());
        let mut child = cmd.spawn().map_err(process_err)?;
        let stdout = child.stdout.take().map(OutputStream::Stdout);
        Ok(LaunchResult {
            child,
            is_admin: true,
            pid: None,
            stdout,
            stderr: None, // stderr 已由 2>&1 合并到 stdout
        })
    }

    #[cfg(target_os = "linux")]
    {
        // pkexec 作为目标进程的父进程，直接转发 stdin/stdout/stderr
        let mut cmd = Command::new("pkexec");
        cmd.arg(path.as_ref());
        for arg in &args {
            cmd.arg(arg);
        }
        cmd.current_dir(working_dir);

        for (key, value) in env {
            cmd.env(key, value);
        }

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(process_err)?;
        let stdout = child.stdout.take().map(OutputStream::Stdout);
        let stderr = child.stderr.take().map(OutputStream::Stderr);
        Ok(LaunchResult {
            child,
            is_admin: true,
            pid: None,
            stdout,
            stderr,
        })
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err(ErrorType::ProcessElevationUnsupported)
    }
}

// ============================================================================
// Windows 提权 + 命名管道实现
// ============================================================================

/// Windows 提权实现：PowerShell RunAs 起提权 helper，helper 经命名管道回传目标进程输出
///
/// - `path`: 可执行文件路径
/// - `args`: 程序参数
/// - `env`: 环境变量
/// - `working_dir`: 工作目录
///
/// # 返回值
///
/// 返回启动结果，`child` 为初始 PowerShell，`pid` 为 helper PID
#[cfg(target_os = "windows")]
pub fn launch_with_elevation_windows<P: AsRef<Path>>(
    path: P,
    args: Vec<String>,
    env: HashMap<String, String>,
    working_dir: P,
) -> CoreResult<LaunchResult> {
    use std::io::{BufRead, BufReader};
    use std::os::windows::io::AsRawHandle;
    use std::os::windows::io::{FromRawHandle, OwnedHandle};
    use std::time::Duration;

    use windows::Win32::Foundation::HANDLE;


    let id = uuid::Uuid::new_v4().to_string().replace('-', "");
    let stdout_pipe_name = format!("mml_stdout_{}", id);
    let stderr_pipe_name = format!("mml_stderr_{}", id);

    // 创建命名管道服务器端
    let stdout_handle = create_named_pipe(&stdout_pipe_name)?;
    let stderr_handle = create_named_pipe(&stderr_pipe_name)?;
    let stdout_owned = unsafe { OwnedHandle::from_raw_handle(stdout_handle.0) };
    let stderr_owned = unsafe { OwnedHandle::from_raw_handle(stderr_handle.0) };

    // 目标进程参数，在 PowerShell 中作为字符串传给 $psi.Arguments
    let arg_str = args
        .iter()
        .map(|a| {
            // 空格参数用双引号包裹，内部双引号转义
            if a.contains(' ') || a.contains('"') {
                format!("\"{}\"", a.replace('"', "\"\"\""))
            } else {
                a.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    // 环境变量
    let mut env_block = String::new();
    for (k, v) in &env {
        env_block.push_str(&format!("$psi.Environment[\"{}\"]=\"{}\";", k, v));
    }

    // 提权 helper 脚本：在新进程中运行，负责连接管道 → 启动目标 → 转发输出
    let helper_script = format!(
        "$op=New-Object System.IO.Pipes.NamedPipeClientStream('.','{stdout}',[System.IO.Pipes.PipeDirection]::Out);\
         $ep=New-Object System.IO.Pipes.NamedPipeClientStream('.','{stderr}',[System.IO.Pipes.PipeDirection]::Out);\
         $op.Connect(10000);$ep.Connect(10000);\
         $si=New-Object System.Diagnostics.ProcessStartInfo;\
         $si.FileName='{path}';\
         $si.Arguments='{args}';\
         $si.WorkingDirectory='{dir}';\
         $si.UseShellExecute=$false;$si.RedirectStandardOutput=$true;$si.RedirectStandardError=$true;\
         {env}\
         $p=[System.Diagnostics.Process]::Start($si);\
         $buf=[byte[]]::new(4096);\
         while($true){{$n=$p.StandardOutput.BaseStream.Read($buf,0,4096);if($n -gt 0){{$op.Write($buf,0,$n)}};if($n -eq 0 -and $p.HasExited){{break}}}};\
         while($true){{$n=$p.StandardError.BaseStream.Read($buf,0,4096);if($n -gt 0){{$ep.Write($buf,0,$n)}};if($n -eq 0 -and $p.HasExited){{break}}}};\
         $op.Close();$ep.Close()",
        stdout = stdout_pipe_name,
        stderr = stderr_pipe_name,
        path = path.as_ref().display(),
        args = arg_str,
        dir = working_dir.as_ref().display(),
        env = env_block,
    );

    // 初始 PowerShell：启动提权 helper → 输出 helper PID → 等待 helper 退出
    let ps_cmd = format!(
        "$h=Start-Process -FilePath powershell -Verb RunAs -PassThru \
         -ArgumentList '-NoProfile','-Command','{helper}'; \
         Write-Output $h.Id; \
         $h.WaitForExit()",
        helper = helper_script.replace('\'', "''")
    );

    let mut cmd = Command::new("powershell");
    cmd.arg("-Command").arg(&ps_cmd);
    cmd.stdout(Stdio::piped()).stderr(Stdio::null());

    let mut launcher = cmd.spawn().map_err(process_err)?;

    // 从初始 PowerShell stdout 读取 helper 的 PID
    let pid = launcher
        .stdout
        .take()
        .and_then(|stdout| {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            reader.read_line(&mut line).ok().map(|_| line)
        })
        .and_then(|line| line.trim().parse::<u32>().ok());

    // 等待 helper PowerShell 连接管道
    let (tx_stdout, rx_stdout) = std::sync::mpsc::channel();
    let (tx_stderr, rx_stderr) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        tx_stdout
            .send(connect_named_pipe_with_timeout(
                HANDLE(stdout_owned.as_raw_handle()),
                10_000,
            ))
            .ok();
    });
    std::thread::spawn(move || {
        tx_stderr
            .send(connect_named_pipe_with_timeout(
                HANDLE(stderr_owned.as_raw_handle()),
                10_000,
            ))
            .ok();
    });

    let mut stdout: Option<OutputStream> = None;
    let mut stderr: Option<OutputStream> = None;
    let deadline = std::time::Instant::now() + Duration::from_secs(15);

    while std::time::Instant::now() < deadline {
        if launcher.try_wait().ok().flatten().is_some() {
            break;
        }
        if stdout.is_none() {
            if let Ok(Ok(file)) = rx_stdout.try_recv() {
                stdout = Some(OutputStream::File(file));
            }
        }
        if stderr.is_none() {
            if let Ok(Ok(file)) = rx_stderr.try_recv() {
                stderr = Some(OutputStream::File(file));
            }
        }
        if stdout.is_some() && stderr.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    Ok(LaunchResult {
        child: launcher,
        is_admin: true,
        pid,
        stdout,
        stderr,
    })
}

// 命名管道相关常量，避免 windows-sys 模块名称差异
#[cfg(target_os = "windows")]
const FILE_FLAG_OVERLAPPED: u32 = 0x40000000;
#[cfg(target_os = "windows")]
const PIPE_ACCESS_INBOUND: u32 = 0x00000001;
#[cfg(target_os = "windows")]
const PIPE_TYPE_BYTE: u32 = 0x00000000;
#[cfg(target_os = "windows")]
const PIPE_READMODE_BYTE: u32 = 0x00000000;
#[cfg(target_os = "windows")]
const PIPE_WAIT: u32 = 0x00000000;

/// 创建一个命名管道服务器并返回原始 HANDLE
///
/// - `name`: 管道名，不含 `\\.\pipe\` 前缀
///
/// # 返回值
///
/// 返回管道句柄，创建失败时返回相应错误
#[cfg(target_os = "windows")]
pub fn create_named_pipe(name: &str) -> Result<windows::Win32::Foundation::HANDLE, ErrorType> {
    use windows::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
    use windows::Win32::System::Pipes::CreateNamedPipeW;
    use windows::Win32::System::Pipes::NAMED_PIPE_MODE;
    use windows::core::{HSTRING, PCWSTR};

    let full_name = format!(r"\\.\pipe\{}", name);

    let hstring = HSTRING::from(full_name);
    let pcwstr = PCWSTR(hstring.as_ptr());

    let handle = unsafe {
        CreateNamedPipeW(
            pcwstr,
            FILE_FLAGS_AND_ATTRIBUTES(PIPE_ACCESS_INBOUND | FILE_FLAG_OVERLAPPED),
            NAMED_PIPE_MODE(PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT),
            1,    // 最大实例数
            8192, // 输出缓冲区大小
            8192, // 输入缓冲区大小
            0,    // 默认超时
            None, // 默认安全描述符
        )
    };

    // INVALID_HANDLE_VALUE 比较，HANDLE 是 *mut c_void，需要转为 usize
    if handle == INVALID_HANDLE_VALUE {
        return Err(ErrorType::NamedPipeCreateFailed(ErrorData::from_error(
            std::io::Error::last_os_error(),
        )));
    }

    Ok(handle)
}

/// 连接到命名管道客户端，带超时；成功返回 `File` 可供读取
///
/// 调用方持 `OwnedHandle` 以保证 Send，此处接收其原始 HANDLE，
/// 连接成功后转移所有权给 `File`
///
/// - `handle`: 管道服务器句柄
/// - `timeout_ms`: 等待连接的超时，单位毫秒
///
/// # 返回值
///
/// 返回可读取的管道文件，超时或失败时返回相应错误
#[cfg(target_os = "windows")]
pub fn connect_named_pipe_with_timeout(
    handle: windows::Win32::Foundation::HANDLE,
    timeout_ms: u32,
) -> CoreResult<std::fs::File> {
    use std::os::windows::io::FromRawHandle;

    use windows::Win32::Foundation::{
        CloseHandle, ERROR_IO_PENDING, ERROR_PIPE_CONNECTED, WAIT_OBJECT_0, WAIT_TIMEOUT,
    };
    use windows::Win32::System::IO::{CancelIo, OVERLAPPED};
    use windows::Win32::System::Pipes::ConnectNamedPipe;
    use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
    use windows::core::HRESULT;

    // 创建事件用于 Overlapped I/O
    let event = unsafe { CreateEventW(None, true, false, None) };
    if let Err(err) = event {
        return Err(ErrorType::NamedPipeEventCreateFailed(ErrorData {
            error: err.message(),
        }));
    }
    let event = event.unwrap();

    let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
    overlapped.hEvent = event;

    // 异步等待客户端连接
    let result = unsafe { ConnectNamedPipe(handle, Some(&mut overlapped)) };

    if let Err(err) = result {
        if err.code() == HRESULT(ERROR_IO_PENDING.0 as i32) {
            // 等待连接完成或超时
            let wait_result = unsafe { WaitForSingleObject(event, timeout_ms) };
            if wait_result != WAIT_OBJECT_0 {
                unsafe {
                    CancelIo(handle).map_err(|err| {
                        ErrorType::ProcessError(ErrorData {
                            error: err.message(),
                        })
                    })?;
                    CloseHandle(event).map_err(|err| {
                        ErrorType::ProcessError(ErrorData {
                            error: err.message(),
                        })
                    })?;
                }
                // handle 被 drop，CloseHandle 会被调用
                return Err(if wait_result == WAIT_TIMEOUT {
                    ErrorType::NamedPipeConnectTimeout
                } else {
                    ErrorType::NamedPipeWaitFailed(ErrorData::from_error(
                        std::io::Error::last_os_error(),
                    ))
                });
            }
        } else if err.code() == HRESULT(ERROR_PIPE_CONNECTED.0 as i32) {
            // 客户端在 ConnectNamedPipe 之前就已连接
        } else {
            unsafe {
                CloseHandle(event).map_err(|err| {
                    ErrorType::ProcessError(ErrorData {
                        error: err.message(),
                    })
                })?;
            };
            return Err(ErrorType::NamedPipeConnectFailed(ErrorData::from_error(err)));
        }
    }
    // result != 0 → 立即连接成功

    // 清理事件句柄，将管道 HANDLE 所有权转移给 File
    unsafe {
        CloseHandle(event).map_err(|err| {
            ErrorType::ProcessError(ErrorData {
                error: err.message(),
            })
        })?;
    };
    let file = unsafe { std::fs::File::from_raw_handle(handle.0) };
    Ok(file)
}

/// 以管理员权限重新启动 ColorMC 自身
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
        // Windows: 使用 PowerShell Start-Process -Verb RunAs 提权重启自身
        let mut ps_cmd = format!(
            "Start-Process -FilePath '{}' -Verb RunAs",
            current_exe.display()
        );
        for arg in args {
            ps_cmd.push_str(&format!(" -ArgumentList '{}'", arg));
        }

        let mut child = Command::new("powershell")
            .arg("-Command")
            .arg(ps_cmd)
            .spawn()
            .map_err(process_err)?;
        child.wait().map_err(process_err)?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "do shell script \"'{}' {}\" with administrator privileges",
            current_exe.display(),
            args.iter()
                .map(|a| format!("\\\"{}\\\"", a))
                .collect::<Vec<_>>()
                .join(" ")
        );

        let mut child = Command::new("osascript").arg("-e").arg(script).spawn()?;
        child.wait()?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    {
        let mut cmd = Command::new("pkexec");
        cmd.arg(&current_exe);
        for arg in args {
            cmd.arg(arg);
        }
        let mut child = cmd.spawn()?;
        child.wait()?;
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
    let output = Command::new(command).args(args).output().map_err(process_err)?;

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
