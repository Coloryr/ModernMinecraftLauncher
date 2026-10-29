//! 子进程：只接受参数数组，隐藏控制台窗口，取消时回收整棵进程树
//!
//! 预览、catalog、git / npm 都从这里走；进程 pid 登记在全局表里，
//! 应用退出时一次性回收，避免关掉工具后还留着 node 进程。

use crate::messages::{MessageKey, StudioError};
use std::{
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
    sync::{Arc, LazyLock, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    sync::mpsc,
};
use tokio_util::sync::CancellationToken;

/// 输出面板只保留尾部，单位字符
const OUTPUT_LIMIT: usize = 200_000;

/// 往输出面板写一行（或一段）原始输出
pub type LogFn = Arc<dyn Fn(&str) + Send + Sync>;

/// 不记录输出的日志回调
pub fn no_log() -> LogFn {
    Arc::new(|_| {})
}

/// 本工具启动过的进程 pid
#[derive(Default)]
pub struct ProcessRegistry {
    pids: Mutex<std::collections::HashSet<u32>>,
}

static REGISTRY: LazyLock<ProcessRegistry> = LazyLock::new(ProcessRegistry::default);

/// 全局进程表
pub fn registry() -> &'static ProcessRegistry {
    &REGISTRY
}

impl ProcessRegistry {
    /// 登记一个刚启动的子进程
    pub fn add(&self, pid: u32) {
        if let Ok(mut pids) = self.pids.lock() {
            pids.insert(pid);
        }
    }

    /// 子进程已回收
    pub fn remove(&self, pid: u32) {
        if let Ok(mut pids) = self.pids.lock() {
            pids.remove(&pid);
        }
    }

    /// 同步回收全部子进程，应用退出时调用
    pub fn kill_all(&self) {
        let pids: Vec<u32> = self
            .pids
            .lock()
            .map(|pids| pids.iter().copied().collect())
            .unwrap_or_default();
        for pid in pids {
            kill_tree_sync(pid);
        }
    }
}

/// 运行主题项目所需的外部环境
#[derive(Clone, Debug)]
pub struct Environment {
    /// node 可执行文件
    pub node: PathBuf,
    /// node 版本，形如 24.22.0
    pub node_version: String,
    /// npm-cli.js 的路径，用 node 直接执行
    pub npm: PathBuf,
    /// git --version 的原始输出
    pub git: String,
}

/// 运行子进程的选项
pub struct RunOptions<'a> {
    /// 工作目录
    pub cwd: Option<&'a Path>,
    /// 输出去向
    pub log: LogFn,
    /// 取消信号；取消时回收进程树
    pub cancel: Option<&'a CancellationToken>,
}

/// 探测 node / npm / git
///
/// # 返回值
///
/// 见 Environment；缺少工具或 Node.js 版本过低时返回对应文案
pub async fn environment() -> Result<Environment, StudioError> {
    let missing_node = || StudioError::value(MessageKey::MissingTool, "tool", "Node.js");
    let probe = run_quiet(
        "node",
        &[
            "-p".to_string(),
            "JSON.stringify({path:process.execPath,version:process.versions.node})".to_string(),
        ],
    )
    .await
    .map_err(|_| missing_node())?;
    let value: serde_json::Value =
        serde_json::from_str(probe.trim()).map_err(|_| missing_node())?;
    let node = value
        .get("path")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(missing_node)?;
    let node_version = value
        .get("version")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let major = node_version
        .split('.')
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or_default();
    if major < 24 {
        return Err(StudioError::value(
            MessageKey::NodeVersion,
            "version",
            &node_version,
        ));
    }

    let node_dir = node.parent().unwrap_or_else(|| Path::new(""));
    #[cfg_attr(windows, allow(unused_mut))]
    let mut candidates = vec![
        node_dir.join("node_modules/npm/bin/npm-cli.js"),
        node_dir.join("../lib/node_modules/npm/bin/npm-cli.js"),
    ];
    // 非 Windows 的 node 安装常把 npm 放在别处，走 which 找真实路径
    #[cfg(not(windows))]
    {
        if let Ok(path) = run_quiet("which", &["npm".to_string()]).await {
            if let Ok(path) = tokio::fs::canonicalize(path.trim()).await {
                candidates.push(path);
            }
        }
    }
    let mut npm = None;
    for candidate in candidates {
        if tokio::fs::try_exists(&candidate).await.unwrap_or(false) {
            npm = Some(candidate);
            break;
        }
    }
    let npm = npm.ok_or_else(|| StudioError::value(MessageKey::MissingTool, "tool", "npm"))?;

    let git = run_quiet("git", &["--version".to_string()])
        .await
        .map_err(|_| StudioError::value(MessageKey::MissingTool, "tool", "Git"))?;

    Ok(Environment {
        node,
        node_version,
        npm,
        git: git.trim().to_string(),
    })
}

