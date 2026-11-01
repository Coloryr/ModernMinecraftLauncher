use super::*;

/// 官方启动器布局：游戏数据在根目录
#[test]
fn game_data_root_uses_root_when_not_isolated() {
    let dir =
        mml_testutil::temp_path(format!("minecraft-detect-root-{}", uuid::Uuid::new_v4()));
    let version = dir.join(names::VERSION_DIR).join("1.21.4");
    std::fs::create_dir_all(&version).unwrap();
    std::fs::write(version.join("1.21.4.json"), "{}").unwrap();
    std::fs::create_dir_all(dir.join("mods")).unwrap();

    let (root, entries) = game_data_root(&dir, "1.21.4");
    assert_eq!(root, dir);
    assert_eq!(entries, vec![dir.join("mods")]);

    let _ = std::fs::remove_dir_all(&dir);
}

/// HMCL / PCL 的版本隔离：游戏数据在 versions/<id>/ 里，版本 json 与 jar 不算
#[test]
fn game_data_root_uses_version_dir_when_isolated() {
    let dir = mml_testutil::temp_path(format!("minecraft-detect-iso-{}", uuid::Uuid::new_v4()));
    let version = dir.join(names::VERSION_DIR).join("1.20.1");
    std::fs::create_dir_all(version.join("mods")).unwrap();
    std::fs::create_dir_all(version.join("PCL")).unwrap();
    std::fs::write(version.join("1.20.1.json"), "{}").unwrap();
    std::fs::write(version.join("1.20.1.jar"), "").unwrap();

    let (root, entries) = game_data_root(&dir, "1.20.1");
    assert_eq!(root, version);
    assert_eq!(entries, vec![version.join("mods")]);

    let _ = std::fs::remove_dir_all(&dir);
}

