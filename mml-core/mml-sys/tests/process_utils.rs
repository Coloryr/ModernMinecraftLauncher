use std::collections::HashMap;
use std::io::Read;

use mml_sys::process_helper;

// ============================================================================
// 测试辅助函数
// ============================================================================

/// 跨平台测试用命令
struct TestCommand {
    /// 可执行文件名或路径；Windows 为 `cmd`，其它平台为 `/bin/echo`
    cmd: &'static str,
    /// 传给可执行文件的参数列表，末尾为待回显的文本 `hello_test`
    args: Vec<String>,
}

/// 获取跨平台测试用命令
fn get_test_echo_cmd() -> TestCommand {
    #[cfg(target_os = "windows")]
    {
        TestCommand {
            cmd: "cmd",
            args: vec!["/c".into(), "echo".into(), "hello_test".into()],
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        TestCommand {
            cmd: "/bin/echo",
            args: vec!["hello_test".into()],
        }
    }
}

/// 获取测试用工作目录，固定为当前目录，仅用于执行轻量命令
fn test_working_dir() -> &'static str {
    "."
}

// ============================================================================
// is_run_as_admin 测试
// ============================================================================

/// is_run_as_admin 不应 panic，返回布尔值
#[test]
fn test_is_run_as_admin_returns_bool() {
    let result = process_helper::is_run_as_admin();
    // 不应该 panic，返回 bool
    assert!(result == true || result == false);
}

// ============================================================================
// launch 普通启动测试
// ============================================================================

/// 普通启动能捕获 stdout 与 stderr
#[test]
fn test_launch_normal_captures_stdout() {
    let cmd = get_test_echo_cmd();
    let result = process_helper::launch(
        cmd.cmd,
        cmd.args,
        HashMap::new(),
        test_working_dir(),
        false, // 不提权
    )
    .expect("launch should succeed");

    // 普通启动校验
    assert!(!result.is_admin, "normal launch: is_admin should be false");
    assert!(result.pid.is_none(), "normal launch: pid should be None");
    assert!(
        result.stdout.is_some(),
        "normal launch: stdout should be captured"
    );
    assert!(
        result.stderr.is_some(),
        "normal launch: stderr should be captured"
    );

    // 读取 stdout 验证内容
    let mut stdout = result.stdout.unwrap();
    let mut buf = String::new();
    stdout.read_to_string(&mut buf).expect("read stdout");
    assert!(
        buf.contains("hello_test"),
        "stdout should contain 'hello_test', got: '{}'",
        buf.trim()
    );
}

/// launch 返回的 child 句柄可 try_wait
#[test]
fn test_launch_result_child_wait() {
    let cmd = get_test_echo_cmd();
    let mut result =
        process_helper::launch(cmd.cmd, cmd.args, HashMap::new(), test_working_dir(), false)
            .expect("launch should succeed");

    // try_wait 可能在进程退出后返回 Some，也可能进程还在运行返回 None
    let status = result.child.try_wait().expect("try_wait should not error");
    // echo 命令很快完成，大概率已经退出
    if let Some(exit) = status {
        assert!(exit.success(), "echo should exit with success");
    }
}

// ============================================================================
// launch 管理员启动测试
// ============================================================================

