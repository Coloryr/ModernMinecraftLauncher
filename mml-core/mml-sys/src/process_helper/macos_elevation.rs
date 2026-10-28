//! AppleScript 与 POSIX shell 各自转义，提权后的目录和环境显式传递。
//!
//! 提权进程的输出走**命名管道**：`do shell script` 会把命令输出攒成脚本返回值，
//! 命令结束前 osascript 一个字都不吐，日志窗口在游戏运行期间只能是空的。
//! 改成把 stdout / stderr 重定向到 FIFO，启动器读另一端，就与 Windows 的命名管道一致了。

use std::{collections::HashMap, path::Path, process::Command};

// FIFO 只在 unix 上存在；这里按 unix 收窄，Windows 的测试构建只编到命令拼装
#[cfg(unix)]
use std::{
    ffi::CString,
    fs::{File, OpenOptions},
    io::{self, Read},
    path::PathBuf,
    sync::mpsc::{self, Receiver},
};

/// 提权进程的日志管道
#[cfg(unix)]
pub(super) struct LogPipe {
    /// FIFO 路径；提权脚本收尾时自己删掉
    pub(super) path: PathBuf,
    /// 读端，交给 `LaunchResult.stdout`
    pub(super) reader: PipeReader,
}

/// FIFO 读端
///
/// 打开 FIFO 读端会阻塞到有写端连上来，而提权脚本要等用户点完密码框才执行，
/// 所以把 `open` 放到后台线程，`read` 先等它把句柄送过来。之后就是普通的管道读：
/// 提权进程退出、写端关闭时自然读到 EOF。
#[cfg(unix)]
pub(super) struct PipeReader {
    /// 后台线程打开好的读句柄
    opened: Receiver<File>,
    /// 已经拿到的句柄
    file: Option<File>,
}

#[cfg(unix)]
impl PipeReader {
    /// 等后台线程把读句柄送过来
    fn handle(&mut self) -> io::Result<&mut File> {
        if self.file.is_none() {
            let file = self.opened.recv().map_err(|_| {
                io::Error::new(io::ErrorKind::BrokenPipe, "日志管道没有建立起来")
            })?;
            self.file = Some(file);
        }
        Ok(self.file.as_mut().expect("刚刚填过"))
    }
}

#[cfg(unix)]
impl Read for PipeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.handle()?.read(buf)
    }
}

/// 保留 shell 参数的字面值，包括空串、单引号和命令替换符。
fn shell_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// 建一个 FIFO 给提权进程写日志
///
/// # 返回值
///
/// 建不出 FIFO（文件系统不支持等）时返回 `None`，调用方退回旧的捕获方式
#[cfg(unix)]
pub(super) fn create_log_pipe() -> Option<LogPipe> {
    let path = std::env::temp_dir().join(format!("mml-launch-{}.fifo", uuid::Uuid::new_v4()));
    let raw = CString::new(path.to_string_lossy().as_bytes()).ok()?;

    // SAFETY: 参数是合法的 C 字符串指针，mkfifo 不保留它
    let created = unsafe { libc::mkfifo(raw.as_ptr(), 0o600) };
    if created != 0 {
        return None;
    }

    let (tx, opened) = mpsc::channel();
    let target = path.clone();
    std::thread::spawn(move || {
        // 写端连上之前一直阻塞在这里；用户取消提权时这个线程会等到进程退出
        if let Ok(file) = OpenOptions::new().read(true).open(&target) {
            let _ = tx.send(file);
        }
    });

    Some(LogPipe {
        path,
        reader: PipeReader { opened, file: None },
    })
}

fn shell_command(
    path: &Path,
    args: &[String],
    mut env: HashMap<String, String>,
    working_dir: &Path,
    log: Option<&Path>,
) -> String {
    for key in ["HOME", "LANG", "LC_ALL"] {
        if let Ok(value) = std::env::var(key) {
            env.entry(key.into()).or_insert(value);
        }
    }
    let mut command = format!(
        "cd {} && /usr/bin/env",
        shell_literal(&working_dir.to_string_lossy())
    );
    for (key, value) in env {
        command.push(' ');
        command.push_str(&shell_literal(&format!("{key}={value}")));
    }
    command.push(' ');
    command.push_str(&shell_literal(&path.to_string_lossy()));
    for arg in args {
        command.push(' ');
        command.push_str(&shell_literal(arg));
    }

    match log {
        // 输出进管道，脚本收尾时自己删掉；读端在我们手里，删文件不影响它
        Some(log) => {
            let log = shell_literal(&log.to_string_lossy());
            command.push_str(&format!(" > {log} 2>&1; rm -f {log}"));
        }
        // 没有管道时退回旧写法：合并到脚本返回值，退出后一次性可见
        None => command.push_str(" 2>&1"),
    }
    command
}

