//! 安装入口
//!
//! 按识别出的包类型分发到各格式的导入实现，本地文件与在线地址共用。

use super::detect::PackType;

use std::path::Path;

use mml_base::{
    archives::BaseArchiveGui,
    file_item::{FileHash, FileItemObj, LaterRun},
};
use mml_names::{
    i18,
    i18_items::{
        error_type::{CoreResult, ErrorType},
        info_type::InfoType,
    },
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    gui_hook::{AddInstanceGui, AddModPackGui},
    launcher::ModPackType,
};

/// 本地压缩包没有项目 / 文件 ID 时，把整合包类型清成「无」
///
/// 类型下拉只对能回查在线项目的包有意义；本地包留着 CurseForge / Modrinth 会让
/// 「检查更新」指向不存在的项目。`is_modpack` 保持原样，启动与模组检查照旧。
/// 服务器实例例外：它本来就没有平台项目号，类型留着才能继续同步服务器包。
///
/// - `uuid`: 刚安装好的实例 uuid
fn clear_modpack_type_without_ids(uuid: &Uuid) {
    let Some(instance) = crate::get_instance(uuid) else {
        return;
    };
    let mut game = instance.write().unwrap();
    if game.pid.is_some() && game.fid.is_some() {
        return;
    }
    // 服务器实例本来就没有平台项目号：它的类型要留着，否则导入后不能继续同步
    if matches!(
        game.modpack_type,
        ModPackType::None | ModPackType::ServerPack
    ) {
        return;
    }
    game.modpack_type = ModPackType::None;
    game.save();
}

/// 从文件路径安装压缩包
///
/// - `file`: 压缩包
/// - `name`: 实例名，覆盖包内元数据里的名字
/// - `group`: 目标分组 uuid，`None` 表示默认分组，不存在或已删的组也会落到默认分组
/// - `unselect`: 不导入的文件列表
/// - `instance_gui`: 实例创建界面回调
/// - `pack_gui`: 安装进度回调
/// - `archive_gui`: 解压进度回调
/// - `pack_type`: 压缩包类型
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 新实例 uuid；路径不存在或元数据缺失返回对应错误
pub async fn install_archive_from_file<P: AsRef<Path>>(
    file: P,
    name: Option<String>,
    group: Option<Uuid>,
    unselect: Option<Vec<String>>,
    instance_gui: AddInstanceGui,
    pack_gui: AddModPackGui,
    archive_gui: BaseArchiveGui,
    pack_type: PackType,
    cancel: CancellationToken,
) -> CoreResult<Uuid> {
    let uuid = match pack_type {
        PackType::CurseForge => {
            super::modpack::modpack(
                file,
                ModPackType::CurseForge,
                name,
                group,
                unselect,
                instance_gui,
                pack_gui,
                archive_gui,
                cancel,
                None, // 本地压缩包没有图标 URL
            )
            .await
        }
        PackType::Modrinth => {
            super::modpack::modpack(
                file,
                ModPackType::Modrinth,
                name,
                group,
                unselect,
                instance_gui,
                pack_gui,
                archive_gui,
                cancel,
                None, // 本地压缩包没有图标 URL
            )
            .await
        }
        PackType::MMC => {
            super::launcher_archive::mmc_archive(
                file,
                name,
                group,
                unselect,
                instance_gui,
                pack_gui,
                archive_gui,
                cancel,
            )
            .await
        }
        PackType::HMCL => {
            super::launcher_archive::hmcl_archive(
                file,
                name,
                group,
                unselect,
                instance_gui,
                pack_gui,
                archive_gui,
                cancel,
            )
            .await
        }
        PackType::HMCLServer => {
            super::launcher_archive::hmcl_server_archive(
                file,
                name,
                group,
                unselect,
                instance_gui,
                pack_gui,
                archive_gui,
                cancel,
            )
            .await
        }
        PackType::ArchivePack => {
            super::archive::archive(
                file,
                name,
                group,
                unselect,
                instance_gui,
                pack_gui,
                archive_gui,
                cancel,
            )
            .await
        }
        PackType::LauncherPack => {
            super::launcher_pack::launcher_pack(
                file,
                name,
                group,
                unselect,
                instance_gui,
                pack_gui,
                archive_gui,
                cancel,
            )
            .await
        }
    }?;

    // 本地包没有项目 / 文件 ID 时，类型留「无」：否则界面上会显示成可更新平台
    clear_modpack_type_without_ids(&uuid);
    Ok(uuid)
}

/// 从在线网址安装整合包
///
/// - `url`: 压缩包下载地址
/// - `name`: 实例名，覆盖包内元数据里的名字
/// - `group`: 目标分组 uuid，`None` 表示默认分组，不存在或已删的组也会落到默认分组
/// - `unselect`: 不导入的文件列表
/// - `instance_gui`: 实例创建界面回调
/// - `pack_gui`: 安装进度回调
/// - `archive_gui`: 解压进度回调
/// - `pack_type`: 压缩包类型
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 新实例 uuid；下载失败或安装失败返回对应错误
pub async fn install_archive_from_url(
    url: &str,
    name: Option<String>,
    group: Option<Uuid>,
    unselect: Option<Vec<String>>,
    instance_gui: AddInstanceGui,
    pack_gui: AddModPackGui,
    archive_gui: BaseArchiveGui,
    pack_type: PackType,
    cancel: CancellationToken,
) -> CoreResult<Uuid> {
    let file = mml_downloader::gen_temp_file();

    let res = mml_downloader::start_download_task(vec![FileItemObj {
        name: i18::get_info(InfoType::TempFile),
        file: file.clone(),
        url: url.to_string(),
        hash: FileHash::None,
        later: LaterRun::None,
    }])
    .await;

    if !res {
        return Err(ErrorType::DownloadFileFail);
    }

    install_archive_from_file(
        file,
        name,
        group,
        unselect,
        instance_gui,
        pack_gui,
        archive_gui,
        pack_type,
        cancel,
    )
    .await
}
