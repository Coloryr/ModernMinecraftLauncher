//! 启动前 / 启动后命令的解析：拆行、去引号、找程序、按后缀挑解释器
use std::path::{Path, PathBuf};

use uuid::Uuid;

use super::*;

/// 一行式命令要拆出程序名与参数，引号只用来分组、不进参数
#[test]
fn split_command_line_splits_program_and_quoted_args() {
    let argv = split_command_line(r#"python.exe "E:\minecraft\mml\update\updater.py" -v"#);
    assert_eq!(
        argv,
        vec![
            "python.exe".to_string(),
            r"E:\minecraft\mml\update\updater.py".to_string(),
            "-v".to_string()
        ]
    );
    assert!(split_command_line("   ").is_empty());
}

/// 参数两端成对的引号要去掉，没写引号的保持原样
#[test]
fn unquote_strips_one_quote_pair() {
    assert_eq!(unquote(r#""C:\a b\x.py""#), r"C:\a b\x.py");
    assert_eq!(unquote("plain.exe"), "plain.exe");
    assert_eq!(unquote(r#""half"#), r#""half"#);
}

/// 只写名字的程序先在实例根目录找，找不到才算路径不存在
#[test]
fn find_program_searches_instance_root() {
    let dir = mml_testutil::temp_path(format!("mml-cmd-shell-{}", Uuid::new_v4().simple()));
    let base = dir.join("base");
    let game = dir.join("game");
    std::fs::create_dir_all(&base).unwrap();
    std::fs::create_dir_all(&game).unwrap();

    let tool = base.join(if cfg!(windows) {
        "mml-test-tool.exe"
    } else {
        "mml-test-tool"
    });
    std::fs::write(&tool, b"").unwrap();

    let name = tool.file_name().unwrap().to_string_lossy().to_string();
    assert_eq!(find_program(&name, &base, &game), Some(tool));
    assert!(find_program("mml-test-tool-missing", &base, &game).is_none());

    let _ = std::fs::remove_dir_all(&dir);
}

/// 首行拆成程序与参数，之后每行算一个参数
#[test]
fn resolve_keeps_one_argument_per_line() {
    let dir = mml_testutil::temp_path(format!("mml-cmd-shell-{}", Uuid::new_v4().simple()));
    let base = dir.join("base");
    let game = dir.join("game");
    std::fs::create_dir_all(&base).unwrap();
    std::fs::create_dir_all(&game).unwrap();

    let tool = base.join(if cfg!(windows) {
        "mml-shell-tool.exe"
    } else {
        "mml-shell-tool"
    });
    std::fs::write(&tool, b"").unwrap();

    let name = tool.file_name().unwrap().to_string_lossy().to_string();
    let cmd = format!("{name} \"first arg\"\nsecond arg");
    let command = resolve(&cmd, &base, &game).expect("应解析出命令");

    assert_eq!(command.program, tool);
    assert_eq!(
        command.args,
        vec!["first arg".to_string(), "second arg".to_string()]
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// Windows 下 .bat 走 cmd /c call，路径带空格也不会被截断
#[cfg(windows)]
#[test]
fn bat_runs_through_cmd_call() {
    let path = PathBuf::from(r"D:\tmp\my tool.bat");
    let (runner, prefix) = script_runner(&path)
        .expect("Windows 上应能找 command")
        .expect("bat 是脚本");

    assert_eq!(
        runner.file_name().unwrap().to_string_lossy().to_ascii_lowercase(),
        "cmd.exe"
    );
    assert_eq!(prefix, vec!["/c".to_string(), "call".to_string()]);
}

/// 非 Windows 上 .bat 跑不了，要给明确错误而不是找不到程序
#[cfg(not(windows))]
#[test]
fn bat_is_unsupported_on_unix() {
    let path = PathBuf::from("/tmp/my tool.bat");
    match script_runner(&path) {
        Err(ErrorType::ScriptUnsupported(data)) => assert_eq!(data.path, path),
        other => panic!("应报 ScriptUnsupported，实际：{other:?}"),
    }
}
/// 命令里出现 python 或 .py 脚本就要 Python
#[test]
fn needs_python_detects_python_commands() {
    let base = Path::new("base");
    let game = Path::new("game");

    assert!(needs_python("python.exe updater.py", base, game));
    assert!(needs_python("\"update/updater.py\"", base, game));
    assert!(!needs_python("java -jar server.jar", base, game));
}

/// Windows 上把路径里的 `/` 换成 `\`，词首的开关保持原样
#[cfg(windows)]
#[test]
fn separators_follow_windows() {
    assert_eq!(
        native_separators("cmd.exe /c python.exe C:/x/y.py"),
        "cmd.exe /c python.exe C:\\x\\y.py"
    );
    assert_eq!(native_separators("\"E:/a/b.py\""), "\"E:\\a\\b.py\"");
}

/// 其它平台把 `\` 换成 `/`
#[cfg(not(windows))]
#[test]
fn separators_follow_unix() {
    assert_eq!(
        native_separators("python3 update\\updater.py"),
        "python3 update/updater.py"
    );
}