/// 构造提权命令，shell 脚本通过 AppleScript 参数传入，避免再嵌套一层字符串。
///
/// - `path`: 目标程序路径
/// - `args`: 原始参数
/// - `env`: 目标程序附加环境
/// - `working_dir`: 提权后使用的工作目录
/// - `log`: 日志管道路径；`None` 表示退回"输出合并进脚本返回值"
///
/// # 返回值
///
/// 尚未执行的 osascript 命令。
pub(super) fn command(
    path: &Path,
    args: &[String],
    env: HashMap<String, String>,
    working_dir: &Path,
    log: Option<&Path>,
) -> Command {
    let mut command = Command::new("/usr/bin/osascript");
    command.args(["-e", "on run argv", "-e", "do shell script (item 1 of argv) with administrator privileges without altering line endings", "-e", "end run", "--"]);
    command.arg(shell_command(path, args, env, working_dir, log));
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_preserves_arguments_environment_and_working_directory() {
        #[cfg(windows)]
        let shell = std::env::split_paths(&std::env::var_os("PATH").unwrap())
            .flat_map(|dir| [dir.join("sh.exe"), dir.join("../bin/sh.exe")])
            .find(|path| path.is_file())
            .expect("Windows 验证使用 Git 随附的 POSIX shell");
        #[cfg(not(windows))]
        let shell = std::path::PathBuf::from("/bin/sh");
        let root = mml_testutil::temp_path(format!("mac elevation ' $ {}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let values = [
            "",
            "sp ace ' \" 中文",
            "$(printf injected)",
            "`printf injected`",
            "ends\\",
            "line\nbreak",
        ];
        let mut args = vec![
            "-c".into(),
            "printf '%s\\0' \"$MML_TEST_VALUE\" \"$PWD\" \"$@\"".into(),
            "mml".into(),
        ];
        args.extend(values.iter().map(|value| value.to_string()));
        let command = command(
            Path::new("/bin/sh"),
            &args,
            HashMap::from([("MML_TEST_VALUE".into(), "value ' $ ` 中文".into())]),
            &root,
            None,
        );
        let script = command.get_args().last().unwrap();
        let output = Command::new(shell)
            .args(["-c"])
            .arg(script)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let parts: Vec<_> = output.stdout.split(|byte| *byte == 0).collect();
        assert_eq!(parts[0], "value ' $ ` 中文".as_bytes());
        let cwd = String::from_utf8_lossy(parts[1]);
        assert!(cwd.ends_with(root.file_name().unwrap().to_str().unwrap()));
        assert_eq!(&parts[2..parts.len() - 1], values.map(str::as_bytes));
        std::fs::remove_dir_all(root).unwrap();
    }

    /// 走日志管道时，目标进程的输出要能从读端逐行拿到，脚本收尾还会删掉 FIFO
    #[cfg(unix)]
    #[test]
    fn log_pipe_carries_output_and_is_removed() {
        let root = mml_testutil::temp_path(format!("mac log fifo {}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();

        let pipe = create_log_pipe().expect("应能建出 FIFO");
        let command = command(
            Path::new("/bin/sh"),
            &["-c".into(), "echo hello-fifo; echo oops >&2".into()],
            HashMap::new(),
            &root,
            Some(&pipe.path),
        );
        let script = command.get_args().last().unwrap();
        let mut child = Command::new("/bin/sh")
            .args(["-c"])
            .arg(script)
            .spawn()
            .expect("脚本应能启动");

        let mut reader = std::io::BufReader::new(pipe.reader);
        let mut lines = Vec::new();
        for line in std::io::BufRead::lines(&mut reader) {
            lines.push(line.unwrap());
        }
        child.wait().unwrap();

        assert_eq!(lines, ["hello-fifo", "oops"], "stdout 与 stderr 都要进管道");
        assert!(!pipe.path.exists(), "脚本收尾应删掉 FIFO");
        std::fs::remove_dir_all(root).unwrap();
    }
}