/// 测试一：当前已是管理员 → launch(admin=true) 应直接 spawn
///
/// 期望：
/// - `is_run_as_admin()` = true
/// - `LaunchResult.is_admin` = false，不走提权，直接 spawn
/// - stdout / stderr 正常捕获
#[test]
fn test_launch_admin_when_is_admin() {
    let running_as_admin = process_helper::is_run_as_admin();
    eprintln!("=== is_run_as_admin() = {running_as_admin} ===");

    if !running_as_admin {
        eprintln!("SKIP: not admin — this test only covers the 'already admin' path");
        return;
    }

    let cmd = get_test_echo_cmd();
    let mut result =
        process_helper::launch(cmd.cmd, cmd.args, HashMap::new(), test_working_dir(), true)
            .expect("launch should succeed");

    eprintln!(
        "Current process: admin (is_run_as_admin=true)\n\
         LaunchResult.is_admin = {} (expected: false, because direct spawn)\n\
         LaunchResult.pid     = {:?} (expected: None)\n\
         LaunchResult.stdout  = {} (expected: Some)\n\
         LaunchResult.stderr  = {} (expected: Some)",
        result.is_admin,
        result.pid,
        result.stdout.is_some(),
        result.stderr.is_some(),
    );

    assert!(
        !result.is_admin,
        "already admin → direct spawn → is_admin must be false, got true"
    );
    assert!(
        result.pid.is_none(),
        "direct spawn → pid must be None, got {:?}",
        result.pid
    );
    assert!(
        result.stdout.is_some(),
        "stdout should be captured even with admin=true"
    );
    assert!(
        result.stderr.is_some(),
        "stderr should be captured even with admin=true"
    );

    // 验证 stdout 内容
    let mut stdout = result.stdout.unwrap();
    let mut out_buf = String::new();
    stdout.read_to_string(&mut out_buf).expect("read stdout");
    assert!(
        out_buf.contains("hello_test"),
        "stdout should contain 'hello_test', got: '{}'",
        out_buf.trim()
    );

    // 验证 stderr，echo 不输出到 stderr
    let mut stderr = result.stderr.unwrap();
    let mut err_buf = String::new();
    stderr.read_to_string(&mut err_buf).expect("read stderr");
    assert!(
        err_buf.trim().is_empty(),
        "stderr should be empty for echo, got: '{}'",
        err_buf.trim()
    );

    // 验证退出码
    let exit_status = result.child.wait().expect("wait for child");
    assert!(exit_status.success(), "child should exit with success");
}

/// 测试二：当前非管理员 → launch(admin=true) 应走提权路径
///
/// 期望：
/// - `is_run_as_admin()` = false
/// - `LaunchResult.is_admin` = true，走了提权路径
/// - 行为和结果取决于平台提权机制：UAC / pkexec / osascript
#[test]
fn test_launch_admin_when_not_admin() {
    let running_as_admin = process_helper::is_run_as_admin();
    eprintln!("=== is_run_as_admin() = {running_as_admin} ===");

    if running_as_admin {
        eprintln!("SKIP: running as admin — this test covers the non-admin elevation path");
        return;
    }

    // 先验证普通启动不受影响
    let cmd = get_test_echo_cmd();
    let normal = process_helper::launch(
        cmd.cmd,
        cmd.args.clone(),
        HashMap::new(),
        test_working_dir(),
        false,
    )
    .expect("normal launch should succeed");
    assert!(!normal.is_admin, "normal launch: is_admin should be false");

    // 提权启动
    let elevated =
        process_helper::launch(cmd.cmd, cmd.args, HashMap::new(), test_working_dir(), true);

    eprintln!(
        "Current process: non-admin (is_run_as_admin=false)\n\
         LaunchResult.is_admin = {} (expected: true if elevation triggered)\n\
         LaunchResult.pid     = {:?}\n\
         LaunchResult.stdout  = {} (expected: Some on Linux/macOS, Some/None on Windows)\n\
         LaunchResult.stderr  = {} (expected: Some on Linux, None on macOS)\n\
         Platform notes:\n\
           Windows: UAC dialog; if declined → is_admin=true, pipes timeout\n\
           Linux:   pkexec password; if cancelled → is_admin=true, stderr has error\n\
           macOS:   osascript dialog; if cancelled → is_admin=true",
        elevated
            .as_ref()
            .map_or("(error)", |r| if r.is_admin { "true" } else { "false" }),
        elevated.as_ref().ok().and_then(|r| r.pid),
        elevated.as_ref().map_or(false, |r| r.stdout.is_some()),
        elevated.as_ref().map_or(false, |r| r.stderr.is_some()),
    );

    if let Ok(r) = elevated {
        assert!(
            r.is_admin,
            "non-admin elevation: is_admin must be true, got false"
        );
    }
    // 如果 launch 返回 Err，如 pkexec / osascript 不在 PATH，也是可接受的
}

// ============================================================================
// LaunchResult 结构体字段测试
// ============================================================================

