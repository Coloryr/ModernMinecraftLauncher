//! 编辑器只访问项目内的文本文件；保存前比对版本，避免盖掉外部编辑。

use crate::messages::{MessageKey, StudioError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// 文件树与路径校验都跳过的目录
pub const IGNORED: &[&str] = &[
    ".git",
    ".mml-deps",
    "node_modules",
    "dist",
    "dist-theme",
    "target",
];
/// 可编辑的扩展名，含点
const TEXT_EXTENSIONS: &[&str] = &[
    ".vue",
    ".ts",
    ".js",
    ".mjs",
    ".cjs",
    ".json",
    ".css",
    ".html",
    ".md",
    ".svg",
    ".txt",
    ".gitignore",
];
/// 可编辑的完整文件名
const TEXT_FILES: &[&str] = &[".gitignore"];
/// 单个文本文件上限，单位字节
const LIMIT: u64 = 2 * 1024 * 1024;

/// 读到的源码文件
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceFile {
    /// 相对项目根目录的路径
    pub path: String,
    /// 文件文本
    pub text: String,
    /// 内容的 SHA-256，保存时用来发现外部修改
    pub hash: String,
}

/// 保存请求
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveInput {
    /// 相对项目根目录的路径
    pub path: String,
    /// 新文本
    pub text: String,
    /// 打开时的 hash；为空表示新建文件
    pub expected_hash: Option<String>,
}

/// 去掉 Windows canonicalize 加的 verbatim 前缀
///
/// 带 \\?\ 的路径显示给用户很难看，前端拼 Monaco 路径时也不该出现。
pub fn clean_path(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let text = path.to_string_lossy();
        if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = text.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    path
}

/// 路径是否位于根目录内
pub fn inside(root: &Path, path: &Path) -> bool {
    path.starts_with(root)
}

/// 校验并拼出项目内的路径
///
/// - `root`: 项目根目录
/// - `name`: 前端给的相对路径
/// - `create`: 缺少中间目录时是否创建
///
/// # 返回值
///
/// 确认过每一层都不是符号链接、且没越界的绝对路径
pub async fn safe_path(root: &Path, name: &str, create: bool) -> Result<PathBuf, StudioError> {
    validate_relative(name)?;
    let base = clean_path(tokio::fs::canonicalize(root).await?);
    let parts: Vec<&str> = name.split('/').collect();
    let mut current = base.clone();
    for (index, part) in parts.iter().enumerate() {
        current.push(part);
        match tokio::fs::symlink_metadata(&current).await {
            Ok(info) => {
                if info.file_type().is_symlink() {
                    return Err(StudioError::value(MessageKey::Outside, "path", name));
                }
                let real = clean_path(tokio::fs::canonicalize(&current).await?);
                if !inside(&base, &real) {
                    return Err(StudioError::value(MessageKey::Outside, "path", name));
                }
                current = real;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if !create {
                    return Err(error.into());
                }
                if index + 1 < parts.len() {
                    tokio::fs::create_dir(&current).await?;
                }
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(current)
}

/// 列出项目内可编辑的文本文件，按路径排序
pub async fn list_files(root: &Path) -> Result<Vec<String>, StudioError> {
    let mut result = Vec::new();
    let mut stack = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, prefix)) = stack.pop() {
        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().into_owned();
            let file_type = entry.file_type().await?;
            if IGNORED.contains(&name.as_str())
                || file_type.is_symlink()
                || name.ends_with(".zip")
                || name == "package-lock.json"
            {
                continue;
            }
            let relative = format!("{prefix}{name}");
            if file_type.is_dir() {
                stack.push((entry.path(), format!("{relative}/")));
            } else if is_text(&relative) {
                result.push(relative);
            }
            if result.len() > 5000 {
                return Err(StudioError::new(MessageKey::InvalidProject));
            }
        }
    }
    result.sort();
    Ok(result)
}

/// 读取项目内的文本文件
///
/// - `root`: 项目根目录
/// - `name`: 相对路径
pub async fn read_text(root: &Path, name: &str) -> Result<SourceFile, StudioError> {
    let target = safe_path(root, name, false).await?;
    if !is_text(name) {
        return Err(StudioError::value(MessageKey::InvalidFile, "path", name));
    }
    let info = tokio::fs::symlink_metadata(&target).await?;
    if info.len() > LIMIT || !info.is_file() {
        return Err(StudioError::value(MessageKey::InvalidFile, "path", name));
    }
    let bytes = tokio::fs::read(&target).await?;
    if bytes.contains(&0) {
        return Err(StudioError::value(MessageKey::InvalidFile, "path", name));
    }
    Ok(SourceFile {
        path: name.to_string(),
        text: String::from_utf8_lossy(&bytes).into_owned(),
        hash: hash(&bytes),
    })
}

/// 保存项目内的文本文件
///
/// - `root`: 项目根目录
/// - `input`: 路径、文本与打开时的 hash
pub async fn save_text(root: &Path, input: SaveInput) -> Result<SourceFile, StudioError> {
    if input.text.len() as u64 > LIMIT || !is_text(&input.path) {
        return Err(StudioError::value(
            MessageKey::InvalidFile,
            "path",
            &input.path,
        ));
    }
    let create = input.expected_hash.is_none();
    let target = safe_path(root, &input.path, create).await?;
    if let Some(expected) = &input.expected_hash {
        let current = read_text(root, &input.path).await?;
        if &current.hash != expected {
            return Err(StudioError::value(
                MessageKey::Conflict,
                "path",
                &input.path,
            ));
        }
    }
    let text = input.text.strip_prefix('\u{feff}').unwrap_or(&input.text);
    let write = async {
        if create {
            use tokio::io::AsyncWriteExt;
            let mut file = tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
                .await?;
            file.write_all(text.as_bytes()).await
        } else {
            tokio::fs::write(&target, text).await
        }
    };
    match write.await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(StudioError::value(MessageKey::Exists, "path", &input.path));
        }
        Err(error) => return Err(error.into()),
    }
    read_text(root, &input.path).await
}

