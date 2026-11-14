//! 其他启动器导出包导入
//!
//! MMC、HMCL 与 HMCL 服务器包各自有一套目录约定，逐个还原成实例。

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
    curseforge::pack_obj::CurseForgePackObj,
    game_options,
    gui_hook::{AddInstanceGui, AddModPackGui, AddModPackState},
    launcher::{file_online_info_obj::OnlineInfoObj, instance_setting_obj::CustomLoaderObj},
    other_launcher::{
        hmcl_obj::{HMCLObj, HMCLServerObj},
        mmc_obj::MMCObj,
    },
};

/// 导入 MMC 导出包
///
/// - `file`: 压缩包
/// - `name`: 实例名，覆盖包内元数据里的名字
/// - `group`: 目标分组 uuid，`None` 表示默认分组，不存在或已删的组也会落到默认分组
/// - `unselect`: 不导入的文件列表
/// - `instance_gui`: 实例创建界面回调
/// - `pack_gui`: 安装进度回调
/// - `archive_gui`: 解压进度回调
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 新实例 uuid；路径不存在或 MMC 元数据缺失返回对应错误
pub(super) async fn mmc_archive<P: AsRef<Path>>(
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
    let archive = BaseArchive::open_readonly(&file)?;

    let mut path = String::new();
    let mut mmc = None;
    let mut cfg = None;

    for item in archive.entries() {
        if item.is_dir {
            continue;
        } else if mmc.is_none() && item.name.ends_with(names::MMCJSON_FILE) {
            path = item.name.replace(names::MMCJSON_FILE, "");
            let data = archive.read(&item.name)?;
            let obj = serialize_tools::json_from_bytes::<MMCObj>(&data)?;
            mmc = Some(obj);
        } else if cfg.is_none() && item.name.ends_with(names::MMCCFG_FILE) {
            let data = archive.read_stream(&item.name)?;
            cfg = Some(game_options::read_options(data, Some('='))?);
        }

        if mmc.is_some() && cfg.is_some() {
            break;
        }
    }

    if mmc.is_none() || cfg.is_none() {
        return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
    }

    let mut obj = mmc.unwrap().to_instance(cfg.unwrap());
    if let Some(name) = name {
        obj.name = name;
    }

    if obj.name.is_empty() {
        obj.name = file
            .as_ref()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
    }

    if cancel.is_cancelled() {
        return Err(ErrorType::TaskCancel);
    }

    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::Extract);
        pack_gui.set_now(2, Some(3));
    }

    let game = obj.create_instance_in_group(instance_gui, group).await?;
    let uuid = game.read().unwrap().uuid;

    if cancel.is_cancelled() {
        crate::delete_instance(&uuid)?;
        return Err(ErrorType::TaskCancel);
    }

    // MMC 元数据按完整条目名加入排除列表
    let mut unselect = unselect.unwrap_or_default();
    unselect.push(format!("{path}{}", names::MMCJSON_FILE));
    unselect.push(format!("{path}{}", names::MMCCFG_FILE));

    // 解压到实例基础目录，`.minecraft/` 下的内容自然落到游戏目录 base/.minecraft。
    // mmc-pack.json 所在目录即包裹层，解压时去掉
    let strip_dir = path.trim_end_matches(['/', '\\']).to_string();
    if let Err(err) = super::archive::extract_pack(
        &archive,
        game.read().unwrap().get_base_path(),
        unselect,
        (!strip_dir.is_empty()).then_some(strip_dir),
        &pack_gui,
        archive_gui.clone(),
        cancel.clone(),
    ) {
        // 解压失败 / 取消：半成品实例留着只会被当成导入成功
        let _ = crate::delete_instance(&uuid);
        return Err(err);
    }

    let json = game.read().unwrap().read_custom_json();
    if !json.is_empty() {
        game.write().unwrap().custom_loader = Some(CustomLoaderObj {
            custom_json: true,
            ..Default::default()
        });
        game.read().unwrap().save();
    }

    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::Done);
        pack_gui.set_now(3, Some(3));
    }

    Ok(uuid)
}

