//! 主题项目的创建、打开、安装与构建
//!
//! Demo 和 SDK 来自同一份检出：先把 demo 复制到缓存里的暂存目录、补齐 .mml-deps，
//! 确认完整后再创建用户目录，失败不会留下半成品。

use crate::{
    files::{IGNORED, clean_path, inside, write_json},
    messages::{MessageKey, StudioError},
    process::{LogFn, RunOptions, environment, no_log, run},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};
use tokio_util::sync::CancellationToken;

/// 官方仓库
pub const REPOSITORY: &str = "https://github.com/Coloryr/ModernMinecraftLauncher.git";

/// 四种 demo
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Template {
    /// CSS 样式包
    Css,
    /// Vue 整页主题
    Page,
    /// Vue 主页组件
    Home,
    /// HTML 整页主题
    Html,
}

impl Template {
    /// 解析模板标识
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "css" => Some(Self::Css),
            "page" => Some(Self::Page),
            "home" => Some(Self::Home),
            "html" => Some(Self::Html),
            _ => None,
        }
    }

    /// 模板标识
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Css => "css",
            Self::Page => "page",
            Self::Home => "home",
            Self::Html => "html",
        }
    }

    /// 官方仓库里的 demo 目录名
    pub fn dir(self) -> &'static str {
        match self {
            Self::Css => "custom-ui-css-demo",
            Self::Page => "custom-ui-demo",
            Self::Home => "custom-main-demo",
            Self::Html => "custom-ui-html-demo",
        }
    }
}

/// 打开的主题项目
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    /// 项目根目录
    pub root: String,
    /// 显示名，取 theme.json 的 name
    pub name: String,
    /// demo 类型
    pub template: Template,
    /// 来源提交，手工创建的项目为空串
    pub revision: String,
}

/// 新建项目请求
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectInput {
    /// 父目录
    pub parent: String,
    /// 项目目录名
    pub name: String,
    /// demo 类型
    pub template: String,
    /// 本地仓库；空串表示从官方仓库下载
    pub source: String,
}

/// 校验项目目录名
///
/// - `name`: 用户输入的项目名
pub fn validate_name(name: &str) -> Result<(), StudioError> {
    let invalid = name.trim().is_empty()
        || name.chars().count() > 80
        || name.contains(['<', '>', ':', '"', '/', '\\', '|', '?', '*'])
        || name.chars().any(|value| (value as u32) < 32)
        || name.ends_with('.')
        || name.ends_with(' ')
        || name == ".."
        || is_reserved(name);
    if invalid {
        Err(StudioError::new(MessageKey::InvalidName))
    } else {
        Ok(())
    }
}

