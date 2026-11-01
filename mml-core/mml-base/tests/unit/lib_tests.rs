use super::*;

/// init / get_base_dir 全局根目录
///
/// BASE_DIR 是进程级全局单例，首次调用的参数生效，重复 init 不 panic 也不覆盖
#[test]
fn test_init_and_get_base_dir() {
    let dir = mml_testutil::temp_dir().join(format!(
        "mml_base_init_test_{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    init(&dir);
    assert_eq!(get_base_dir(), dir);

    // 重复调用不应 panic，且保留第一次的值
    init("/mml_base_should_not_override");
    assert_eq!(get_base_dir(), dir);

    let _ = std::fs::remove_dir_all(&dir);
}