/// 导入 HMCL 导出包
///
/// - `file`: 压缩包
/// - `name`: 实例名，覆盖包内元数据里的名字
/// - `group`: 目标分组 uuid，`None` 表示默认分组，不存在或已删的组也会落到默认分组
/// - `unselect`: 不导入的文件列表
/// - `instance_gui`: 实例创建界面回调
/// - `pack_gui`: 安装进度回调
/// - `archive_gui`: 解压进度回调
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 新实例 uuid；路径不存在或 HMCL 元数据缺失返回对应错误
pub(super) async fn hmcl_archive<P: AsRef<Path>>(
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

    let archive = BaseArchive::open_readonly(&file)?;

    let mut path = String::new();
    let mut hmcl = None;
    let mut cfg = None;

    for item in archive.entries() {
        if item.is_dir {
            continue;
        } else if hmcl.is_none() && item.name.ends_with(names::HMCLFILE) {
            path = item.name.replace(names::HMCLFILE, "");
            let data = archive.read(&item.name)?;
            let obj = serialize_tools::json_from_bytes::<HMCLObj>(&data)?;
            hmcl = Some(obj);
        } else if cfg.is_none() && item.name.ends_with(names::MANIFEST_FILE) {
            let data = archive.read(&item.name)?;
            let obj = serialize_tools::json_from_bytes::<CurseForgePackObj>(&data)?;
            cfg = Some(obj);
        }

        if hmcl.is_some() && cfg.is_some() {
            break;
        }
    }

    if hmcl.is_none() || cfg.is_none() {
        return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
    }

    let mut obj = hmcl.unwrap().to_instance();
    if let Some(name) = name {
        obj.name = name;
    }

    if obj.name.is_empty() {
        obj.name = file
            .as_ref()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
    }

    if cancel.is_cancelled() {
        return Err(ErrorType::TaskCancel);
    }

    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::Extract);
        pack_gui.set_now(2, Some(3));
    }

    let game = obj.create_instance_in_group(instance_gui, group).await?;
    let uuid = game.read().unwrap().uuid;

    if cancel.is_cancelled() {
        crate::delete_instance(&uuid)?;
        return Err(ErrorType::TaskCancel);
    }

    // HMCL 元数据按完整条目名加入排除列表
    let mut unselect = unselect.unwrap_or_default();
    unselect.push(format!("{path}{}", names::HMCLFILE));

    let over = if let Some(cfg) = cfg {
        cfg.overrides.clone()
    } else {
        names::OVERRIDE_DIR.to_string()
    };

    let mut strip_dir = path.trim_end_matches(['/', '\\']).to_string();
    let dir = if !strip_dir.is_empty() {
        over
    } else {
        strip_dir.push_str(&over);
        strip_dir
    };

    if let Err(err) = super::archive::extract_pack(
        &archive,
        game.read().unwrap().get_base_path(),
        unselect,
        Some(dir),
        &pack_gui,
        archive_gui.clone(),
        cancel.clone(),
    ) {
        // 解压失败 / 取消：半成品实例留着只会被当成导入成功
        let _ = crate::delete_instance(&uuid);
        return Err(err);
    }

    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::Done);
        pack_gui.set_now(3, Some(3));
    }

    Ok(uuid)
}