/// 普通启动时 is_admin 与 pid.is_some() 一致
#[test]
fn test_launch_result_fields_consistency() {
    let cmd = get_test_echo_cmd();
    let result =
        process_helper::launch(cmd.cmd, cmd.args, HashMap::new(), test_working_dir(), false)
            .expect("launch should succeed");

    // is_admin 和 pid 的一致性：普通启动时 pid 应为 None
    assert_eq!(
        result.is_admin,
        result.pid.is_some(),
        "normal launch: is_admin should match pid.is_some()"
    );
}

// ============================================================================
// Linux 管理员启动测试 (pkexec)
// ============================================================================

/// Linux 提权路径测试，依赖 pkexec
#[cfg(target_os = "linux")]
mod linux_admin_tests {
    use std::collections::HashMap;
    use std::io::Read;

    use mml_sys::process_helper;

    /// 非 root 用户请求提权时会调用 pkexec；无 TTY 的 CI 环境中 pkexec 会失败，
    /// 但 launch() 本身不应 panic，且返回的 is_admin 应为 true
    #[test]
    fn test_launch_admin_linux_elevation_path() {
        if process_helper::is_run_as_admin() {
            eprintln!("SKIP: already root, pkexec elevation path not tested");
            return;
        }

        let result = process_helper::launch(
            "/bin/echo",
            vec!["hello_admin".into()],
            HashMap::new(),
            ".",
            true,
        );

        match result {
            Ok(r) => {
                // 走的是提权路径
                assert!(r.is_admin, "linux elevation: is_admin should be true");
                // pkexec 被 spawn 了，stdout/stderr 是 piped 的
                assert!(r.stdout.is_some(), "pkexec stdout should be piped");
                assert!(r.stderr.is_some(), "pkexec stderr should be piped");
                // 在无 TTY 环境下 pkexec 会失败，stderr 可能包含错误信息
                let mut stderr = r.stderr.unwrap();
                let mut err_buf = String::new();
                let _ = stderr.read_to_string(&mut err_buf);
                eprintln!("pkexec stderr (may contain auth error): {}", err_buf.trim());
            }
            Err(e) => {
                // pkexec 不在 PATH 上会直接 Err
                eprintln!(
                    "pkexec launch error (expected if pkexec not installed): {:?}",
                    e
                );
            }
        }
    }

    /// 以 root 身份运行时，admin=true 应直接 spawn，不走 pkexec
    #[test]
    fn test_launch_admin_linux_when_root() {
        if !process_helper::is_run_as_admin() {
            eprintln!("SKIP: not root, direct admin path not tested");
            return;
        }

        let result = process_helper::launch(
            "/bin/echo",
            vec!["hello_root".into()],
            HashMap::new(),
            ".",
            true,
        )
        .expect("launch as root should succeed");

        // root 用户 → 直接 spawn，不走提权
        assert!(!result.is_admin, "already root: is_admin should be false");
        assert!(result.stdout.is_some(), "stdout should be captured");
    }
}

// ============================================================================
// macOS 管理员启动测试 (osascript)
// ============================================================================

/// macOS 提权路径测试，依赖 osascript
#[cfg(target_os = "macos")]
mod macos_admin_tests {
    use std::collections::HashMap;
    use std::io::Read;

    use mml_sys::process_helper;

    /// 非 root 用户请求提权时会调用 osascript；无 GUI 的 CI 环境中 osascript 会失败，
    /// 但 launch() 本身不应 panic，且返回的 is_admin 应为 true
    #[test]
    fn test_launch_admin_macos_elevation_path() {
        if process_helper::is_run_as_admin() {
            eprintln!("SKIP: already root, osascript elevation path not tested");
            return;
        }

        let result = process_helper::launch(
            "/bin/echo",
            vec!["hello_admin".into()],
            HashMap::new(),
            ".",
            true,
        );

        match result {
            Ok(r) => {
                assert!(r.is_admin, "macOS elevation: is_admin should be true");
                // osascript stdout 是 piped 的，stderr 已通过 2>&1 合并到 stdout
                assert!(r.stdout.is_some(), "osascript stdout should be piped");
                // macOS 上 stderr 被合并到 stdout，所以 stderr 为 None
                assert!(
                    r.stderr.is_none(),
                    "macOS: stderr should be None (merged into stdout)"
                );
            }
            Err(e) => {
                eprintln!(
                    "osascript launch error (expected if no GUI session): {:?}",
                    e
                );
            }
        }
    }

