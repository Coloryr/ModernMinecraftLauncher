//! 普通压缩包导入
//!
//! 没有元数据的压缩包按「整份解压进实例目录」处理。

use super::path_not_exists;

use std::path::Path;

use mml_base::{
    archives::{BaseArchive, BaseArchiveGui},
    serialize_tools,
};
use mml_names::{
    i18_items::error_type::{CoreResult, DataNotFoundData, ErrorType},
    names,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    gui_hook::{AddInstanceGui, AddModPackGui, AddModPackState},
    launcher::instance_setting_obj::InstanceSettingObj,
    modpack::pack_extract_gui,
};

/// 解压整合包压缩包到实例目录，统一处理压缩包整体套一层文件夹的情况
///
/// - `archive`: 已打开的压缩包
/// - `output_dir`: 解压目标目录，实例基础目录
/// - `unselect`: 按压缩包内完整条目名排除的文件列表
/// - `strip_dir`: 压缩包整体套的顶层目录名，不剥时传 `None`
/// - `pack_gui`: 安装进度回调，逐条解压进度写进它的子进度
/// - `archive_gui`: 可选的压缩包弹窗回调
///
/// # 返回值
///
/// 成功返回 `Ok(())`；解压失败返回对应错误
pub(super) fn extract_pack<P: AsRef<Path>>(
    archive: &BaseArchive,
    output_dir: P,
    unselect: Vec<String>,
    strip_dir: Option<String>,
    pack_gui: &AddModPackGui,
    archive_gui: BaseArchiveGui,
) -> CoreResult<()> {
    // 桥由内核统一挂：只等调用方传 archive_gui 的话，漏挂一处整段解压就没有
    // 文件名与子进度条。安装弹窗与压缩包弹窗拿的是同一份逐条进度
    let gui = pack_extract_gui(pack_gui.clone(), archive_gui);
    archive.extract_all(output_dir, Some(unselect), strip_dir, gui)
}

/// 直接解压
///
/// - `file`: 压缩包
/// - `name`: 实例名，覆盖 `game.json` 里的名字
///
/// 压缩包内是本启动器导出的实例包，含 `game.json`。
/// - `group`: 目标分组 uuid，`None` 表示默认分组，不存在或已删的组也会落到默认分组
/// - `unselect`: 不导入的文件列表
/// - `instance_gui`: 实例创建界面回调
/// - `pack_gui`: 安装进度回调
/// - `archive_gui`: 解压进度回调
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 新实例 uuid；路径不存在或 `game.json` 缺失返回对应错误
pub(super) async fn archive<P: AsRef<Path>>(
    file: P,
    name: Option<String>,
    group: Option<Uuid>,
    unselect: Option<Vec<String>>,
    instance_gui: AddInstanceGui,
    pack_gui: AddModPackGui,
    archive_gui: BaseArchiveGui,
    cancel: CancellationToken,
) -> CoreResult<Uuid> {
    if !file.as_ref().exists() || file.as_ref().is_dir() {
        return Err(path_not_exists(file.as_ref().to_path_buf()));
    }

    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::ReadInfo);
        pack_gui.set_now(1, Some(3));
    }

    let archive = BaseArchive::open_readonly(file)?;
    // 找到 game.json 所在条目，兼容压缩包整体套一层文件夹的情况
    let game_entry = archive
        .entries()
        .iter()
        .find(|e| !e.is_dir && e.name.ends_with(names::GAME_FILE))
        .ok_or_else(|| ErrorType::DataNotFound(DataNotFoundData::Info))?;
    let data = archive.read(&game_entry.name)?;
    let mut obj = serialize_tools::json_from_bytes::<InstanceSettingObj>(&data)?;

    if let Some(name) = name {
        obj.name = name;
    }

    let game = obj.create_instance_in_group(instance_gui, group).await?;
    let uuid = game.read().unwrap().uuid;
    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::Extract);
        pack_gui.set_now(2, Some(3));
    }

    if cancel.is_cancelled() {
        crate::delete_instance(&uuid)?;
        return Err(ErrorType::TaskCancel);
    }

    // game.json 所在目录即实例包裹目录，解压时去掉该层，直接放进实例目录
    let strip_dir = game_entry
        .name
        .strip_suffix(names::GAME_FILE)
        .filter(|s| !s.is_empty())
        .map(|s| s.trim_end_matches(['/', '\\']).to_string());

    // 包内的 game.json 是导出时的旧元数据，旧 uuid；create_instance 已保存
    // 新实例的 game.json，不能被解压覆盖
    let mut unselect = unselect.unwrap_or_default();
    unselect.push(game_entry.name.clone());

    extract_pack(
        &archive,
        game.read().unwrap().get_base_path(),
        unselect,
        strip_dir,
        &pack_gui,
        archive_gui,
    )?;

    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::Done);
        pack_gui.set_now(3, Some(3));
    }

    Ok(uuid)
}
