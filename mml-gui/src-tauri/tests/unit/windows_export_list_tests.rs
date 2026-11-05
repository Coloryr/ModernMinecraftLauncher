use super::*;

#[test]
fn local_entries_get_stable_custom_ids() {
    let dir = crate::test_support::temp_dir("export-custom-ids");
    std::fs::create_dir_all(dir.join("mods")).unwrap();
    std::fs::write(dir.join("mods/local.jar"), b"jar").unwrap();

    let entries = list_entries(
        &dir.join("mods"),
        &dir,
        &std::collections::HashMap::new(),
        false,
        true,
    )
    .unwrap();
    let entry = &entries[0];
    assert!(entry.custom_id);
    assert!(!entry.sha1.is_empty(), "本地文件也要算哈希");
    let (expected, _) = custom_ids(&entry.path);
    assert_eq!(entry.custom_project_id.as_deref(), Some(expected.as_str()));
    assert!(
        entry
            .custom_project_id
            .as_deref()
            .is_some_and(|id| id.starts_with("custom-p-"))
    );

    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn directory_size_sums_nested_files() {
    let dir = crate::test_support::temp_dir("export-dir-size");
    std::fs::create_dir_all(dir.join("config/nested")).unwrap();
    std::fs::write(dir.join("config/a.txt"), b"abc").unwrap();
    std::fs::write(dir.join("config/nested/b.txt"), b"defgh").unwrap();
    assert_eq!(dir_size(&dir.join("config")), 8);
    assert_eq!(dir_size(&dir.join("missing")), 0);
    crate::test_support::cleanup(&[&dir]);
}

