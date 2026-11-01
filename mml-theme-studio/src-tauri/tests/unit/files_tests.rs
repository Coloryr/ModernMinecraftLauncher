use super::*;

fn temp(name: &str) -> PathBuf {
    let path = mml_testutil::temp_path(format!("theme-studio-{name}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&path).unwrap();
    path
}

/// 保存支持中文与空格路径，清除 BOM 并保留外部编辑
#[tokio::test]
async fn saves_and_detects_external_edits() {
    let root = temp("files");
    let initial = save_text(
        &root,
        SaveInput {
            path: "src/中文 文件.css".into(),
            text: "\u{feff}a { color: red }".into(),
            expected_hash: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(initial.text, "a { color: red }");
    let bytes = std::fs::read(root.join(&initial.path)).unwrap();
    assert_ne!(bytes[0], 0xef);

    let saved = save_text(
        &root,
        SaveInput {
            path: initial.path.clone(),
            text: "b {}".into(),
            expected_hash: Some(initial.hash.clone()),
        },
    )
    .await
    .unwrap();
    assert_ne!(saved.hash, initial.hash);

    std::fs::write(root.join(&initial.path), "external").unwrap();
    let conflict = save_text(
        &root,
        SaveInput {
            path: initial.path.clone(),
            text: "overwrite".into(),
            expected_hash: Some(saved.hash.clone()),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(
        conflict.text(crate::messages::Locale::EnUs),
        "File changed outside the editor. Reopen it before saving: src/中文 文件.css"
    );
    assert_eq!(
        read_text(&root, &initial.path).await.unwrap().text,
        "external"
    );

    let exists = save_text(
        &root,
        SaveInput {
            path: initial.path.clone(),
            text: String::new(),
            expected_hash: None,
        },
    )
    .await
    .unwrap_err();
    assert!(
        exists
            .text(crate::messages::Locale::EnUs)
            .contains("already exists")
    );
}

/// 拒绝越界、依赖目录、符号链接以及二进制和大文件
#[tokio::test]
async fn rejects_unsafe_paths_and_files() {
    let root = temp("paths");
    for path in [
        "../secret.txt",
        "src/../../secret.txt",
        "C:/secret.txt",
        "/secret.txt",
        "src\\secret.txt",
        "node_modules/a.js",
        ".mml-deps/a.js",
        "a:stream.txt",
        "NUL.txt",
        "src /file.txt",
        "CON/test.css",
    ] {
        assert!(
            safe_path(&root, path, true).await.is_err(),
            "{path} should be rejected"
        );
    }

    let outside = temp("outside");
    let link = root.join("link");
    #[cfg(windows)]
    let linked = std::os::windows::fs::symlink_dir(&outside, &link).is_ok();
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(&outside, &link).is_ok();
    if linked {
        assert!(
            save_text(
                &root,
                SaveInput {
                    path: "link/new.txt".into(),
                    text: "x".into(),
                    expected_hash: None,
                },
            )
            .await
            .is_err()
        );
    }

    std::fs::write(root.join("binary.txt"), [1u8, 0, 2]).unwrap();
    assert!(read_text(&root, "binary.txt").await.is_err());
    std::fs::write(root.join("huge.txt"), "x".repeat((LIMIT + 1) as usize)).unwrap();
    assert!(read_text(&root, "huge.txt").await.is_err());
}

/// 文件树排除依赖、归档与目录链接
#[tokio::test]
async fn lists_only_editable_files() {
    let root = temp("tree");
    std::fs::create_dir_all(root.join("node_modules")).unwrap();
    std::fs::write(root.join("node_modules/secret.js"), "hidden").unwrap();
    std::fs::write(root.join("theme.zip"), "zip").unwrap();
    std::fs::write(root.join("package-lock.json"), "{}").unwrap();
    save_text(
        &root,
        SaveInput {
            path: "src/test.vue".into(),
            text: "<template />".into(),
            expected_hash: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(list_files(&root).await.unwrap(), vec!["src/test.vue"]);
}

