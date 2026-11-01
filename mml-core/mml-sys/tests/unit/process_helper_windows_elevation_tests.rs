use super::*;
use std::io::Read;

#[test]
fn stop_channel_terminates_target_before_wrapper_exits() {
    let script = "[Console]::Out.WriteLine('ready'); Start-Sleep -Seconds 2; [Console]::Out.WriteLine('target-still-running')";
    let mut result = launch_helper(
        Path::new("powershell.exe"),
        vec![
            "-NoProfile".into(),
            "-NonInteractive".into(),
            "-EncodedCommand".into(),
            encoded(script),
        ],
        HashMap::new(),
        &std::env::current_dir().unwrap(),
        false,
    )
    .unwrap();
    let mut reader = BufReader::new(result.stdout.take().unwrap());
    let mut ready = String::new();
    reader.read_line(&mut ready).unwrap();
    assert_eq!(ready.trim(), "ready");
    result.control.as_ref().unwrap().request_stop().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = result.child.try_wait().unwrap() {
            assert!(!status.success());
            break;
        }
        assert!(
            Instant::now() < deadline,
            "停止请求必须在目标进程退出后完成"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut remaining = String::new();
    reader.read_to_string(&mut remaining).unwrap();
    assert!(!remaining.contains("target-still-running"));
}

#[test]
fn helper_preserves_environment_and_drains_both_streams() {
    let mut environment = HashMap::new();
    environment.insert("MML_PIPE_TEST".into(), "space ' quote 中文".into());
    let script = "[Console]::Out.WriteLine([Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($env:MML_PIPE_TEST))); [Console]::Error.Write(('E'*131072)); [Console]::Out.Write(('O'*131072)); exit 7";
    let mut result = launch_helper(
        Path::new("powershell.exe"),
        vec![
            "-NoProfile".into(),
            "-NonInteractive".into(),
            "-EncodedCommand".into(),
            encoded(script),
        ],
        environment,
        &std::env::current_dir().unwrap(),
        false,
    )
    .unwrap();
    let mut stdout = result.stdout.take().unwrap();
    let mut stderr = result.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let err = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).unwrap();
        bytes
    });
    assert_eq!(result.child.wait().unwrap().code(), Some(7));
    let out = out.join().unwrap();
    assert!(String::from_utf8_lossy(&out).contains(&STANDARD.encode("space ' quote 中文")));
    assert!(out.ends_with(&vec![b'O'; 131072]));
    assert_eq!(err.join().unwrap(), vec![b'E'; 131072]);
}

#[test]
fn helper_preserves_empty_quotes_and_trailing_backslashes() {
    let file = HelperScript(mml_testutil::temp_path(format!(
        "mml args ' 中文 {}.ps1",
        uuid::Uuid::new_v4()
    )));
    std::fs::write(&file.0, "param([string]$a,[string]$b,[string]$c)\nforeach($v in @($a,$b,$c)){[Console]::Out.WriteLine([Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($v)))}").unwrap();
    let expected = ["", "sp ace ' \" 中文", "ends\\"];
    let mut args = vec![
        "-NoProfile".into(),
        "-NonInteractive".into(),
        "-ExecutionPolicy".into(),
        "Bypass".into(),
        "-File".into(),
        file.0.to_string_lossy().into_owned(),
    ];
    args.extend(expected.iter().map(|s| s.to_string()));
    let mut result = launch_helper(
        Path::new("powershell.exe"),
        args,
        HashMap::new(),
        &std::env::current_dir().unwrap(),
        false,
    )
    .unwrap();
    let mut out = String::new();
    result
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut out)
        .unwrap();
    assert!(result.child.wait().unwrap().success());
    assert_eq!(
        out.lines().collect::<Vec<_>>(),
        expected.map(|s| STANDARD.encode(s))
    );
}