    /// 以 root 身份运行时，admin=true 应直接 spawn，不走 osascript
    #[test]
    fn test_launch_admin_macos_when_root() {
        if !process_helper::is_run_as_admin() {
            eprintln!("SKIP: not root, direct admin path not tested");
            return;
        }

        let result = process_helper::launch(
            "/bin/echo",
            vec!["hello_root".into()],
            HashMap::new(),
            ".",
            true,
        )
        .expect("launch as root should succeed");

        assert!(!result.is_admin, "already root: is_admin should be false");
        assert!(result.stdout.is_some(), "stdout should be captured");
    }
}

// ============================================================================
// Windows 命名管道单元测试
// ============================================================================

/// Windows 命名管道与提权链路测试
#[cfg(target_os = "windows")]
mod windows_pipe_tests {
    use std::io::{BufRead, Read, Write};
    use std::os::windows::io::{IntoRawHandle, FromRawHandle, OwnedHandle};
    use std::sync::Arc;
    use std::time::Duration;

    use mml_sys::process_helper;
    use windows::Win32::Foundation::HANDLE;
    use windows::core::Free;

    /// 打开命名管道客户端，模拟提权进程通过 -RedirectStandardOutput 连接管道
    fn open_pipe_client(name: &str) -> std::io::Result<std::fs::File> {
        std::fs::OpenOptions::new()
            .write(true)
            .open(format!(r"\\.\pipe\{}", name))
    }

    /// create_named_pipe 返回有效句柄
    #[test]
    fn test_create_named_pipe_success() {
        let name = format!("test_pipe_create_{}", std::process::id());
        let mut handle =
            process_helper::create_named_pipe(&name).expect("create_named_pipe should succeed");

        // 有效 HANDLE 不是 INVALID_HANDLE_VALUE
        assert!(!handle.is_invalid(), "handle should be valid");

        unsafe {
            handle.free();
        }
    }

    /// 命名管道连接后可读到客户端写入的数据
    #[test]
    fn test_named_pipe_connect_and_read() {
        let name = format!("test_pipe_rw_{}", std::process::id());

        let handle = process_helper::create_named_pipe(&name).expect("create pipe");

        let pipe_name = name.clone();
        let client = std::thread::spawn(move || {
            let mut file = open_pipe_client(&pipe_name).expect("client open pipe");
            file.write_all(b"hello from pipe").expect("client write");
        });

        let mut server_file = process_helper::connect_named_pipe_with_timeout(handle, 5000)
            .expect("connect should succeed");

        let mut buf = Vec::new();
        server_file.read_to_end(&mut buf).expect("server read");
        assert_eq!(buf, b"hello from pipe");

        client.join().expect("client thread join");
    }

    /// 无客户端连接时按超时返回错误
    #[test]
    fn test_named_pipe_timeout() {
        let name = format!("test_pipe_timeout_{}", std::process::id());

        let handle = process_helper::create_named_pipe(&name).expect("create pipe");

        let result = process_helper::connect_named_pipe_with_timeout(handle, 200);
        assert!(result.is_err(), "should timeout with no client");
        let err_msg = format!("{:?}", result.unwrap_err());
        assert!(
            err_msg.contains("NamedPipeConnectTimeout"),
            "error should mention timeout: {}",
            err_msg
        );
    }

    /// 两次创建命名管道得到不同句柄
    #[test]
    fn test_named_pipe_multiple_pipes() {
        let name_a = format!("test_pipe_multi_a_{}", std::process::id());
        let name_b = format!("test_pipe_multi_b_{}", std::process::id());

        let mut h1 = process_helper::create_named_pipe(&name_a).expect("pipe A");
        let mut h2 = process_helper::create_named_pipe(&name_b).expect("pipe B");

        assert_ne!(h1, h2, "different pipes should have different handles");

        unsafe {
            h1.free();
            h2.free();
        }
    }

