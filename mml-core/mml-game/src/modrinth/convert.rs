//! Modrinth 版本信息到下载项、在线文件信息的转换

use super::*;

/// 转换为 Modrinth 的加载器 ID
///
/// - `loader`: 加载器类型
///
/// # 返回值
///
/// 返回加载器 ID；该加载器不被 Modrinth 支持返回 `None`
pub fn to_loader_id(loader: &LoaderType) -> Option<String> {
    match loader {
        LoaderType::Forge => Some(loader.id().to_string()),
        LoaderType::Fabric => Some(loader.id().to_string()),
        LoaderType::Quilt => Some(loader.id().to_string()),
        LoaderType::NeoForge => Some(loader.id().to_string()),
        LoaderType::Cleanroom => Some(String::from("forge")),
        _ => None,
    }
}

/// 创建下载项目
///
/// - `obj`: Modrinth 版本信息
/// - `path`: 保存目录
///
/// # 返回值
///
/// 返回主文件下载项
pub fn make_download_obj<P: AsRef<Path>>(obj: &ModrinthVersionObj, path: P) -> FileItemObj {
    let file = obj
        .files
        .iter()
        .filter(|item| item.primary)
        .next()
        .unwrap_or(&obj.files[0]);
    FileItemObj {
        name: obj.name.clone(),
        file: path.as_ref().join(&file.filename),
        url: file.url.clone(),
        hash: FileHash::Sha1Sha512(file.hashes.sha1.clone(), file.hashes.sha512.clone()),
        later: LaterRun::None,
    }
}

/// 创建整合包内文件的下载项目
///
/// - `obj`: 整合包文件信息
/// - `path`: 保存目录
///
/// # 返回值
///
/// 返回下载项
pub fn make_pack_download_obj<P: AsRef<Path>>(obj: &ModrinthPackFileObj, path: P) -> FileItemObj {
    FileItemObj {
        url: obj.downloads[0].clone(),
        name: obj.path.clone(),
        file: path.as_ref().join(&obj.path),
        hash: FileHash::Sha1Sha512(obj.hashes.sha1.clone(), obj.hashes.sha512.clone()),
        ..Default::default()
    }
}

/// 创建在线文件信息
///
/// - `obj`: Modrinth 版本信息
/// - `path`: 实例内相对目录
///
/// # 返回值
///
/// 返回在线文件信息
pub fn make_file_online_obj(obj: &ModrinthVersionObj, path: &str) -> OnlineInfoObj {
    let file = obj
        .files
        .iter()
        .filter(|item| item.primary)
        .next()
        .unwrap_or(&obj.files[0]);
    OnlineInfoObj {
        path: path.to_string(),
        name: obj.name.clone(),
        file: file.filename.clone(),
        sha1: file.hashes.sha1.clone(),
        url: file.url.clone(),
        modid: obj.project_id.clone(),
        fileid: obj.id.clone(),
    }
}
