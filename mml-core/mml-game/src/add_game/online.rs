//! 在线平台整合包安装
//!
//! Modrinth 与 CurseForge 的在线安装入口，下载交给 `super::modpack`。

use mml_base::archives::BaseArchiveGui;
use mml_names::i18_items::error_type::{CoreResult, ErrorType};
use mml_net::{
    curseforge_api::file_obj::CurseForgeFileDataObj, modrinth_api::version_obj::ModrinthVersionObj,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    curseforge,
    gui_hook::{AddInstanceGui, AddModPackGui},
    launcher::ModPackType,
    modrinth,
};

/// 安装 Modrinth 在线整合包
///
/// - `data`: Modrinth 版本信息
/// - `group`: 目标分组 uuid，`None` 表示默认分组，不存在或已删的组也会落到默认分组
/// - `icon`: 整合包图标 URL，交给安装器，创建实例时写入；下载失败只记日志，不影响安装
/// - `instance_gui`: 实例创建界面回调
/// - `pack_gui`: 安装进度回调
/// - `archive_gui`: 解压进度回调
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 新实例 uuid；下载或安装失败返回对应错误
pub async fn install_modrinth(
    data: &ModrinthVersionObj,
    group: Option<Uuid>,
    icon: Option<String>,
    instance_gui: AddInstanceGui,
    pack_gui: AddModPackGui,
    archive_gui: BaseArchiveGui,
    cancel: CancellationToken,
) -> CoreResult<Uuid> {
    let item = modrinth::make_download_obj(data, mml_downloader::get_download_path());
    let file = item.file.clone();

    let outcome =
        super::modpack::download_with_progress(
            vec![item],
            pack_gui.clone(),
            cancel.clone(),
            super::modpack::DownloadProgressUnit::Bytes,
        )
        .await;
    if !outcome.is_done() {
        return Err(ErrorType::DownloadFileFail);
    }

    // 直接走 modpack，不走 install_archive_from_file：图标要一路塞给 worker，
    // 让它在创建实例时一起写，那个 7 臂分发器吃不下这个参数，其中 5 个臂也用不到
    let uuid = super::modpack::modpack(
        file,
        ModPackType::Modrinth,
        None,
        group,
        None,
        instance_gui,
        pack_gui,
        archive_gui,
        cancel,
        icon,
    )
    .await?;

    if let Some(instance) = crate::get_instance(&uuid) {
        let mut write = instance.write().unwrap();
        write.pid = Some(data.project_id.clone());
        write.fid = Some(data.id.clone());
        write.save();
    }

    Ok(uuid)
}

/// 安装 CurseForge 在线整合包
///
/// - `data`: CurseForge 文件信息，安装成功后回填项目与文件 ID
/// - `group`: 目标分组 uuid，`None` 表示默认分组，不存在或已删的组也会落到默认分组
/// - `icon`: 整合包图标 URL，交给安装器，创建实例时写入；下载失败只记日志，不影响安装
/// - `instance_gui`: 实例创建界面回调
/// - `pack_gui`: 安装进度回调
/// - `archive_gui`: 解压进度回调
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 新实例 uuid；下载或安装失败返回对应错误
pub async fn install_curseforge(
    data: &mut CurseForgeFileDataObj,
    group: Option<Uuid>,
    icon: Option<String>,
    instance_gui: AddInstanceGui,
    pack_gui: AddModPackGui,
    archive_gui: BaseArchiveGui,
    cancel: CancellationToken,
) -> CoreResult<Uuid> {
    let item = curseforge::make_file_item_obj(data, mml_downloader::get_download_path());
    let file = item.file.clone();

    let outcome =
        super::modpack::download_with_progress(
            vec![item],
            pack_gui.clone(),
            cancel.clone(),
            super::modpack::DownloadProgressUnit::Bytes,
        )
        .await;
    if !outcome.is_done() {
        return Err(ErrorType::DownloadFileFail);
    }

    // 直接走 modpack，不走 install_archive_from_file：理由同 install_modrinth
    let uuid = super::modpack::modpack(
        file,
        ModPackType::CurseForge,
        None,
        group,
        None,
        instance_gui,
        pack_gui,
        archive_gui,
        cancel,
        icon,
    )
    .await?;

    if let Some(instance) = crate::get_instance(&uuid) {
        let mut write = instance.write().unwrap();
        write.pid = Some(data.mod_id.to_string());
        write.fid = Some(data.id.to_string());
        write.save();
    }

    Ok(uuid)
}