    /// 管道可完整传输 64 KiB 数据
    #[test]
    fn test_named_pipe_large_data() {
        let name = format!("test_pipe_large_{}", std::process::id());

        let handle = process_helper::create_named_pipe(&name).expect("create pipe");

        let pipe_name = name.clone();
        let client = std::thread::spawn(move || {
            let mut file = open_pipe_client(&pipe_name).expect("client open pipe");
            let data = vec![0x41u8; 65536];
            file.write_all(&data).expect("client write large data");
        });

        let mut server_file =
            process_helper::connect_named_pipe_with_timeout(handle, 5000).expect("connect");

        let mut buf = Vec::new();
        server_file.read_to_end(&mut buf).expect("server read");

        assert_eq!(buf.len(), 65536);
        assert!(buf.iter().all(|&b| b == 0x41));

        client.join().expect("client join");
    }

    /// 客户端先连接时服务端仍能连上并读到数据
    #[test]
    fn test_named_pipe_client_first_connect() {
        let name = format!("test_pipe_early_{}", std::process::id());

        let handle = process_helper::create_named_pipe(&name).expect("create pipe");

        let pipe_name = name.clone();
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let client_ready = Arc::new(std::sync::Barrier::new(2));

        let b = Arc::clone(&barrier);
        let cr = Arc::clone(&client_ready);
        let client = std::thread::spawn(move || {
            b.wait();
            let mut file = open_pipe_client(&pipe_name).expect("client open pipe");
            cr.wait();
            file.write_all(b"early bird").expect("client write");
        });

        barrier.wait();
        std::thread::sleep(Duration::from_millis(100));

        let mut server_file = process_helper::connect_named_pipe_with_timeout(handle, 5000)
            .expect("connect after client");

        client_ready.wait();

        let mut buf = Vec::new();
        server_file.read_to_end(&mut buf).expect("server read");
        assert_eq!(buf, b"early bird");

        client.join().expect("client join");
    }

    /// E2E：直接调用 `launch_with_elevation_windows()`，验证提权管道完整链路
    ///
    /// 命名管道 → 提权 helper NamedPipeClientStream → .NET Process.Start → 输出抄送 → 管道读取；
    /// UAC 接受时验证 stdout 内容，UAC 拒绝时 stdout/stderr 为 None 但不 panic
    #[test]
    #[ignore = "需要人工确认 UAC，自动验证使用不提权的辅助进程测试"]
    fn test_launch_with_elevation_windows_e2e() {
        use std::collections::HashMap;

        // 提权 helper 按 CRT 规则给每个参数单独加引号，而 cmd.exe 的 /c 有自己一套
        // 去引号规则：`"/c" "echo" ...` 会被它当成程序名，输出全落到 stderr。
        // 生产上提权的是 java / 启动器本体，这里用 PowerShell 直接验证管道本身
        let result = process_helper::launch_with_elevation_windows(
            "powershell.exe",
            vec![
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-Command".into(),
                "[Console]::Out.WriteLine('hello_from_pipe')".into(),
            ],
            HashMap::new(),
            ".",
        );

        match result {
            Ok(r) => {
                assert!(r.is_admin, "elevation path: is_admin should be true");
                eprintln!(
                    "launch_with_elevation_windows: is_admin={} pid={:?} stdout={} stderr={}",
                    r.is_admin,
                    r.pid,
                    r.stdout.is_some(),
                    r.stderr.is_some(),
                );
                if let Some(mut stdout) = r.stdout {
                    let mut buf = String::new();
                    stdout.read_to_string(&mut buf).expect("read stdout");
                    eprintln!("stdout: '{}'", buf.trim());
                    assert!(
                        buf.contains("hello_from_pipe"),
                        "expected 'hello_from_pipe', got: '{}'",
                        buf.trim()
                    );
                } else {
                    eprintln!("stdout is None (UAC declined or timed out)");
                }
            }
            Err(e) => {
                eprintln!("launch_with_elevation_windows error: {:?}", e);
            }
        }
    }

