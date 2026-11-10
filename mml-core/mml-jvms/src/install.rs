//! Java 压缩包解压与已解压目录注册

use super::*;

/// 解压 Java 压缩包并注册其中的 Java
///
/// - `name`: Java 显示名称，`None` 时取压缩包文件名
/// - `path`: 压缩包路径
/// - `gui`: 解压进度回调
///
/// # 返回值
///
/// 解压并注册成功返回该 Java 的信息；格式不支持时返回 `Err`，其中找不到 Java 或测试不通过时返回 `None`
pub async fn unzip_java<P: AsRef<Path>>(
    name: Option<String>,
    path: P,
    gui: BaseArchiveGui,
) -> CoreResult<Option<Arc<JavaInfoObj>>> {
    let dir = extract_java(name, path, gui).await?;
    register_java_dir(dir).await
}

/// 只解压 Java 压缩包到 `java/<名字>`，不识别也不注册
///
/// 下载器只负责把 zip 拿到本地，解压单独作为一步，界面按阶段显示进度。
///
/// - `name`: 解压目录名；`None` 时取压缩包文件名
/// - `path`: 压缩包路径
/// - `gui`: 解压进度回调
///
/// # 返回值
///
/// 解压出来的目录路径；格式不支持时返回 `Err`
pub async fn extract_java<P: AsRef<Path>>(
    name: Option<String>,
    path: P,
    gui: BaseArchiveGui,
) -> CoreResult<PathBuf> {
    let path = path.as_ref().to_path_buf();
    // 解压是阻塞操作：丢到阻塞线程池，避免占住异步 worker
    tokio::task::spawn_blocking(move || extract_java_blocking(name, path, gui))
        .await
        .map_err(|_| ErrorType::InvalidOperation)?
}

/// 从已解压目录识别并注册 Java
///
/// - `dir`: 解压出来的目录
///
/// # 返回值
///
/// 注册成功的 Java；目录里找不到 Java 时返回 `Ok(None)`
pub async fn register_java_dir(dir: PathBuf) -> CoreResult<Option<Arc<JavaInfoObj>>> {
    // 探测要跑一次 java -version，同样是阻塞操作
    tokio::task::spawn_blocking(move || register_java_dir_blocking(dir))
        .await
        .map_err(|_| ErrorType::InvalidOperation)?
}

/// 解压 Java 压缩包的阻塞实现，调用方负责把它放到阻塞线程上
fn extract_java_blocking(
    name: Option<String>,
    path: PathBuf,
    gui: BaseArchiveGui,
) -> CoreResult<PathBuf> {
    let name = name.unwrap_or(
        path.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
    );

    let Some(arch) = ArchiveType::try_from_path(&path) else {
        return Err(ErrorType::InvalidOperation);
    };

    let dir = JAVA_DIR.get().unwrap().join(&name);
    archives::decompress(arch, &path, &dir, gui)?;
    Ok(dir)
}

/// 识别并注册的阻塞实现，调用方负责把它放到阻塞线程上
fn register_java_dir_blocking(dir: PathBuf) -> CoreResult<Option<Arc<JavaInfoObj>>> {
    let Some(data) = find_java_from_path(&dir) else {
        return Ok(None);
    };

    let file = if data.to_string_lossy().contains("jre")
        && let Some(path) = data
            .parent()
            .and_then(|data| data.parent())
            .map(|data| data.join("bin").join(data.file_name().unwrap()))
        && path.exists()
    {
        path
    } else {
        data
    };

    let Some(data) = java_helper::test_java(&file) else {
        return Ok(None);
    };

    Ok(Some(add_info_item(data)))
}
