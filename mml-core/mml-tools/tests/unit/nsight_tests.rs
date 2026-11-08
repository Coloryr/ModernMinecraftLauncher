//! Nsight Graphics 安装检测与启动参数
use std::collections::HashMap;

use super::*;

/// 版本目录多套一层时，仍能在里面找到 ngfx-capture
#[test]
fn find_in_checks_versioned_install() {
    let dir = mml_testutil::temp_path(format!("mml-tools-nsight-{}", std::process::id()));
    let cli_name = if cfg!(windows) {
        "ngfx-capture.exe"
    } else {
        "ngfx-capture"
    };
    let cli = dir.join("Nsight Graphics 2024.3").join("host").join(cli_name);
    std::fs::create_dir_all(cli.parent().unwrap()).unwrap();
    std::fs::write(&cli, b"").unwrap();

    assert_eq!(find_in(&dir).map(|item| item.cli), Some(cli));
    assert!(find_in(&dir.join("empty")).is_none());

    let _ = std::fs::remove_dir_all(&dir);
}

/// --env 在 --args 之前，--args 放最后：两者都是可变长，顺序反了会把选项当成游戏参数
#[test]
fn capture_args_put_env_before_args() {
    let java = Path::new("java.exe");
    let game = Path::new("game");
    let captures = Path::new("captures");
    let args = vec!["-Xmx2G".to_string(), "-cp".to_string(), "a.jar".to_string()];
    let mut env = HashMap::new();
    env.insert("MML_LAUNCHER".to_string(), "1".to_string());

    let out = capture_args(java, &args, game, captures, &env);

    let env_at = out.iter().position(|item| item == "--env").unwrap();
    let args_at = out.iter().position(|item| item == "--args").unwrap();
    assert!(env_at < args_at);
    assert_eq!(out[env_at + 1], "MML_LAUNCHER=1");
    assert_eq!(&out[args_at + 1..], args.as_slice());
}