/// 从 demo 创建项目
///
/// - `input`: 父目录、目录名、模板、可选本地仓库
/// - `cache`: 下载与暂存目录
/// - `cancel`: 取消信号
/// - `log`: 命令输出去向
pub async fn create_project(
    input: CreateProjectInput,
    cache: &Path,
    cancel: &CancellationToken,
    log: &LogFn,
) -> Result<Project, StudioError> {
    if cancel.is_cancelled() {
        return Err(StudioError::new(MessageKey::Cancelled));
    }
    validate_name(&input.name)?;
    let template = Template::parse(&input.template)
        .ok_or_else(|| StudioError::new(MessageKey::InvalidProject))?;
    let parent = clean_path(tokio::fs::canonicalize(&input.parent).await?);
    let destination = parent.join(&input.name);
    if tokio::fs::try_exists(&destination).await.unwrap_or(false) {
        return Err(StudioError::value(
            MessageKey::Exists,
            "path",
            destination.to_string_lossy(),
        ));
    }

    tokio::fs::create_dir_all(cache).await?;
    let stage = cache.join(format!("demo-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&stage).await?;
    let result = async {
        let prepared = prepare(&input, template, &stage, cancel, log).await?;
        if cancel.is_cancelled() {
            return Err(StudioError::new(MessageKey::Cancelled));
        }
        let created = async {
            tokio::fs::create_dir(&destination).await?;
            // 暂存目录已经过滤过一遍，这里要连 .mml-deps 一起搬进用户项目
            copy_all(&prepared, &destination).await?;
            if cancel.is_cancelled() {
                return Err(StudioError::new(MessageKey::Cancelled));
            }
            open_project(&destination).await
        }
        .await;
        if created.is_err() {
            // 目标目录是本函数刚创建的，出错时只清理它
            let _ = tokio::fs::remove_dir_all(&destination).await;
        }
        created
    }
    .await;
    // stage 是刚创建的临时目录，删之前再确认它仍在缓存目录下
    if inside(cache, &stage) {
        let _ = tokio::fs::remove_dir_all(&stage).await;
    }
    result
}

/// 组装 sparse-checkout 参数
///
/// 三个路径必须各占一个参数：带空格的单个参数会被 git 当成一个不存在的目录名，
/// 结果只有空的 custom/ 被检出，后面检查 demo 文件时就会误报 badSource。
fn sparse_checkout_args(repo: &Path, template: Template) -> Vec<String> {
    vec![
        "-C".into(),
        repo.to_string_lossy().into_owned(),
        "sparse-checkout".into(),
        "set".into(),
        format!("custom/{}", template.dir()),
        "mml-vue".into(),
        "mml-ipc".into(),
    ]
}

async fn prepare(
    input: &CreateProjectInput,
    template: Template,
    stage: &Path,
    cancel: &CancellationToken,
    log: &LogFn,
) -> Result<PathBuf, StudioError> {
    let remote = input.source.trim().is_empty();
    let repo = if remote {
        stage.join("repo")
    } else {
        clean_path(tokio::fs::canonicalize(&input.source).await?)
    };
    if remote {
        run(
            "git",
            &[
                "-c".into(),
                "core.longpaths=true".into(),
                "clone".into(),
                "--depth".into(),
                "1".into(),
                "--filter=blob:none".into(),
                "--sparse".into(),
                "--".into(),
                REPOSITORY.into(),
                repo.to_string_lossy().into_owned(),
            ],
            RunOptions {
                cwd: None,
                log: log.clone(),
                cancel: Some(cancel),
            },
        )
        .await?;
        run(
            "git",
            &sparse_checkout_args(&repo, template),
            RunOptions {
                cwd: None,
                log: log.clone(),
                cancel: Some(cancel),
            },
        )
        .await?;
    }

    let demo = repo.join("custom").join(template.dir());
    for file in [
        demo.join("package.json"),
        demo.join("src/theme.json"),
        repo.join("mml-ipc/package.json"),
        repo.join("mml-vue/package.json"),
    ] {
        if !tokio::fs::try_exists(&file).await.unwrap_or(false) {
            return Err(StudioError::value(
                MessageKey::BadSource,
                "path",
                file.to_string_lossy(),
            ));
        }
    }

    let revision = match run(
        "git",
        &[
            "-C".into(),
            repo.to_string_lossy().into_owned(),
            "rev-parse".into(),
            "HEAD".into(),
        ],
        RunOptions {
            cwd: None,
            log: no_log(),
            cancel: Some(cancel),
        },
    )
    .await
    {
        Ok(value) => value,
        Err(error) if cancel.is_cancelled() => return Err(error),
        Err(_) => String::new(),
    };

    let prepared = stage.join("project");
    copy_sources(&demo, &prepared).await?;
    if template != Template::Html {
        for package in ["mml-ipc", "mml-vue"] {
            copy_sources(
                &repo.join(package),
                &prepared.join(".mml-deps").join(package),
            )
            .await?;
        }
        let package_path = prepared.join("package.json");
        let text = tokio::fs::read_to_string(&package_path).await?;
        let mut package: Value = serde_json::from_str(&text)
            .map_err(|_| StudioError::new(MessageKey::InvalidProject))?;
        if !package.get("dependencies").is_some_and(Value::is_object) {
            package["dependencies"] = json!({});
        }
        let dependencies = package
            .get_mut("dependencies")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| StudioError::new(MessageKey::InvalidProject))?;
        dependencies.insert("mml-ipc".into(), json!("file:.mml-deps/mml-ipc"));
        dependencies.insert("mml-vue".into(), json!("file:.mml-deps/mml-vue"));
        // 工具已随 demo 固定 SDK，安装时不再运行示例的仓库探测与二次克隆
        if let Some(scripts) = package.get_mut("scripts").and_then(Value::as_object_mut) {
            scripts.remove("preinstall");
            scripts.remove("deps");
        }
        write_json(&package_path, &package).await?;
    }
    write_json(
        &prepared.join(".mml-deps/studio.json"),
        &json!({
            "template": template.as_str(),
            "revision": revision,
            "source": if remote { REPOSITORY } else { &input.source },
        }),
    )
    .await?;
    Ok(prepared)
}

/// 打开项目并识别模板
pub async fn open_project(root: &Path) -> Result<Project, StudioError> {
    let invalid = || StudioError::new(MessageKey::InvalidProject);
    let root = clean_path(tokio::fs::canonicalize(root).await?);
    let manifest = read_json(&root.join("src/theme.json"))
        .await
        .map_err(|_| invalid())?;
    let package = read_json(&root.join("package.json"))
        .await
        .map_err(|_| invalid())?;
    let kind = manifest
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(invalid)?;
    let template = if kind == "page"
        && tokio::fs::try_exists(root.join("src/index.html"))
            .await
            .unwrap_or(false)
    {
        Template::Html
    } else {
        Template::parse(kind).ok_or_else(invalid)?
    };
    if !package
        .get("scripts")
        .and_then(|scripts| scripts.get("build"))
        .is_some_and(Value::is_string)
    {
        return Err(invalid());
    }
    let revision = read_json(&root.join(".mml-deps/studio.json"))
        .await
        .ok()
        .and_then(|value| {
            value
                .get("revision")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default();
    let name = manifest
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .or_else(|| {
            root.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_default();
    Ok(Project {
        root: root.to_string_lossy().into_owned(),
        name,
        template,
        revision,
    })
}

/// 安装项目依赖
pub async fn install_project(
    project: &Project,
    cancel: &CancellationToken,
    log: &LogFn,
) -> Result<(), StudioError> {
    let env = environment().await?;
    let root = Path::new(&project.root);
    let node = env.node.to_string_lossy().into_owned();
    let npm = env.npm.to_string_lossy().into_owned();
    if !tokio::fs::try_exists(root.join(".mml-deps/studio.json"))
        .await
        .unwrap_or(false)
        && tokio::fs::try_exists(root.join("scripts/deps.mjs"))
            .await
            .unwrap_or(false)
    {
        // npm 在 preinstall 之前就可能解析 file 依赖，必须先准备契约包
        run(
            &node,
            &["scripts/deps.mjs".into()],
            RunOptions {
                cwd: Some(root),
                log: log.clone(),
                cancel: Some(cancel),
            },
        )
        .await?;
    }
    run(
        &node,
        &[
            npm,
            "install".into(),
            "--no-audit".into(),
            "--no-fund".into(),
        ],
        RunOptions {
            cwd: Some(root),
            log: log.clone(),
            cancel: Some(cancel),
        },
    )
    .await?;
    Ok(())
}

/// 构建项目，返回本次新生成的 ZIP
pub async fn build_project(
    project: &Project,
    cancel: &CancellationToken,
    log: &LogFn,
) -> Result<Vec<String>, StudioError> {
    let env = environment().await?;
    let root = Path::new(&project.root);
    let node = env.node.to_string_lossy().into_owned();
    let npm = env.npm.to_string_lossy().into_owned();

    let mut before: std::collections::HashMap<String, (Option<SystemTime>, Option<SystemTime>)> =
        std::collections::HashMap::new();
    let mut entries = tokio::fs::read_dir(root).await?;
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".zip") {
            let info = entry.metadata().await?;
            before.insert(name, (info.modified().ok(), info.created().ok()));
        }
    }

    run(
        &node,
        &[npm, "run".into(), "build".into()],
        RunOptions {
            cwd: Some(root),
            log: log.clone(),
            cancel: Some(cancel),
        },
    )
    .await?;

    let mut artifacts = Vec::new();
    let mut entries = tokio::fs::read_dir(root).await?;
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".zip") {
            continue;
        }
        let info = entry.metadata().await?;
        let current = (info.modified().ok(), info.created().ok());
        let changed = before.get(&name) != Some(&current);
        if info.is_file() && changed {
            artifacts.push(root.join(name).to_string_lossy().into_owned());
        }
    }
    if artifacts.is_empty() {
        return Err(StudioError::new(MessageKey::MissingBuild));
    }
    Ok(artifacts)
}

/// 递归复制，跳过依赖目录、归档与符号链接
async fn copy_sources(source: &Path, target: &Path) -> Result<(), StudioError> {
    tokio::fs::create_dir_all(target).await?;
    let mut stack = vec![PathBuf::new()];
    while let Some(relative) = stack.pop() {
        let directory = source.join(&relative);
        let mut entries = tokio::fs::read_dir(&directory).await?;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().into_owned();
            let file_type = entry.file_type().await?;
            if file_type.is_symlink() || IGNORED.contains(&name.as_str()) || name.ends_with(".zip")
            {
                continue;
            }
            let child = relative.join(&name);
            let destination = target.join(&child);
            if file_type.is_dir() {
                tokio::fs::create_dir_all(&destination).await?;
                stack.push(child);
            } else if file_type.is_file() {
                if let Some(parent) = destination.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                tokio::fs::copy(entry.path(), &destination).await?;
            }
        }
    }
    Ok(())
}

