use super::*;

/// 临时目录必须落在仓库的 `target/temp` 下，这是本 crate 存在的唯一理由
#[test]
fn temp_dir_is_under_target() {
    let dir = temp_dir();
    assert!(
        dir.ends_with(Path::new("target").join("temp")),
        "实际是 {dir:?}"
    );
    // 不含 `..`：否则写进日志 / 做前缀比较都会很难看
    assert!(
        !dir.to_string_lossy().contains(".."),
        "路径不该含 `..`：{dir:?}"
    );
    // 仓库根必须真的存在，parent() 走错层级时这条会挂
    assert!(
        dir.parent().is_some_and(|p| p.is_dir()),
        "target 目录应存在"
    );
}

/// temp_path 拼接在 temp_dir 之下
#[test]
fn temp_path_joins_under_temp_dir() {
    let p = temp_path("mml-some-case");
    assert_eq!(p.parent(), Some(temp_dir().as_path()));
}

