use super::*;
use crate::test_support;

#[test]
fn writes_hidden_lock_and_verifies() {
    let dir = test_support::temp_dir("client-lock");
    let paths = vec![dir.join(FILE_NAME)];
    assert!(!locked_in(&paths));
    // 没有锁时任何密码都放行，命令层也按未锁定处理
    assert!(verify_in(&paths, "whatever"));
    set_in(&paths, "pw").unwrap();
    assert!(locked_in(&paths));
    // 单串里带算法与轮数，以后调参数也能验旧密码
    let raw = fs::read_to_string(&paths[0]).unwrap();
    assert!(
        raw.contains(r#""Password": "$pbkdf2-sha256$120000$"#),
        "{raw}"
    );
    assert!(verify_in(&paths, "pw"));
    assert!(!verify_in(&paths, "bad"));
    clear_in(&paths).unwrap();
    assert!(!locked_in(&paths));
    test_support::cleanup(&[&dir]);
}

#[test]
fn verifies_with_rounds_from_the_string() {
    let dir = test_support::temp_dir("client-lock-rounds");
    let paths = vec![dir.join(FILE_NAME)];
    let password = format!(
        "$pbkdf2-sha256$1000$aabb${}",
        hash_password("pw", "aabb", 1000)
    );
    fs::write(
        &paths[0],
        serde_json::to_vec(&StoredLock { password }).unwrap(),
    )
    .unwrap();
    assert!(verify_in(&paths, "pw"));
    assert!(!verify_in(&paths, "bad"));
    assert!(parse_ok(&fs::read(&paths[0]).unwrap()));
    test_support::cleanup(&[&dir]);
}

#[test]
fn reads_legacy_two_field_lock() {
    let dir = test_support::temp_dir("client-lock-legacy");
    let paths = vec![dir.join(FILE_NAME)];
    let legacy = format!(
        r#"{{"Salt":"aa","Hash":"{}"}}"#,
        hash_password("pw", "aa", ROUNDS)
    );
    fs::write(&paths[0], legacy).unwrap();
    assert!(verify_in(&paths, "pw"));
    assert!(!verify_in(&paths, "bad"));
    test_support::cleanup(&[&dir]);
}

#[test]
fn mirrors_between_both_locations() {
    let run = test_support::temp_dir("client-lock-run");
    let data = test_support::temp_dir("client-lock-data");
    let paths = vec![run.join(FILE_NAME), data.join(FILE_NAME)];
    set_in(&paths, "pw").unwrap();
    assert!(paths[0].is_file() && paths[1].is_file());
    // 两份不一致时以运行目录为准
    fs::write(&paths[1], b"stale").unwrap();
    assert!(locked_in(&paths));
    assert_eq!(fs::read(&paths[1]).unwrap(), fs::read(&paths[0]).unwrap());
    // 删掉内部数据目录那份：下次访问用运行目录补回来
    fs::remove_file(&paths[1]).unwrap();
    assert!(locked_in(&paths));
    assert!(paths[1].is_file());
    // 只剩一份也能反向补，两边内容一致
    fs::remove_file(&paths[0]).unwrap();
    assert!(verify_in(&paths, "pw"));
    assert!(paths[0].is_file());
    // 解锁两份一起删
    clear_in(&paths).unwrap();
    assert!(!paths[0].exists() && !paths[1].exists());
    test_support::cleanup(&[&run, &data]);
}

#[test]
fn mirror_name_follows_run_dir() {
    let a = test_support::temp_dir("client-lock-name-a");
    let b = test_support::temp_dir("client-lock-name-b");
    // 同一个运行目录算出同一个名字，换一个安装就是另一个名字
    assert_eq!(mirror_name(&a), mirror_name(&a));
    assert_ne!(mirror_name(&a), mirror_name(&b));
    let name = mirror_name(&a);
    assert!(
        name.starts_with(".client_lock.") && name.ends_with(".json"),
        "{name}"
    );
    test_support::cleanup(&[&a, &b]);
}

#[test]
fn lock_stays_per_install() {
    let a = test_support::temp_dir("client-lock-install-a");
    let b = test_support::temp_dir("client-lock-install-b");
    // 内部数据目录那份按运行目录摘要区分，两个安装的落点不重叠
    let a_paths = vec![a.join(FILE_NAME), a.join(mirror_name(&a))];
    let b_paths = vec![b.join(FILE_NAME), b.join(mirror_name(&b))];
    set_in(&a_paths, "pw").unwrap();
    assert!(locked_in(&a_paths));
    // 一个安装上锁不能把另一个也锁上
    assert!(!locked_in(&b_paths));
    assert!(b_paths.iter().all(|path| !path.exists()));
    set_in(&b_paths, "other").unwrap();
    // 各解各的锁：解开一个，另一个照旧锁着
    clear_in(&a_paths).unwrap();
    assert!(locked_in(&b_paths));
    assert!(verify_in(&b_paths, "other"));
    assert!(!verify_in(&b_paths, "pw"));
    test_support::cleanup(&[&a, &b]);
}

#[cfg(windows)]
#[test]
fn windows_marks_lock_hidden() {
    use std::os::windows::fs::MetadataExt;
    let dir = test_support::temp_dir("client-lock-hidden");
    let paths = vec![dir.join(FILE_NAME)];
    set_in(&paths, "pw").unwrap();
    let attrs = fs::metadata(&paths[0]).unwrap().file_attributes();
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
    let paths = vec![dir.join(FILE_NAME)];
    fs::write(&paths[0], b"not json").unwrap();
    assert!(locked_in(&paths));
    assert!(!verify_in(&paths, "pw"));
    clear_in(&paths).unwrap();
    test_support::cleanup(&[&dir]);
}

/// 运行目录就是内部数据目录时只能有一个落点，别再算出一份带摘要的
#[test]
fn same_directory_keeps_single_lock_file() {
    let dir = test_support::temp_dir("client-lock-same-dir");
    let data = test_support::temp_dir("client-lock-same-data");
    fs::create_dir_all(&dir).unwrap();
    fs::create_dir_all(&data).unwrap();

    // 同一个目录：只留不带摘要的那份
    assert_eq!(targets_in(&dir, &dir), vec![dir.join(FILE_NAME)]);
    // 两个不同目录：多一份带运行目录摘要的
    let paths = targets_in(&dir, &data);
    assert_eq!(paths.len(), 2);
    assert_eq!(paths[0], dir.join(FILE_NAME));
    assert_eq!(paths[1], data.join(mirror_name(&dir)));
    test_support::cleanup(&[&dir, &data]);
}
