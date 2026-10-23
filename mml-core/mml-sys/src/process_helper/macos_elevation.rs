//! AppleScript 与 POSIX shell 各自转义，提权后的目录和环境显式传递。

use std::{collections::HashMap, path::Path, process::Command};

/// 保留 shell 参数的字面值，包括空串、单引号和命令替换符。
fn shell_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn shell_command(
    path: &Path,
    args: &[String],
    mut env: HashMap<String, String>,
    working_dir: &Path,
) -> String {
    for key in ["HOME", "LANG", "LC_ALL"] {
        if let Ok(value) = std::env::var(key) {
            env.entry(key.into()).or_insert(value);
        }
    }
    let mut command = format!(
        "cd {} && exec /usr/bin/env",
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
    command.push_str(" 2>&1");
    command
}

/// 构造提权命令，shell 脚本通过 AppleScript 参数传入，避免再嵌套一层字符串。
/// - `path`: 目标程序路径。
/// - `args`: 原始参数。
/// - `env`: 目标程序附加环境。
/// - `working_dir`: 提权后使用的工作目录。
/// # 返回值
/// 尚未执行的 osascript 命令。
pub(super) fn command(
    path: &Path,
    args: &[String],
    env: HashMap<String, String>,
    working_dir: &Path,
) -> Command {
    let mut command = Command::new("/usr/bin/osascript");
    command.args(["-e", "on run argv", "-e", "do shell script (item 1 of argv) with administrator privileges without altering line endings", "-e", "end run", "--"]);
    command.arg(shell_command(path, args, env, working_dir));
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
}
