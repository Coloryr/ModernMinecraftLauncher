use super::*;
use crate::test_support;

#[test]
fn writes_hidden_lock_and_verifies() {
    let dir = test_support::temp_dir("client-lock");
    let dirs = vec![dir.clone()];
    assert!(!locked_in(&dirs));
    // 没有锁时任何密码都放行，命令层也按未锁定处理
    assert!(verify_in(&dirs, "whatever"));
    set_in(&dirs, "pw").unwrap();
    assert!(locked_in(&dirs));
    // 单串里带算法与轮数，以后调参数也能验旧密码
    let raw = fs::read_to_string(lock_path(&dir)).unwrap();
    assert!(raw.contains("$pbkdf2-sha256$120000$"), "{raw}");
    assert!(verify_in(&dirs, "pw"));
    assert!(!verify_in(&dirs, "bad"));
    clear_in(&dirs).unwrap();
    assert!(!locked_in(&dirs));
    test_support::cleanup(&[&dir]);
}

#[test]
fn verifies_with_rounds_from_the_string() {
    let dir = test_support::temp_dir("client-lock-rounds");
    let dirs = vec![dir.clone()];
    let password = format!(
        "$pbkdf2-sha256$1000$aabb${}",
        hash_password("pw", "aabb", 1000)
    );
    fs::write(
        lock_path(&dir),
        serde_json::to_vec(&StoredLock { password }).unwrap(),
    )
    .unwrap();
    assert!(verify_in(&dirs, "pw"));
    assert!(!verify_in(&dirs, "bad"));
    assert!(parse_ok(&fs::read(lock_path(&dir)).unwrap()));
    test_support::cleanup(&[&dir]);
}

#[test]
fn reads_legacy_two_field_lock() {
    let dir = test_support::temp_dir("client-lock-legacy");
    let dirs = vec![dir.clone()];
    let legacy = format!(
        r#"{{"Salt":"aa","Hash":"{}"}}"#,
        hash_password("pw", "aa", ROUNDS)
    );
    fs::write(lock_path(&dir), legacy).unwrap();
    assert!(verify_in(&dirs, "pw"));
    assert!(!verify_in(&dirs, "bad"));
    test_support::cleanup(&[&dir]);
}

#[test]
fn mirrors_between_both_locations() {
    let run = test_support::temp_dir("client-lock-run");
    let data = test_support::temp_dir("client-lock-data");
    let dirs = vec![run.clone(), data.clone()];
    set_in(&dirs, "pw").unwrap();
    assert!(lock_path(&run).is_file() && lock_path(&data).is_file());
    // 两份不一致时以运行目录为准
    fs::write(lock_path(&data), b"stale").unwrap();
    assert!(locked_in(&dirs));
    assert_eq!(
        fs::read(lock_path(&data)).unwrap(),
        fs::read(lock_path(&run)).unwrap()
    );
    // 删掉内部数据目录那份：下次访问用运行目录补回来
    fs::remove_file(lock_path(&data)).unwrap();
    assert!(locked_in(&dirs));
    assert!(lock_path(&data).is_file());
    // 只剩一份也能反向补，两边内容一致
    fs::remove_file(lock_path(&run)).unwrap();
    assert!(verify_in(&dirs, "pw"));
    assert!(lock_path(&run).is_file());
    // 解锁两份一起删
    clear_in(&dirs).unwrap();
    assert!(!lock_path(&run).exists() && !lock_path(&data).exists());
    test_support::cleanup(&[&run, &data]);
}

#[cfg(windows)]
#[test]
fn windows_marks_lock_hidden() {
    use std::os::windows::fs::MetadataExt;
    let dir = test_support::temp_dir("client-lock-hidden");
    set_in(&[dir.clone()], "pw").unwrap();
    let attrs = fs::metadata(lock_path(&dir)).unwrap().file_attributes();
    assert_ne!(
        attrs & 0x2,
        0,
        "lock file should carry FILE_ATTRIBUTE_HIDDEN"
    );
    test_support::cleanup(&[&dir]);
}

#[test]
fn corrupt_lock_stays_locked_but_never_verifies() {
    let dir = test_support::temp_dir("client-lock-corrupt");
    let dirs = vec![dir.clone()];
    fs::write(lock_path(&dir), b"not json").unwrap();
    assert!(locked_in(&dirs));
    assert!(!verify_in(&dirs, "pw"));
    clear_in(&dirs).unwrap();
    test_support::cleanup(&[&dir]);
}