    /// E2E：通过 `launch_with_elevation_windows` 启动 `java -version`
    ///
    /// Java 将版本信息输出到 **stderr** 而非 stdout，可验证 stderr 管道是否正常工作；
    /// 需要已安装 Java 且 UAC 可交互，或当前已是管理员
    #[test]
    #[ignore = "需要人工确认 UAC，自动验证使用不提权的辅助进程测试"]
    fn test_launch_java_version_via_elevation() {
        use std::collections::HashMap;
        use std::process::Command;

        // 检查 Java 是否可用
        let java_available = Command::new("java")
            .arg("-version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if !java_available {
            eprintln!("SKIP: java not found on PATH");
            return;
        }

        let result = process_helper::launch_with_elevation_windows(
            "java",
            vec!["-version".into()],
            HashMap::new(),
            ".",
        );

        match result {
            Ok(r) => {
                assert!(r.is_admin, "elevation path: is_admin should be true");
                eprintln!(
                    "java -version via elevation: is_admin={} pid={:?} stdout={} stderr={}",
                    r.is_admin,
                    r.pid,
                    r.stdout.is_some(),
                    r.stderr.is_some(),
                );

                // java -version 输出到 stderr
                if let Some(mut stderr) = r.stderr {
                    let mut buf = String::new();
                    stderr.read_to_string(&mut buf).expect("read stderr");
                    eprintln!("java stderr:\n{}", buf);
                    assert!(
                        buf.contains("version") || buf.contains("java"),
                        "stderr should contain Java version info, got: '{}'",
                        buf.trim()
                    );
                } else if let Some(mut stdout) = r.stdout {
                    // 某些 Java 实现可能输出到 stdout
                    let mut buf = String::new();
                    stdout.read_to_string(&mut buf).expect("read stdout");
                    eprintln!("java stdout:\n{}", buf);
                    assert!(
                        buf.contains("version") || buf.contains("java"),
                        "stdout should contain Java version info, got: '{}'",
                        buf.trim()
                    );
                } else {
                    eprintln!("java -version: no output captured (UAC declined or timed out)");
                }
            }
            Err(e) => {
                eprintln!("launch_with_elevation_windows(java) error: {:?}", e);
            }
        }
    }

    /// 验证 `Start-Process -RedirectStandardOutput` 对命名管道的已知限制：
    /// 管道能连接但**数据不会写入**，因为 `Start-Process` 用 `File.OpenWrite`
    /// 模式打开路径，不兼容命名管道；这正是改用提权 helper 的原因
    #[test]
    fn test_start_process_redirect_to_pipe_is_broken() {
        use std::io::Read;
        use std::process::{Command, Stdio};
        use std::sync::atomic::{AtomicU64, Ordering};

        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = format!(
            "sp_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let pipe_out = format!("mml_sp_out_{}", id);

        let h_out = process_helper::create_named_pipe(&pipe_out).expect("create pipe");
        let o_out = unsafe { OwnedHandle::from_raw_handle(h_out.0) };
        let (tx_out, rx_out) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            tx_out
                .send(process_helper::connect_named_pipe_with_timeout(
                    HANDLE(o_out.into_raw_handle()),
                    5_000,
                ))
                .ok();
        });
        std::thread::sleep(Duration::from_millis(100));

        let ps_cmd = format!(
            "$p = Start-Process -FilePath 'cmd' \
             -ArgumentList '/c echo should_not_appear' \
             -PassThru -NoNewWindow \
             -RedirectStandardOutput '\\\\.\\pipe\\{pipe_out}'; \
             Write-Output $p.Id; Wait-Process -Id $p.Id"
        );

        let mut launcher = Command::new("powershell")
            .arg("-Command")
            .arg(&ps_cmd)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn powershell");

        // 读 PID 证明 PowerShell 正常执行
        let mut r = std::io::BufReader::new(launcher.stdout.take().unwrap());
        let mut line = String::new();
        r.read_line(&mut line).ok();
        eprintln!("PID: {:?}", line.trim().parse::<u32>().ok());

