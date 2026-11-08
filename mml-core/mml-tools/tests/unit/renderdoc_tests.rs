//! RenderDoc 目录布局与启动参数
use super::*;

/// 便携包多套一层版本目录，仍能找到 renderdoccmd
#[test]
fn find_in_checks_nested_portable_layout() {
    let dir = mml_testutil::temp_path(format!("mml-tools-renderdoc-{}", std::process::id()));
    let name = if cfg!(windows) {
        "renderdoccmd.exe"
    } else {
        "renderdoccmd"
    };
    let cmd = dir.join("RenderDoc_1.47_64").join(name);
    std::fs::create_dir_all(cmd.parent().unwrap()).unwrap();
    std::fs::write(&cmd, b"").unwrap();

    assert_eq!(find_in(&dir), Some(cmd));
    assert_eq!(find_in(&dir.join("empty")), None);

    let _ = std::fs::remove_dir_all(&dir);
}

/// capture 参数把工作目录、捕获前缀与原 Java 参数按顺序拼好
#[test]
fn capture_args_keep_java_arguments_after_executable() {
    let java = Path::new("java");
    let game = Path::new("game");
    let captures = Path::new("captures");
    let args = vec!["-Xmx2G".to_string(), "-cp".to_string(), "a.jar".to_string()];

    let out = capture_args(java, &args, game, captures, "instance_123");

    assert_eq!(&out[..5], &["capture", "-w", "-d", "game", "-c"]);
    assert_eq!(
        out[5],
        captures
            .join("instance_123_")
            .to_string_lossy()
            .into_owned()
    );
    assert_eq!(out[6], "java");
    assert_eq!(&out[7..], args.as_slice());
}
