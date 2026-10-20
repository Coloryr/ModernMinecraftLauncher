//! LiteLoader 加载器安装

use mml_base::{
    file_item::{FileHash, FileItemObj, LaterRun},
    serialize_tools,
};
use mml_names::i18_items::error_type::{ArgEmptyData, CoreResult, DataNotFoundData, ErrorType};
use mml_net::{liteloader_api, maven_utils::version_name_to_path};

use crate::{
    launcher::instance_setting_obj::InstanceSettingObj,
    launcher_path::{libraries_path, version_path},
    loader::liteloader_meta_obj::{LiteloaderMetaObj, LiteloaderVersionObj, LoaderObj},
};

/// 获取 LiteLoader 元数据
///
/// # 返回值
///
/// 返回解析后的元数据
pub async fn get_liteloader_meta() -> CoreResult<LiteloaderMetaObj> {
    let data = liteloader_api::get_meta().await?;
    let obj = serialize_tools::json_from_bytes::<LiteloaderMetaObj>(&data)?;
    Ok(obj)
}

/// 按加载器版本号从版本信息中取加载器数据
///
/// - `data`: 游戏版本对应的 LiteLoader 版本信息
/// - `version`: 加载器版本号
///
/// # 返回值
///
/// 返回加载器数据，优先正式版 artefacts，取不到再找快照 snapshots
pub fn find_loader_obj<'a>(data: &'a LiteloaderVersionObj, version: &str) -> Option<&'a LoaderObj> {
    data.artefacts
        .loader
        .get(version)
        .or_else(|| data.snapshots.loader.get(version))
}

/// LiteLoader 本体仓库：元数据里 1.12.2 的 `repo.url` 指向已下线的 mumfrey 仓库，
/// 但同一个包在 dl.liteloader.com 的 versions 目录下仍然可用
const LITELOADER_REPO: &str = "http://dl.liteloader.com/versions/";

/// 元数据里 `url` 为空的库按包名选默认仓库
///
/// launchwrapper 在 Mojang 库；asm-all 5.2 只在 Maven Central，Mojang 库没有
///
/// - `name`: Maven 坐标
///
/// # 返回值
///
/// 返回该坐标应使用的仓库根地址
fn default_repo(name: &str) -> &'static str {
    if name.starts_with("net.minecraft:") {
        "https://libraries.minecraft.net/"
    } else {
        "https://repo1.maven.org/maven2/"
    }
}

/// 获取 LiteLoader 运行库列表
///
/// - `mc`: 游戏版本号
/// - `version`: 加载器版本号
///
/// # 返回值
///
/// 返回运行库下载项列表；版本信息缺失或加载器版本不存在返回对应错误
pub async fn get_liteloader_lib(mc: &str, version: &str) -> CoreResult<Vec<FileItemObj>> {
    // 版本信息取缓存；无缓存时在线拉取并落盘
    let data = match version_path::get_liteloader(mc) {
        Some(data) => data,
        None => {
            let meta = get_liteloader_meta().await?;
            version_path::add_liteloader(meta);
            version_path::get_liteloader(mc)
                .ok_or(ErrorType::DataNotFound(DataNotFoundData::Info))?
        }
    };

    // 加载器数据与公共依赖取自同一组，正式版或快照，避免两组依赖重复
    let (loader, common_libs) = if let Some(loader) = data.artefacts.loader.get(version) {
        (loader, &data.artefacts.libraries)
    } else if let Some(loader) = data.snapshots.loader.get(version) {
        (loader, &data.snapshots.libraries)
    } else {
        return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
    };
    let mut list = Vec::new();

    // 公共依赖运行库，元数据不带校验值
    for item in common_libs {
        // LiteLoader 1.12.2 把 Mixin shade 进了本体（org/spongepowered/asm 共 494 个类），
        // 元数据里那份 0.7.4-SNAPSHOT 仓库早已下线，硬下只会拿到 404 页面
        if item.name.starts_with("org.spongepowered:mixin") {
            continue;
        }
        list.push(make_lib_item(&item.name, &item.url, FileHash::None));
    }

    // 加载器自身运行库，liteloader 本体带 MD5 校验
    for item in &loader.libraries {
        let hash = if item.name.starts_with("com.mumfrey:liteloader") {
            FileHash::Md5(loader.md5.clone())
        } else {
            FileHash::None
        };
        list.push(make_lib_item(&item.name, &item.url, hash));
    }

    // LiteLoader 本体：元数据只给了文件名与 MD5，Maven 坐标用 LoaderObj.version 拼，
    // 没有这一项主类换成 launchwrapper 也找不到 tweaker
    list.push(make_lib_item(
        &format!("com.mumfrey:liteloader:{}", loader.version),
        LITELOADER_REPO,
        FileHash::Md5(loader.md5.clone()),
    ));

    Ok(list)
}

/// 由 Maven 坐标生成运行库下载项
///
/// - `name`: Maven 坐标
/// - `url`: 仓库根地址
/// - `hash`: 文件校验值
///
/// # 返回值
///
/// 返回下载项
fn make_lib_item(name: &str, url: &str, hash: FileHash) -> FileItemObj {
    let path = version_name_to_path(name);
    // 空 url 表示元数据没写仓库：按坐标选默认仓库
    let url = if url.is_empty() {
        default_repo(name)
    } else {
        url
    };
    // 仓库根地址缺末尾分隔符时补上
    let url = if url.ends_with('/') {
        format!("{url}{path}")
    } else {
        format!("{url}/{path}")
    };

    FileItemObj {
        name: name.to_string(),
        file: libraries_path::get_lib_dir().join(&path),
        url,
        hash,
        later: LaterRun::None,
    }
}

impl InstanceSettingObj {
    /// 获取 LiteLoader 的全部运行库
    ///
    /// # 返回值
    ///
    /// 返回运行库下载项列表；未选择加载器版本或加载器版本不存在返回对应错误
    pub async fn get_liteloader_libs(&self) -> CoreResult<Vec<FileItemObj>> {
        let version = self
            .loader_version
            .as_ref()
            .ok_or(ErrorType::ArgEmpty(ArgEmptyData::Version))?;
        get_liteloader_lib(&self.version, version).await
    }
}
