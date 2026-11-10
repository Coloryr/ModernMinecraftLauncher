//! CurseForge 版本信息到下载项、在线文件信息的转换

use super::*;

/// 排序编号
pub enum CurseForgeSortField {
    /// 精选
    Featured,
    /// 热度
    Popularity,
    /// 最后更新
    LastUpdated,
    /// 名字
    Name,
    /// 作者
    Author,
    /// 下载量
    TotalDownloads,
    /// 分类
    Category,
    /// 游戏版本
    GameVersion,
}

impl CurseForgeSortField {
    /// 获取排序编号
    ///
    /// # 返回值
    ///
    /// 返回 CurseForge API 对应的排序 ID
    pub fn get_id(&self) -> u32 {
        match self {
            CurseForgeSortField::Featured => 1,
            CurseForgeSortField::Popularity => 2,
            CurseForgeSortField::LastUpdated => 3,
            CurseForgeSortField::Name => 4,
            CurseForgeSortField::Author => 5,
            CurseForgeSortField::TotalDownloads => 6,
            CurseForgeSortField::Category => 7,
            CurseForgeSortField::GameVersion => 8,
        }
    }
}

/// 转换为 CurseForge 的加载器 ID
///
/// - `loader`: 加载器类型
///
/// # 返回值
///
/// 返回加载器 ID；该加载器不被 CurseForge 支持返回 `None`
pub fn to_loader_id(loader: &LoaderType) -> Option<u32> {
    match loader {
        LoaderType::Forge => Some(1),
        LoaderType::Fabric => Some(4),
        LoaderType::Quilt => Some(5),
        LoaderType::NeoForge => Some(6),
        LoaderType::Cleanroom => Some(1),
        _ => None,
    }
}

/// 创建下载项目
///
/// - `obj`: CurseForge 文件信息
/// - `path`: 保存目录
///
/// # 返回值
///
/// 返回主文件下载项
pub fn make_file_item_obj<P: AsRef<Path>>(obj: &mut CurseForgeFileDataObj, path: P) -> FileItemObj {
    obj.fix_download_url();

    FileItemObj {
        url: obj.download_url.clone().unwrap(),
        name: obj.display_name.clone(),
        file: path.as_ref().join(&obj.file_name),
        hash: FileHash::Sha1(obj.sha1_hash()),
        later: LaterRun::None,
    }
}

/// 创建在线文件信息
///
/// - `obj`: CurseForge 文件信息
/// - `path`: 实例内相对目录
///
/// # 返回值
///
/// 返回在线文件信息
pub fn make_file_online_info_obj(obj: &mut CurseForgeFileDataObj, path: &str) -> OnlineInfoObj {
    obj.fix_download_url();

    OnlineInfoObj {
        path: path.to_string(),
        name: obj.display_name.clone(),
        file: obj.file_name.clone(),
        sha1: obj.sha1_hash(),
        url: obj.download_url.clone().unwrap_or_default(),
        modid: obj.mod_id.to_string(),
        fileid: obj.id.to_string(),
    }
}
