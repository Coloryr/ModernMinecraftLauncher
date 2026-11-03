//! Python 运行环境的目录布局
use super::*;

/// 解压出来的解释器按几个固定位置找
#[test]
fn find_in_checks_known_layouts() {
    let dir = mml_testutil::temp_path(format!("mml-tools-python-{}", std::process::id()));
    let rel = if cfg!(windows) { "python.exe" } else { "bin/python3" };
    let python = dir.join(rel);
    std::fs::create_dir_all(python.parent().unwrap()).unwrap();
    std::fs::write(&python, b"").unwrap();

    assert_eq!(find_in(&dir), Some(python));
    assert_eq!(find_in(&dir.join("empty")), None);

    let _ = std::fs::remove_dir_all(&dir);
}
