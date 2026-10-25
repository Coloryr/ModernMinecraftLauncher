//! 平台查询：取当前实例可用的最新文件
//!
//! 按游戏版本与加载器过滤，两个平台各有一套口径。

use mml_game::launcher::ModPackType;
use mml_game::loader::LoaderType;
use mml_game::{curseforge, modrinth};
use mml_net::curseforge_api::{self, CurseFogreArg, file_obj::CurseForgeFileDataObj};
use mml_net::modrinth_api::{self, version_obj::ModrinthVersionObj};

use crate::dtos::ModUpdateDto;

use super::check::Target;

/// 查一个模组，判断装上的是不是最新
///
/// - `target`: 待检查的模组
/// - `version`: 实例的游戏版本
/// - `loader`: 实例的加载器
///
/// # 返回值
///
/// 有更新时是那条新文件；已是最新，或平台在当前版本 / 加载器下没有文件时返回 `Ok(None)`；
/// 请求失败返回 `Err`
pub(super) async fn check_one(
    target: &Target,
    version: &str,
    loader: LoaderType,
) -> Result<Option<ModUpdateDto>, String> {
    let latest = match target.source {
        ModPackType::CurseForge => latest_curseforge(&target.project_id, version, loader).await?,
        ModPackType::Modrinth => latest_modrinth(&target.project_id, version, loader).await?,
        // 其余来源没有查询接口，也不该带着在线信息出现在这里
        _ => return Ok(None),
    };
    let Some(latest) = latest else {
        return Ok(None);
    };
    // 文件号一样就是同一个文件；平台已按版本与加载器过滤，剩下的都是本实例能装的
    if latest.id == target.file_id {
        return Ok(None);
    }
    Ok(Some(ModUpdateDto {
        uuid: target.uuid.clone(),
        file: target.file.clone(),
        disable: target.disable,
        name: target.name.clone(),
        version: target.version.clone(),
        source: target.source.to_string(),
        project_id: target.project_id.clone(),
        file_id: target.file_id.clone(),
        new_file_id: latest.id,
        new_version: latest.version,
        new_file: latest.file,
        date: latest.date,
    }))
}

/// 平台上的一个候选新文件
pub(crate) struct LatestFile {
    /// 文件编号
    pub(crate) id: String,
    /// 版本号；CurseForge 没有这个字段，取文件的显示名
    pub(crate) version: String,
    /// 文件名
    pub(crate) file: String,
    /// 发布时间，ISO 8601
    pub(crate) date: String,
}

/// 取 CurseForge 上最新的那个文件
///
/// - `pid`: 项目编号
/// - `version`: 游戏版本
/// - `loader`: 加载器
///
/// # 返回值
///
/// 见 [`LatestFile`]；当前版本 / 加载器下一个文件都没有时返回 `Ok(None)`
pub(super) async fn latest_curseforge(
    pid: &str,
    version: &str,
    loader: LoaderType,
) -> Result<Option<LatestFile>, String> {
    let list = curseforge_api::get_files_page(CurseFogreArg {
        id: Some(pid.to_string()),
        version: Some(version.to_string()),
        loader: curseforge::to_loader_id(&loader),
        ..Default::default()
    })
    .await
    .map_err(|err| err.to_string())?;

    Ok(newest_curseforge(list.data))
}

/// 在 CurseForge 的文件列表里挑发布时间最晚的那个
///
/// 不直接取第一个：接口没在文档里承诺排序，按 `fileDate` 挑才不赌它。
///
/// - `list`: 接口返回的那一页文件
///
/// # 返回值
///
/// 见 [`LatestFile`]；空列表返回 `None`
pub(crate) fn newest_curseforge(list: Vec<CurseForgeFileDataObj>) -> Option<LatestFile> {
    list.into_iter()
        .max_by(|a, b| a.file_date.cmp(&b.file_date))
        .map(|item| LatestFile {
            id: item.id.to_string(),
            version: item.display_name,
            file: item.file_name,
            date: item.file_date,
        })
}

/// 取 Modrinth 上最新的那个版本
///
/// - `pid`: 项目编号
/// - `version`: 游戏版本
/// - `loader`: 加载器
///
/// # 返回值
///
/// 见 [`LatestFile`]；当前版本 / 加载器下一个版本都没有时返回 `Ok(None)`
pub(super) async fn latest_modrinth(
    pid: &str,
    version: &str,
    loader: LoaderType,
) -> Result<Option<LatestFile>, String> {
    let loader = modrinth::to_loader_id(&loader);
    let list = modrinth_api::get_file_versions(pid, Some(version), loader.as_deref())
        .await
        .map_err(|err| err.to_string())?;

    Ok(newest_modrinth(list))
}

/// 在 Modrinth 的版本列表里挑发布时间最晚的那个
///
/// - `list`: 接口返回的版本列表
///
/// # 返回值
///
/// 见 [`LatestFile`]；空列表返回 `None`，版本里一个文件都没带时文件名是空串
pub(crate) fn newest_modrinth(list: Vec<ModrinthVersionObj>) -> Option<LatestFile> {
    list.into_iter()
        .max_by(|a, b| a.date_published.cmp(&b.date_published))
        .map(|item| {
            // 取 primary 那一份；Modrinth 偶尔给一个主文件都不带的版本，退回第一个
            let file = item
                .files
                .iter()
                .find(|file| file.primary)
                .or_else(|| item.files.first())
                .map(|file| file.filename.clone())
                .unwrap_or_default();
            LatestFile {
                id: item.id,
                version: item.version_number,
                file,
                date: item.date_published,
            }
        })
}
