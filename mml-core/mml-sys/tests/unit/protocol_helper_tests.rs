use super::exe_of_command;
use std::path::PathBuf;

/// 带引号的命令行：取第一对引号里的路径，忽略后面的参数
#[test]
fn parses_quoted_command() {
    assert_eq!(
        exe_of_command(r#""C:\Program Files\ColorMC\ColorMC.Launcher.exe" "%1""#),
        Some(PathBuf::from(
            r"C:\Program Files\ColorMC\ColorMC.Launcher.exe"
        ))
    );
}

/// 不带引号：取第一个空白前的部分
#[test]
fn parses_unquoted_command() {
    assert_eq!(
        exe_of_command(r"D:\ColorMC\ColorMC.Launcher.exe %1"),
        Some(PathBuf::from(r"D:\ColorMC\ColorMC.Launcher.exe"))
    );
}

/// 空命令行不该造出一个空路径
#[test]
fn empty_command_has_no_exe() {
    assert_eq!(exe_of_command("  "), None);
}