/// 执行命令并收集输出
///
/// - program: 可执行文件
/// - args: 参数数组，不经过 shell
/// - options: 工作目录 / 输出去向 / 取消信号
///
/// # 返回值
///
/// 去掉首尾空白的 stdout；stderr 只写进日志，不混进返回值。
/// 退出码非零时返回 commandFailed。
pub async fn run(
    program: &str,
    args: &[String],
    options: RunOptions<'_>,
) -> Result<String, StudioError> {
    let cancelled = || options.cancel.is_some_and(CancellationToken::is_cancelled);
    if cancelled() {
        return Err(StudioError::new(MessageKey::Cancelled));
    }

    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(cwd) = options.cwd {
        command.current_dir(cwd);
    }
    configure(&mut command);
    let mut child = command
        .spawn()
        .map_err(|error| StudioError::raw(format!("{program}: {error}")))?;
    let pid = child.id().unwrap_or_default();
    registry().add(pid);

    let (tx, mut rx) = mpsc::channel::<Event>(64);
    if let Some(stdout) = child.stdout.take() {
        spawn_reader(stdout, tx.clone(), options.log.clone(), true);
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_reader(stderr, tx.clone(), options.log.clone(), false);
    }
    let waiter = tx.clone();
    tokio::spawn(async move {
        let status = child.wait().await;
        let _ = waiter.send(Event::Exit(status)).await;
    });
    drop(tx);

    let mut output = String::new();
    let mut exit: Option<std::io::Result<ExitStatus>> = None;
    let mut killed = false;
    loop {
        if exit.is_some() {
            // 进程已退出，管道可能还被残留句柄占着，最多再收 300ms
            match tokio::time::timeout(Duration::from_millis(300), rx.recv()).await {
                Ok(Some(Event::Chunk(text, true))) => push_output(&mut output, &text),
                Ok(Some(Event::Chunk(_, false))) => {}
                Ok(Some(Event::Exit(_))) | Ok(None) | Err(_) => break,
            }
            continue;
        }
        tokio::select! {
            _ = wait_cancel(options.cancel), if !killed => {
                killed = true;
                kill_tree(pid).await;
            }
            event = rx.recv() => match event {
                Some(Event::Chunk(text, true)) => push_output(&mut output, &text),
                Some(Event::Chunk(_, false)) => {}
                Some(Event::Exit(status)) => exit = Some(status),
                None => exit = Some(Err(std::io::Error::other("process exited without status"))),
            },
        }
    }
    registry().remove(pid);

    if killed || cancelled() {
        return Err(StudioError::new(MessageKey::Cancelled));
    }
    let status =
        exit.ok_or_else(|| StudioError::raw(format!("{program}: missing exit status")))??;
    if !status.success() {
        let command = format!("{program} {}", args.join(" "));
        return Err(
            StudioError::value(MessageKey::CommandFailed, "command", command)
                .with("code", status.code().unwrap_or(-1).to_string()),
        );
    }
    Ok(output.trim().to_string())
}

/// 回收进程树
pub async fn kill_tree(pid: u32) {
    if pid == 0 {
        return;
    }
    #[cfg(windows)]
    {
        let mut command = Command::new("taskkill");
        command
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        configure(&mut command);
        let _ = command.status().await;
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &format!("-{pid}")])
            .status()
            .await;
    }
}

/// 同步回收进程树，只能用在退出清理里
pub fn kill_tree_sync(pid: u32) {
    if pid == 0 {
        return;
    }
    #[cfg(windows)]
    {
        let mut command = std::process::Command::new("taskkill");
        command
            .arg("/PID")
            .arg(pid.to_string())
            .arg("/T")
            .arg("/F")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        hide_console_std(&mut command);
        let _ = command.status();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &format!("-{pid}")])
            .status();
    }
}

/// 预览 worker 逐行读取输出
pub fn spawn_line_reader<R>(reader: R, log: LogFn, tx: mpsc::Sender<crate::preview::WorkerEvent>)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    use tokio::io::AsyncBufReadExt;
    tokio::spawn(async move {
        let mut reader = tokio::io::BufReader::new(reader);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let text = line.trim_end_matches(['\r', '\n']);
                    if let Some(message) = crate::preview::parse_worker_line(text) {
                        if tx.send(message).await.is_err() {
                            break;
                        }
                    } else {
                        log(&format!("{text}\n"));
                    }
                }
            }
        }
    });
}

/// 给子进程设置平台相关参数：隐藏控制台、非 Windows 独立进程组
pub fn configure(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.as_std_mut().creation_flags(0x0800_0000);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.as_std_mut().process_group(0);
    }
}

enum Event {
    Chunk(String, bool),
    Exit(std::io::Result<ExitStatus>),
}

fn spawn_reader<R: AsyncRead + Unpin + Send + 'static>(
    mut reader: R,
    tx: mpsc::Sender<Event>,
    log: LogFn,
    stdout: bool,
) {
    tokio::spawn(async move {
        let mut buffer = [0u8; 8192];
        loop {
            match reader.read(&mut buffer).await {
                Ok(0) | Err(_) => break,
                Ok(size) => {
                    let text = String::from_utf8_lossy(&buffer[..size]).into_owned();
                    log(&text);
                    if tx.send(Event::Chunk(text, stdout)).await.is_err() {
                        break;
                    }
                }
            }
        }
    });
}

async fn wait_cancel(cancel: Option<&CancellationToken>) {
    match cancel {
        Some(cancel) => cancel.cancelled().await,
        None => std::future::pending().await,
    }
}

async fn run_quiet(program: &str, args: &[String]) -> Result<String, StudioError> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure(&mut command);
    let output = command
        .output()
        .await
        .map_err(|error| StudioError::raw(format!("{program}: {error}")))?;
    if !output.status.success() {
        return Err(StudioError::raw(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn push_output(output: &mut String, text: &str) {
    output.push_str(text);
    if output.len() > OUTPUT_LIMIT {
        let mut cut = output.len() - OUTPUT_LIMIT;
        while !output.is_char_boundary(cut) {
            cut += 1;
        }
        output.drain(..cut);
    }
}

#[cfg(windows)]
fn hide_console_std(command: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x0800_0000);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 返回值只算 stdout；stderr 只进日志
    #[tokio::test]
    async fn returns_stdout_only() {
        let output = run(
            "node",
            &[
                "-e".to_string(),
                "console.log('out'); console.error('err')".to_string(),
            ],
            RunOptions {
                cwd: None,
                log: no_log(),
                cancel: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(output, "out");
    }
}
