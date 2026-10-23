//! 提权进程的双路输出转发，管道句柄只交接一次。

use std::{collections::HashMap, fs::File, io::{BufRead, BufReader, Write},
    os::windows::{io::{FromRawHandle, IntoRawHandle, OwnedHandle}, process::CommandExt},
    path::Path, process::{Command, Stdio}, time::{Duration, Instant}};
use base64::{Engine, engine::general_purpose::STANDARD};
use windows::{core::{HSTRING, HRESULT}, Win32::{Foundation::{HANDLE, INVALID_HANDLE_VALUE, ERROR_PIPE_CONNECTED, ERROR_PIPE_LISTENING, ERROR_NO_DATA},
    Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES,
    System::Pipes::{CreateNamedPipeW, ConnectNamedPipe, SetNamedPipeHandleState, NAMED_PIPE_MODE}}};
use mml_names::i18_items::error_type::{CoreResult, ErrorType};
use super::{LaunchResult, OutputStream, process_err};

const NO_WINDOW: u32 = 0x08000000;

/// 创建只允许本机连接的同步字节管道。
/// - `name`: 不含系统前缀的唯一管道名。
/// # 返回值
/// 调用方负责关闭或转移所有权的句柄。
pub fn create_named_pipe(name: &str) -> CoreResult<HANDLE> {
    let name = HSTRING::from(format!(r"\\.\pipe\{name}"));
    let handle = unsafe {
        CreateNamedPipeW(&name, FILE_FLAGS_AND_ATTRIBUTES(0x3 | 0x80000),
            NAMED_PIPE_MODE(0x1 | 0x8), 1, 65536, 65536, 0, None)
    };
    if handle == INVALID_HANDLE_VALUE { return Err(process_err(format!("CreateNamedPipeW: {}", std::io::Error::last_os_error()))); }
    Ok(handle)
}

/// 等待客户端连接并将同步管道交给文件读取器。
/// - `handle`: 转移所有权的句柄，成功或失败都会由本函数接管。
/// - `timeout_ms`: 连接时限，单位毫秒。
/// # 返回值
/// 可阻塞读取的文件，或超时错误。
pub fn connect_named_pipe_with_timeout(handle: HANDLE, timeout_ms: u32) -> CoreResult<File> {
    let owned = unsafe { OwnedHandle::from_raw_handle(handle.0) };
    let deadline = Instant::now() + Duration::from_millis(timeout_ms.into());
    loop {
        match unsafe { ConnectNamedPipe(handle, None) } {
            Ok(()) => break,
            Err(e) if [ERROR_PIPE_CONNECTED, ERROR_NO_DATA].iter().any(|code| e.code() == HRESULT::from_win32(code.0)) => break,
            Err(e) if e.code() == HRESULT::from_win32(ERROR_PIPE_LISTENING.0) => {
                if Instant::now() >= deadline { return Err(ErrorType::NamedPipeConnectTimeout); }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => return Err(process_err(e)),
        }
    }
    unsafe { SetNamedPipeHandleState(handle, Some(&NAMED_PIPE_MODE(0)), None, None) }.map_err(process_err)?;
    Ok(File::from(owned))
}

fn ps_literal(value: &str) -> String { format!("'{}'", value.replace('\'', "''")) }

// ProcessStartInfo.Arguments 使用 Windows CRT 的引号与反斜杠规则。
fn quote_arg(value: &str) -> String {
    let mut quoted = String::from("\"");
    let mut slashes = 0;
    for ch in value.chars() {
        if ch == '\\' { slashes += 1; continue; }
        quoted.push_str(&"\\".repeat(if ch == '"' { slashes * 2 + 1 } else { slashes }));
        quoted.push(ch);
        slashes = 0;
    }
    quoted.push_str(&"\\".repeat(slashes * 2));
    quoted.push('"');
    quoted
}

fn encoded(script: &str) -> String {
    STANDARD.encode(script.encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<_>>())
}

fn powershell() -> Command {
    let mut command = Command::new("powershell.exe");
    command.args(["-NoProfile", "-NonInteractive"]).creation_flags(NO_WINDOW);
    command
}

pub(super) fn restart_elevated(path: &Path, args: &[String]) -> CoreResult<()> {
    let arguments = args.iter().map(|s| quote_arg(s)).collect::<Vec<_>>().join(" ");
    let script = format!("$ErrorActionPreference='Stop'; Start-Process -FilePath {} -ArgumentList {} -Verb RunAs -WindowStyle Hidden | Out-Null",
        ps_literal(&path.to_string_lossy()), ps_literal(&arguments));
    let result = powershell().args(["-EncodedCommand", &encoded(&script)]).output().map_err(process_err)?;
    if !result.status.success() { return Err(process_err(String::from_utf8_lossy(&result.stderr))); }
    Ok(())
}

struct HelperScript(std::path::PathBuf);
impl Drop for HelperScript {
    fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); }
}