/// 递归复制整棵目录，只跳过符号链接
async fn copy_all(source: &Path, target: &Path) -> Result<(), StudioError> {
    tokio::fs::create_dir_all(target).await?;
    let mut stack = vec![PathBuf::new()];
    while let Some(relative) = stack.pop() {
        let directory = source.join(&relative);
        let mut entries = tokio::fs::read_dir(&directory).await?;
        while let Some(entry) = entries.next_entry().await? {
            let file_type = entry.file_type().await?;
            if file_type.is_symlink() {
                continue;
            }
            let child = relative.join(entry.file_name());
            let destination = target.join(&child);
            if file_type.is_dir() {
                tokio::fs::create_dir_all(&destination).await?;
                stack.push(child);
            } else if file_type.is_file() {
                if let Some(parent) = destination.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                tokio::fs::copy(entry.path(), &destination).await?;
            }
        }
    }
    Ok(())
}

async fn read_json(path: &Path) -> Result<Value, StudioError> {
    let text = tokio::fs::read_to_string(path).await?;
    serde_json::from_str(&text).map_err(|error| StudioError::raw(error.to_string()))
}

fn is_reserved(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let base = lower.split('.').next().unwrap_or("");
    matches!(base, "con" | "prn" | "aux" | "nul")
        || (base.len() == 4
            && (base.starts_with("com") || base.starts_with("lpt"))
            && base.as_bytes()[3].is_ascii_digit())
}

/// 供测试构造最小项目
#[cfg(test)]
async fn fixture(root: &Path, template: &str) {
    write_json(
        &root.join("package.json"),
        &json!({ "type": "module", "scripts": { "build": "node build.mjs" } }),
    )
    .await
    .unwrap();
    write_json(
        &root.join("src/theme.json"),
        &json!({ "name": "Studio test", "type": template, "version": "1" }),
    )
    .await
    .unwrap();
}

#[cfg(test)]
mod tests {
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
}
