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
#[path = "../../tests/unit/process_helper_linux_elevation_tests.rs"]
mod tests;