/// 写 JSON 文件，UTF-8 无 BOM
pub async fn write_json(path: &Path, value: &serde_json::Value) -> Result<(), StudioError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let text =
        serde_json::to_string_pretty(value).map_err(|error| StudioError::raw(error.to_string()))?;
    tokio::fs::write(path, text + "\n").await?;
    Ok(())
}

/// 内容的 SHA-256 十六进制串
pub fn hash(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    let mut text = String::with_capacity(digest.len() * 2);
    for byte in digest {
        text.push_str(&format!("{byte:02x}"));
    }
    text
}

fn is_text(name: &str) -> bool {
    let base = name.rsplit('/').next().unwrap_or(name);
    if TEXT_FILES.contains(&base) {
        return true;
    }
    match base.rfind('.') {
        Some(index) => TEXT_EXTENSIONS.contains(&&base[index..]),
        None => false,
    }
}

fn validate_relative(name: &str) -> Result<(), StudioError> {
    let outside = || StudioError::value(MessageKey::Outside, "path", name);
    if name.is_empty()
        || Path::new(name).is_absolute()
        || name.contains(['\\', ':', '<', '>', '"', '|', '?', '*'])
        || name.chars().any(|value| (value as u32) < 32)
    {
        return Err(outside());
    }
    for part in name.split('/') {
        if part.is_empty()
            || part.ends_with('.')
            || part.ends_with(' ')
            || IGNORED.contains(&part)
            || is_reserved(part)
        {
            return Err(outside());
        }
    }
    Ok(())
}

fn is_reserved(part: &str) -> bool {
    let lower = part.to_ascii_lowercase();
    let base = lower.split('.').next().unwrap_or("");
    matches!(base, "con" | "prn" | "aux" | "nul")
        || (base.len() == 4
            && (base.starts_with("com") || base.starts_with("lpt"))
            && base.as_bytes()[3].is_ascii_digit())
}

#[cfg(test)]
mod tests {
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
}