/// 导入 HMCL 服务器包，并转成本启动器的 MML 服务器实例
///
/// 转换后的清单写进实例的 `server.json`，清单里带地址的文件按 MML 同步流程下载；
/// HMCL 的 `server-manifest.json` 不留在实例目录里。
///
/// - `file`: 压缩包
/// - `name`: 实例名，覆盖包内元数据里的名字
/// - `group`: 目标分组 uuid，`None` 表示默认分组，不存在或已删的组也会落到默认分组
/// - `unselect`: 不导入的文件列表
/// - `instance_gui`: 实例创建界面回调
/// - `pack_gui`: 安装进度回调
/// - `archive_gui`: 解压进度回调
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 新实例 uuid；路径不存在或服务器清单缺失返回对应错误
pub(super) async fn hmcl_server_archive<P: AsRef<Path>>(
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

    let archive = BaseArchive::open_readonly(&file)?;

    let mut path = String::new();
    let mut hmcl = None;

    for item in archive.entries() {
        if item.is_dir {
            continue;
        } else if hmcl.is_none() && item.name.ends_with(names::SERVER_MANIFEST_FILE) {
            path = item.name.replace(names::SERVER_MANIFEST_FILE, "");
            let data = archive.read(&item.name)?;
            let obj = serialize_tools::json_from_bytes::<HMCLServerObj>(&data)?;
            hmcl = Some(obj);
            break;
        }
    }

    if hmcl.is_none() {
        return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
    }

    let hmcl = hmcl.unwrap();

    let mut obj = hmcl.to_instance();
    if let Some(name) = name {
        obj.name = name;
    }

    if obj.name.is_empty() {
        obj.name = file
            .as_ref()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
    }

    if cancel.is_cancelled() {
        return Err(ErrorType::TaskCancel);
    }

    // 先把 HMCL 清单转成 MML 清单，实例建好后写进 server.json
    let mut pack = hmcl.to_server_pack();
    pack.name = obj.name.clone();

    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::Extract);
        pack_gui.set_now(2, Some(3));
    }

    let game = obj.create_instance_in_group(instance_gui, group).await?;
    let uuid = game.read().unwrap().uuid;

    if cancel.is_cancelled() {
        crate::delete_instance(&uuid)?;
        return Err(ErrorType::TaskCancel);
    }

    // HMCL 元数据按完整条目名加入排除列表；服务器清单也不留在实例里
    let mut unselect = unselect.unwrap_or_default();
    unselect.push(format!("{path}{}", names::HMCLFILE));
    unselect.push(format!("{path}{}", names::SERVER_MANIFEST_FILE));

    let over = names::OVERRIDE_DIR.to_string();
    let mut strip_dir = path.trim_end_matches(['/', '\\']).to_string();
    let dir = if !strip_dir.is_empty() {
        over
    } else {
        strip_dir.push_str(&over);
        strip_dir
    };

    if let Err(err) = super::archive::extract_pack(
        &archive,
        game.read().unwrap().get_base_path(),
        unselect,
        Some(dir),
        &pack_gui,
        archive_gui.clone(),
        cancel.clone(),
    ) {
        // 解压失败 / 取消：半成品实例留着只会被当成导入成功
        let _ = crate::delete_instance(&uuid);
        return Err(err);
    }

    // 清单里带下载地址的文件按 MML 的同步流程拉下来，同时把 server.json 落进实例；
    // fileApi 为空时只落清单，文件靠 overrides 里带的那份
    let online_items: Vec<(String, OnlineInfoObj)> = pack
        .online_list
        .iter()
        .filter_map(|item| {
            let url = item.url.clone()?;
            let path = Path::new(&item.file);
            let name = path.file_name()?.to_string_lossy().to_string();
            let dir = path
                .parent()
                .map(|parent| parent.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            let key = item
                .sha1
                .clone()
                .filter(|sha1| !sha1.is_empty())
                .unwrap_or_else(|| item.file.clone());
            Some((
                key,
                OnlineInfoObj {
                    modid: String::new(),
                    fileid: String::new(),
                    path: dir,
                    name: name.clone(),
                    file: name,
                    sha1: item.sha1.clone().unwrap_or_default(),
                    url,
                },
            ))
        })
        .collect();

    let has_online = pack
        .online_list
        .iter()
        .any(|item| item.url.as_deref().is_some_and(|url| !url.is_empty()));
    if has_online {
        let total = pack.online_list.len() + pack.archive_list.len();
        if let Some(pack_gui) = &pack_gui {
            pack_gui.set_state(AddModPackState::CheckFile);
            pack_gui.set_now(0, Some(total));
        }
        let progress_gui = pack_gui.clone();
        let report = move |done: usize, all: usize| {
            if let Some(gui) = &progress_gui {
                gui.set_now(done, Some(all));
            }
        };
        let phase_gui = pack_gui.clone();
        let phase = move |state: AddModPackState| {
            if let Some(gui) = &phase_gui {
                gui.set_state(state);
            }
        };
        let snapshot = game.read().unwrap().clone();
        if let Err(err) = snapshot
            .upgrade_serverpack_with(pack, cancel.clone(), Some(&report), Some(&phase), &[])
            .await
        {
            // 半成品实例留着只会让人以为导入成功；与在线实例导入同一处理
            let _ = crate::delete_instance(&uuid);
            return Err(err);
        }
    } else {
        // 导入返回后马上会读清单，这里同步写，别排进后台队列
        game.read().unwrap().save_serverpack_sync(&pack)?;
    }

    // HMCL 的下载地址没有项目号，走不了平台来源；记进在线信息，
    // 重新生成 MML 服务器实例时还能按地址直接下发，不必再复制一份文件
    if !online_items.is_empty() {
        let mut online = game.read().unwrap().read_online_info();
        for (key, item) in online_items {
            online.insert(key, item);
        }
        game.read().unwrap().save_online_info(&online);
    }

    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::Done);
        pack_gui.set_now(3, Some(3));
    }

    Ok(uuid)
}