        // 管道应连接成功，但数据为空
        if let Ok(Ok(mut f)) = rx_out.recv_timeout(Duration::from_secs(10)) {
            let mut buf = String::new();
            f.read_to_string(&mut buf).unwrap();
            eprintln!("pipe connected, got {} bytes (expected: empty)", buf.len());
            // 已知限制：数据为空
            assert!(
                buf.trim().is_empty(),
                "KNOWN LIMITATION: Start-Process -RedirectStandardOutput should NOT work with named pipes. \
                 If this assertion fails, Microsoft may have fixed it — switch back to pipes!"
            );
        } else {
            eprintln!("pipe connect failed or timed out");
        }
        let _ = launcher.wait();
    }

    /// 验证：启动真实子进程，命名管道保证收到 stdout/stderr 数据
    ///
    /// 通过 .NET `Process.Start` + `NamedPipeClientStream` 这套与提权 helper 完全相同的组合，
    /// 验证管道能收到真实进程的输出
    #[test]
    fn test_real_child_process_data_through_pipe() {
        use std::process::{Command, Stdio};
        use std::sync::atomic::{AtomicU64, Ordering};

        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = format!(
            "child_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let pipe_out = format!("mml_child_out_{}", id);
        let pipe_err = format!("mml_child_err_{}", id);

        // 1. 创建管道 + 连接线程，必须在 PowerShell 之前
        let h_out = process_helper::create_named_pipe(&pipe_out).expect("create pipe out");
        let h_err = process_helper::create_named_pipe(&pipe_err).expect("create pipe err");
        let o_out = unsafe { OwnedHandle::from_raw_handle(h_out.0) };
        let o_err = unsafe { OwnedHandle::from_raw_handle(h_err.0) };

        let (tx_out, rx_out) = std::sync::mpsc::channel();
        let (tx_err, rx_err) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            tx_out
                .send(process_helper::connect_named_pipe_with_timeout(
                    HANDLE(o_out.into_raw_handle()),
                    10_000,
                ))
                .ok();
        });
        std::thread::spawn(move || {
            tx_err
                .send(process_helper::connect_named_pipe_with_timeout(
                    HANDLE(o_err.into_raw_handle()),
                    10_000,
                ))
                .ok();
        });
        std::thread::sleep(Duration::from_millis(50));

        // 2. PowerShell：启动真实 cmd 子进程 → 捕获 stdout/stderr → 写入命名管道
        //    Process.Start → StandardOutput.ReadToEnd() → NamedPipeClientStream (CreateFileW)
        let ps = format!(
            "$psi=New-Object System.Diagnostics.ProcessStartInfo('cmd.exe','/c echo hello_stdout&echo hello_stderr >&2'); \
             $psi.UseShellExecute=$false;$psi.RedirectStandardOutput=$true;$psi.RedirectStandardError=$true; \
             $p=[System.Diagnostics.Process]::Start($psi);$out=$p.StandardOutput.ReadToEnd();$err=$p.StandardError.ReadToEnd();$p.WaitForExit(); \
             function wp {{ param($n,$t) $x=New-Object System.IO.Pipes.NamedPipeClientStream('.',$n,[System.IO.Pipes.PipeDirection]::Out);$x.Connect(8000);$w=New-Object System.IO.StreamWriter($x);$w.Write($t);$w.Flush();$w.Close() }}; \
             wp '{pipe_out}' $out; \
             wp '{pipe_err}' $err"
        );

        let mut launcher = Command::new("powershell")
            .arg("-Command")
            .arg(&ps)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn powershell");

        // 3. 收管道数据
        let mut stdout_file = rx_out
            .recv_timeout(Duration::from_secs(15))
            .expect("out pipe connect timeout")
            .expect("out pipe connect error");
        let mut stderr_file = rx_err
            .recv_timeout(Duration::from_secs(15))
            .expect("err pipe connect timeout")
            .expect("err pipe connect error");

        let mut out_buf = String::new();
        let mut err_buf = String::new();
        stdout_file.read_to_string(&mut out_buf).expect("read out");
        stderr_file.read_to_string(&mut err_buf).expect("read err");

        launcher.wait().expect("powershell wait");

        // 4. 断言：管道理应收到真实子进程的输出
        assert!(
            out_buf.contains("hello_stdout"),
            "stdout pipe must contain 'hello_stdout', got: '{}'",
            out_buf.trim()
        );
        assert!(
            err_buf.contains("hello_stderr"),
            "stderr pipe must contain 'hello_stderr', got: '{}'",
            err_buf.trim()
        );

        eprintln!(
            "Child process pipe test: stdout='{}' stderr='{}'",
            out_buf.trim(),
            err_buf.trim()
        );
    }
}
