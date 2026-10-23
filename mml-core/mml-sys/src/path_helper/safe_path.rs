//! 不可信相对路径的目录边界检查。

use super::{CoreResult, ErrorType, Path, PathBuf, fs, fs_error, io};

/// 将不可信相对路径限制在指定目录，拒绝父目录、绝对路径及相对路径上的已有链接。
///
/// - `root`: 可信根目录，可以尚未创建
/// - `name`: 相对路径，同时识别两种平台的分隔符
///
/// # 返回值
///
/// 返回安全目标路径；路径越界、经过链接或无法检查时返回错误。
pub fn safe_join(root: &Path, name: &str) -> CoreResult<PathBuf> {
    let reject = || ErrorType::UnsafePath {
        root: root.to_path_buf(),
        path: PathBuf::from(name),
    };
    let normalized = name.replace('\\', "/");
    if normalized.starts_with('/') || normalized.contains('\0') {
        return Err(reject());
    }
    let mut relative = PathBuf::new();
    for part in normalized
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
    {
        // 在 Unix 上也拒绝 Windows 盘符，避免同一清单在不同平台越界。
        if part == ".." || part.as_bytes().get(1) == Some(&b':') || Path::new(part).has_root() {
            return Err(reject());
        }
        relative.push(part);
    }
    let canonical_root = match root.canonicalize() {
        Ok(path) => Some(path),
        Err(err) if err.kind() == io::ErrorKind::NotFound => None,
        Err(err) => return Err(fs_error(root, err)),
    };
    let mut path = root.to_path_buf();
    for part in relative.components() {
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(reject());
                }
                if let Some(root) = &canonical_root {
                    let canonical = path.canonicalize().map_err(|err| fs_error(&path, err))?;
                    if !canonical.starts_with(root) {
                        return Err(reject());
                    }
                }
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(fs_error(&path, err)),
        }
    }
    Ok(path)
}
