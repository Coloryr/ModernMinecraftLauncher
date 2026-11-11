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

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(unix)]
use std::os::unix::process::CommandExt;

/// 不给子进程建控制台窗口
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 建一个不给子进程开控制台窗口的命令
///
/// 启动器是 GUI 进程，`java -version`、`where` 这类控制台程序默认会自建一个窗口闪一下；
/// 只跑一次、只要输出的探测类调用都走这里。
///
/// - `path`: 可执行文件路径或命令名
///
/// # 返回值
///
/// 已配好隐藏标志的命令，调用方继续加参数 / 配 stdio
pub fn hidden_command<P: AsRef<std::ffi::OsStr>>(path: P) -> Command {
    let mut cmd = Command::new(path);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

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
/// Windows 上不创建控制台窗口：启动器是 GUI 进程，子进程若是控制台程序
/// （如用户手动选的 `java.exe`），默认会自建一个黑窗。
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
    launch_inner(path, args, env, working_dir, admin, false, false)
}

/// 启动进程并断开 stdin
///
/// 用在输出已经接到日志的命令上：那个窗口本身是空的，脚本里的交互式提问也没人能回答，
/// 所以 stdin 直接接空设备，`input()` 这类调用会立刻读到 EOF 而不是一直挂着。
/// 控制台窗口与 [`launch`] 一样不创建。
///
/// 参数含义同 [`launch`]。
pub fn launch_hidden<P: AsRef<Path>>(
    path: P,
    args: Vec<String>,
    env: HashMap<String, String>,
    working_dir: P,
    admin: bool,
) -> CoreResult<LaunchResult> {
    launch_inner(path, args, env, working_dir, admin, true, false)
}

/// 启动进程并让它在 Unix 上独占一个进程组
///
/// 用在 RenderDoc 这类会再拉起子进程的工具上：停止时整组结束，避免只杀掉
/// renderdoccmd 却留下还在跑的游戏。Windows 上等价于 launch_hidden。
///
/// 参数含义同 launch。
pub fn launch_hidden_in_group<P: AsRef<Path>>(
    path: P,
    args: Vec<String>,
    env: HashMap<String, String>,
    working_dir: P,
    admin: bool,
) -> CoreResult<LaunchResult> {
    launch_inner(path, args, env, working_dir, admin, true, true)
}

/// 启动进程的实现
///
/// - `hidden`: 是否断开 stdin；Windows 上控制台窗口始终不创建
/// - `new_group`: Unix 上是否让子进程自成进程组，停止时可整组结束
fn launch_inner<P: AsRef<Path>>(
    path: P,
    args: Vec<String>,
    env: HashMap<String, String>,
    working_dir: P,
    admin: bool,
    hidden: bool,
    new_group: bool,
) -> CoreResult<LaunchResult> {
    #[cfg(not(unix))]
    let _ = new_group;

    if admin && !is_run_as_admin() {
        return launch_with_elevation(path, args, env, working_dir);
    }

    let mut cmd = Command::new(path.as_ref());
    cmd.current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        // 启动器是 GUI 进程、自己没有控制台，子进程只要是控制台程序（用户手动选的 java.exe、
        // 自定义包装器）就会自建一个黑窗。输出已经接到日志，这里一律不建窗口
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(unix)]
    if new_group {
        cmd.process_group(0);
    }

    if hidden {
        // 空设备在 Windows 上算字符设备，`isatty()` 会返回真，脚本里的 input() 照样会打印提示
        // 并等按键。用管道再把写端关掉，子进程看到的是 EOF，isatty() 才是假
        cmd.stdin(Stdio::piped());
    }

    for (key, value) in env {
        cmd.env(key, value);
    }

    for arg in args {
        cmd.arg(arg);
    }

    let mut child = cmd.spawn().map_err(process_err)?;
    if hidden {
        // 立刻关掉写端：子进程读 stdin 拿到 EOF
        drop(child.stdin.take());
    }
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

/// 结束进程及其子进程
///
/// - `child`: 要结束的子进程
///
/// # 返回值
///
/// 进程已结束（或本来就不在了）返回 `Ok(())`；等待失败返回相应错误
///
/// Windows 走 `taskkill /T /F`，连带子进程一起结束；其它平台只结束直接子进程。
pub fn kill_tree(child: &mut Child) -> io::Result<()> {
    #[cfg(windows)]
    {
        // 启动器没有控制台，taskkill 自己也得带上不建窗口的标志，否则会闪一下黑框
        let _ = std::process::Command::new("taskkill")
            .args(["/T", "/F", "/PID", &child.id().to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }

    let _ = child.kill();
    child.wait().map(|_| ())
}

/// 结束由 launch_hidden_in_group 启动的整组进程
///
/// Unix 上子进程是组长，向负 PID 发信号即可带上 RenderDoc 拉起的游戏；
/// Windows 上仍走 taskkill /T。
///
/// - child: 要结束的子进程
///
/// # 返回值
///
/// 等待进程退出失败时返回错误
pub fn kill_tree_group(child: &mut Child) -> io::Result<()> {
    #[cfg(unix)]
    {
        let pid = child.id() as i32;
        if pid > 1 {
            // SAFETY: 只向本进程组的组长发信号；组长就是这里的子进程
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
        }
        let _ = child.kill();
        child.wait().map(|_| ())
    }
    #[cfg(not(unix))]
    {
        kill_tree(child)
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

#[cfg(any(target_os = "linux", test))]
mod linux_elevation;
#[cfg(any(target_os = "macos", test))]
mod macos_elevation;
#[cfg(windows)]
mod windows_elevation;
#[cfg(windows)]
pub use windows_elevation::{
    connect_named_pipe_with_timeout, create_named_pipe, launch_with_elevation_windows,
};

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
        let status = macos_elevation::command(
            &current_exe,
            args,
            HashMap::new(),
            &std::env::current_dir().map_err(process_err)?,
            None,
        )
        .status()
        .map_err(process_err)?;
        if !status.success() {
            return Err(process_err(format!("osascript: {status}")));
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    {
        let mut cmd = linux_elevation::command(
            &current_exe,
            args,
            HashMap::new(),
            &std::env::current_dir().map_err(process_err)?,
        );
        let status = cmd.status().map_err(process_err)?;
        if !status.success() {
            return Err(process_err(format!("pkexec: {status}")));
        }
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
    let output = hidden_command(command).output().map_err(process_err)?;

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
    let output = hidden_command(command)
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
#[path = "../../tests/unit/process_helper_tests.rs"]
mod tests;