/// 启动提权辅助进程并接收目标程序的两路日志。
/// - `path`: 目标可执行文件。
/// - `args`: 原始参数数组。
/// - `env`: 附加环境变量。
/// - `working_dir`: 目标工作目录。
/// # 返回值
/// 辅助进程及两路输出；提权取消或连接失败返回错误。
pub fn launch_with_elevation_windows<P: AsRef<Path>>(
    path: P, args: Vec<String>, env: HashMap<String, String>, working_dir: P,
) -> CoreResult<LaunchResult> {
    launch_helper(path.as_ref(), args, env, working_dir.as_ref(), true)
}

fn launch_helper(path: &Path, args: Vec<String>, env: HashMap<String, String>, working_dir: &Path, elevate: bool) -> CoreResult<LaunchResult> {
    let id = uuid::Uuid::new_v4();
    let output_name = format!("mml-out-{id}");
    let error_name = format!("mml-err-{id}");
    let output = unsafe { OwnedHandle::from_raw_handle(create_named_pipe(&output_name)?.0) };
    let error = unsafe { OwnedHandle::from_raw_handle(create_named_pipe(&error_name)?.0) };
    let args = args.iter().map(|s| quote_arg(s)).collect::<Vec<_>>().join(" ");
    let environment = env.iter().map(|(key, value)| format!("$si.EnvironmentVariables[{}]={};", ps_literal(key), ps_literal(value))).collect::<String>();
    let script = format!(r#"$ErrorActionPreference='Stop'
$op = [IO.Pipes.NamedPipeClientStream]::new('.', '{output_name}', [IO.Pipes.PipeDirection]::Out)
$ep = [IO.Pipes.NamedPipeClientStream]::new('.', '{error_name}', [IO.Pipes.PipeDirection]::Out)
$code = 1
try {{
  $op.Connect(15000); $ep.Connect(15000)
  $si = [Diagnostics.ProcessStartInfo]::new()
  $si.FileName={path}; $si.Arguments={args}; $si.WorkingDirectory={working_dir}
  $si.UseShellExecute=$false; $si.CreateNoWindow=$true
  $si.RedirectStandardOutput=$true; $si.RedirectStandardError=$true
  {environment}
  $p = [Diagnostics.Process]::Start($si)
  $outTask = $p.StandardOutput.BaseStream.CopyToAsync($op)
  $errTask = $p.StandardError.BaseStream.CopyToAsync($ep)
  $p.WaitForExit()
  [Threading.Tasks.Task]::WaitAll([Threading.Tasks.Task[]]@($outTask, $errTask))
  $code = $p.ExitCode
}} catch {{
  if ($ep.IsConnected) {{ $bytes=[Text.Encoding]::UTF8.GetBytes($_.ToString()); $ep.Write($bytes,0,$bytes.Length) }}
}} finally {{ $op.Dispose(); $ep.Dispose() }}
exit $code
"#, path=ps_literal(&path.to_string_lossy()), args=ps_literal(&args), working_dir=ps_literal(&working_dir.to_string_lossy()));
    #[cfg(test)]
    let folder = mml_testutil::temp_dir();
    #[cfg(not(test))]
    let folder = std::env::temp_dir();
    std::fs::create_dir_all(&folder).map_err(process_err)?;
    let script_file = HelperScript(folder.join(format!("mml-elevation-{id}.ps1")));
    let mut file = File::create_new(&script_file.0).map_err(process_err)?;
    // Windows PowerShell 按 BOM 识别脚本编码，路径和环境变量中的中文也需保真。
    file.write_all(&[0xef, 0xbb, 0xbf]).and_then(|_| file.write_all(script.as_bytes())).map_err(process_err)?;
    drop(file);
    let arguments = format!("-NoProfile -NonInteractive -ExecutionPolicy Bypass -File {}", quote_arg(&script_file.0.to_string_lossy()));
    let wrapper = format!("$ErrorActionPreference='Stop'; $p=Start-Process powershell.exe {} -WindowStyle Hidden -PassThru -ArgumentList {}; [Console]::Out.WriteLine($p.Id); $p.WaitForExit(); exit $p.ExitCode",
        if elevate { "-Verb RunAs" } else { "" }, ps_literal(&arguments));
    let mut child = powershell().args(["-EncodedCommand", &encoded(&wrapper)])
        .stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(process_err)?;
    let stdout = child.stdout.take().unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(stdout).read_line(&mut line).map(|_| line.trim().parse::<u32>().ok());
        let _ = sender.send(result);
    });
    let connected = (|| {
        let pid = receiver.recv_timeout(Duration::from_secs(90)).map_err(process_err)?
            .map_err(process_err)?.ok_or_else(|| process_err("Elevation cancelled or helper failed"))?;
        let stdout = connect_named_pipe_with_timeout(HANDLE(output.into_raw_handle()), 15000)?;
        let stderr = connect_named_pipe_with_timeout(HANDLE(error.into_raw_handle()), 15000)?;
        Ok::<_, ErrorType>((pid, stdout, stderr))
    })();
    match connected {
        Ok((pid, stdout, stderr)) => Ok(LaunchResult { child, is_admin: true, pid: Some(pid),
            stdout: Some(OutputStream::File(stdout)), stderr: Some(OutputStream::File(stderr)) }),
        Err(error) => { let _ = child.kill(); let _ = child.wait(); Err(error) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn helper_preserves_environment_and_drains_both_streams() {
        let mut environment = HashMap::new();
        environment.insert("MML_PIPE_TEST".into(), "space ' quote 中文".into());
        let script = "[Console]::Out.WriteLine([Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($env:MML_PIPE_TEST))); [Console]::Error.Write(('E'*131072)); [Console]::Out.Write(('O'*131072)); exit 7";
        let mut result = launch_helper(Path::new("powershell.exe"), vec!["-NoProfile".into(), "-NonInteractive".into(),
            "-EncodedCommand".into(), encoded(script)], environment, &std::env::current_dir().unwrap(), false).unwrap();
        let mut stdout = result.stdout.take().unwrap();
        let mut stderr = result.stderr.take().unwrap();
        let out = std::thread::spawn(move || { let mut bytes=Vec::new(); stdout.read_to_end(&mut bytes).unwrap(); bytes });
        let err = std::thread::spawn(move || { let mut bytes=Vec::new(); stderr.read_to_end(&mut bytes).unwrap(); bytes });
        assert_eq!(result.child.wait().unwrap().code(), Some(7));
        let out = out.join().unwrap();
        assert!(String::from_utf8_lossy(&out).contains(&STANDARD.encode("space ' quote 中文")));
        assert!(out.ends_with(&vec![b'O'; 131072]));
        assert_eq!(err.join().unwrap(), vec![b'E'; 131072]);
    }

    #[test]
    fn helper_preserves_empty_quotes_and_trailing_backslashes() {
        let file = HelperScript(mml_testutil::temp_path(format!("mml args ' 中文 {}.ps1", uuid::Uuid::new_v4())));
        std::fs::write(&file.0, "param([string]$a,[string]$b,[string]$c)\nforeach($v in @($a,$b,$c)){[Console]::Out.WriteLine([Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($v)))}").unwrap();
        let expected = ["", "sp ace ' \" 中文", "ends\\"];
        let mut args = vec!["-NoProfile".into(), "-NonInteractive".into(), "-ExecutionPolicy".into(), "Bypass".into(), "-File".into(), file.0.to_string_lossy().into_owned()];
        args.extend(expected.iter().map(|s| s.to_string()));
        let mut result = launch_helper(Path::new("powershell.exe"), args, HashMap::new(), &std::env::current_dir().unwrap(), false).unwrap();
        let mut out = String::new();
        result.stdout.take().unwrap().read_to_string(&mut out).unwrap();
        assert!(result.child.wait().unwrap().success());
        assert_eq!(out.lines().collect::<Vec<_>>(), expected.map(|s| STANDARD.encode(s)));
    }
}
