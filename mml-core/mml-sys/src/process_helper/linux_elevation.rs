//! pkexec 会清理环境并切换目录，目标进程的用户环境需在提权后恢复。

use std::{collections::HashMap, path::Path, process::Command};

pub(super) fn command(
    path: &Path,
    args: &[String],
    mut env: HashMap<String, String>,
    working_dir: &Path,
) -> Command {
    for key in [
        "HOME",
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XAUTHORITY",
        "XDG_RUNTIME_DIR",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "DBUS_SESSION_BUS_ADDRESS",
        "LANG",
        "LC_ALL",
    ] {
        if let Ok(value) = std::env::var(key) {
            env.entry(key.into()).or_insert(value);
        }
    }
    let mut command = Command::new("pkexec");
    command
        .arg("/usr/bin/env")
        .arg(format!("--chdir={}", working_dir.display()));
    for (key, value) in env {
        command.arg(format!("{key}={value}"));
    }
    command.arg(path).args(args);
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restores_user_home_after_pkexec_without_shell_interpolation() {
        let command = command(
            Path::new("/opt/my launcher"),
            &["a ' b".into()],
            HashMap::from([("HOME".into(), "/home/player".into())]),
            Path::new("/home/player/mml"),
        );
        let args: Vec<_> = command
            .get_args()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args[0], "/usr/bin/env");
        assert!(args.contains(&"--chdir=/home/player/mml".into()));
        assert!(args.contains(&"HOME=/home/player".into()));
        assert_eq!(&args[args.len() - 2..], ["/opt/my launcher", "a ' b"]);
    }
}
