use super::*;

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

