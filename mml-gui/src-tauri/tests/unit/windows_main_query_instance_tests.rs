use super::*;

/// 只数启用中的 jar：大写扩展名算，`.jar.disabled`、目录与非 jar 文件不算
#[test]
fn counts_enabled_jars_only() {
    let dir = crate::test_support::temp_dir("launch-memory-mods");
    fs::write(dir.join("a.jar"), b"").unwrap();
    fs::write(dir.join("b.JAR"), b"").unwrap();
    fs::write(dir.join("c.jar.disabled"), b"").unwrap();
    fs::write(dir.join("d.txt"), b"").unwrap();
    fs::create_dir(dir.join("e.jar")).unwrap();
    assert_eq!(count_enabled_mods(&dir), 2);
    crate::test_support::cleanup(&[&dir]);
}

/// 阈值沿用 ColorMC：4 GB 档超过 150、8 GB 档超过 300
#[test]
fn memory_low_thresholds() {
    assert!(!memory_low(150, 4096));
    assert!(memory_low(151, 4096));
    assert!(!memory_low(300, 8192));
    assert!(memory_low(301, 8192));
    // 超过 8 GB 不再按这两档提醒
    assert!(!memory_low(301, 8193));
}

