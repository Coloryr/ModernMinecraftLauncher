#[test]
fn dropped_zip_and_install_flag_use_same_request() {
    let dir = crate::test_support::temp_dir("dropped-server-pack");
    let zip = dir.join("server pack.zip");
    std::fs::write(&zip, b"zip").unwrap();
    let path = zip.to_string_lossy().to_string();
    let dropped = super::parse(&[path.clone()]).unwrap();
    let flagged = super::parse(&["--install".into(), path.clone()]).unwrap();
    assert_eq!(dropped.action, "install");
    assert_eq!(dropped.value, flagged.value);
    assert_eq!(dropped.value, path);
    let restarted = super::parse(&[
        "--restart".into(),
        "--run-dir".into(),
        dir.to_string_lossy().into_owned(),
        path.clone(),
    ])
    .unwrap();
    assert_eq!(restarted.value, path);
    assert_eq!(
        super::run_dir(["--run-dir".into(), dir.to_string_lossy().into_owned()]),
        Some(dir.clone())
    );
    crate::test_support::cleanup(&[&dir]);
}

