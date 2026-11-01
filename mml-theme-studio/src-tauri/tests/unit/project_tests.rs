use super::*;
use crate::files::list_files;
use std::sync::Arc;

fn temp(name: &str) -> PathBuf {
    let path = mml_testutil::temp_path(format!("theme-studio-{name}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn log() -> LogFn {
    Arc::new(|_| {})
}

/// sparse-checkout 的三个路径必须各占一个参数，不能被拼成一条
#[test]
fn sparse_checkout_paths_are_separate_arguments() {
    let args = sparse_checkout_args(Path::new("repo"), Template::Css);
    assert_eq!(
        &args[args.len() - 3..],
        &[
            "custom/custom-ui-css-demo".to_string(),
            "mml-vue".to_string(),
            "mml-ipc".to_string()
        ]
    );
    assert!(!args.iter().any(|arg| arg.split_whitespace().count() > 1));
}

/// 项目名拒绝保留名称与路径注入
#[test]
fn rejects_unsafe_names() {
    for name in [
        "", "..", ".", "CON", "nul.css", "LPT1", "a/b", "a\\b", "name.", "name ", "\0bad",
    ] {
        assert!(validate_name(name).is_err(), "{name} should be rejected");
    }
    assert!(validate_name("主题 demo").is_ok());
}

/// 四类模板完整复制，锁定 SDK，已有目标不覆盖
#[tokio::test]
async fn copies_templates_and_locks_sdk() {
    let root = temp("projects");
    let source = root.join("repo");
    for package in ["mml-ipc", "mml-vue"] {
        write_json(
            &source.join(package).join("package.json"),
            &json!({ "name": package }),
        )
        .await
        .unwrap();
    }
    for template in [
        Template::Css,
        Template::Page,
        Template::Home,
        Template::Html,
    ] {
        let demo = source.join("custom").join(template.dir());
        let kind = if template == Template::Html {
            "page"
        } else {
            template.as_str()
        };
        fixture(&demo, kind).await;
        if template == Template::Html {
            std::fs::write(demo.join("src/index.html"), "<html></html>").unwrap();
        }
        std::fs::write(demo.join("old.zip"), "do not copy").unwrap();
        std::fs::create_dir_all(demo.join("node_modules")).unwrap();
        std::fs::write(demo.join("node_modules/secret.txt"), "do not copy").unwrap();

        let input = CreateProjectInput {
            parent: root.to_string_lossy().into_owned(),
            name: template.as_str().into(),
            template: template.as_str().into(),
            source: source.to_string_lossy().into_owned(),
        };
        let cache = root.join("cache");
        let token = CancellationToken::new();
        let project = create_project(input.clone(), &cache, &token, &log())
            .await
            .unwrap();
        assert_eq!(project.template, template);
        assert_eq!(
            open_project(Path::new(&project.root)).await.unwrap().name,
            "Studio test"
        );
        assert!(
            !std::fs::read_dir(&project.root)
                .unwrap()
                .any(|entry| entry.unwrap().file_name() == "old.zip")
        );
        assert!(!project.root.ends_with("node_modules"));
        if template != Template::Html {
            let package = read_json(&Path::new(&project.root).join("package.json"))
                .await
                .unwrap();
            assert_eq!(
                package["dependencies"]["mml-vue"],
                json!("file:.mml-deps/mml-vue")
            );
            assert!(package["scripts"].get("preinstall").is_none());
            assert!(
                Path::new(&project.root)
                    .join(".mml-deps/mml-vue/package.json")
                    .is_file()
            );
        }
        assert!(create_project(input, &cache, &token, &log()).await.is_err());
        assert_eq!(
            open_project(Path::new(&project.root))
                .await
                .unwrap()
                .template,
            template
        );
    }
    assert_eq!(std::fs::read_dir(root.join("cache")).unwrap().count(), 0);
}

/// 模板损坏或取消时不留下半成品目标
#[tokio::test]
async fn cleans_up_failed_creates() {
    let root = temp("invalid-project");
    let input = CreateProjectInput {
        parent: root.to_string_lossy().into_owned(),
        name: "new".into(),
        template: "css".into(),
        source: root.to_string_lossy().into_owned(),
    };
    let cache = root.join("cache");
    let token = CancellationToken::new();
    let error = create_project(input.clone(), &cache, &token, &log())
        .await
        .unwrap_err();
    assert!(
        error
            .text(crate::messages::Locale::EnUs)
            .contains("repository")
    );
    assert!(!root.join("new").exists());
    assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 0);
    assert!(open_project(&root).await.is_err());

    token.cancel();
    assert!(create_project(input, &cache, &token, &log()).await.is_err());
    assert!(!root.join("new").exists());
}

/// 构建失败不能被旧 ZIP 掩盖，成功返回本次新产物
#[tokio::test]
async fn builds_only_fresh_archives() {
    let root = temp("build");
    fixture(&root, "css").await;
    std::fs::write(root.join("old.zip"), "old").unwrap();
    std::fs::write(root.join("build.mjs"), "process.exit(0);").unwrap();
    let project = open_project(&root).await.unwrap();
    let token = CancellationToken::new();
    assert!(build_project(&project, &token, &log()).await.is_err());

    std::fs::write(
        root.join("build.mjs"),
        "import {writeFileSync} from 'node:fs'; writeFileSync('new.zip','new');",
    )
    .unwrap();
    let artifacts = build_project(&project, &token, &log()).await.unwrap();
    assert_eq!(artifacts.len(), 1);
    assert!(artifacts[0].ends_with("new.zip"));

    std::fs::write(root.join("build.mjs"), "process.exit(3);").unwrap();
    assert!(build_project(&project, &token, &log()).await.is_err());
}

/// 文件树排除规则与项目根一致
#[tokio::test]
async fn open_project_requires_manifest_and_build_script() {
    let root = temp("open");
    assert!(open_project(&root).await.is_err());
    fixture(&root, "css").await;
    let project = open_project(&root).await.unwrap();
    assert_eq!(project.template, Template::Css);
    assert!(
        list_files(&root)
            .await
            .unwrap()
            .contains(&"src/theme.json".to_string())
    );
}

