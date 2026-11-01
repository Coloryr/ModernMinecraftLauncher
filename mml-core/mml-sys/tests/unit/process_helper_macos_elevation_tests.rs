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

