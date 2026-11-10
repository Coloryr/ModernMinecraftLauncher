//! 在线实例包
//!
//! 读 `server.json` 清单，下载整合包与服务端文件并安装。

use super::SERVER_PACK_SUFFIX;

use std::path::Path;

use mml_base::serialize_tools;
use mml_names::{
    i18_items::error_type::{CoreResult, DataNotFoundData, ErrorType},
    names,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    gui_hook::{AddInstanceGui, AddModPackGui, AddModPackState},
    launcher::{ModPackType, instance_setting_obj::InstanceSettingObj},
    serverpack::serverpack_obj::ServerPackObj,
};

/// 把用户填的在线实例地址规整成 `server.json` 的下载地址
///
/// 前端填的可能是发布目录，也可能直接是清单文件：末尾没有 `server.json` 就补上。
///
/// - `value`: 用户填的地址
///
/// # 返回值
///
/// 以 `server.json` 结尾的地址；空地址返回 `DataNotFound(Url)`
pub fn normalize_server_pack_url(value: &str) -> CoreResult<String> {
    let url = value.trim().trim_end_matches('/');
    if url.is_empty() {
        return Err(ErrorType::DataNotFound(DataNotFoundData::Url));
    }
    if url.to_ascii_lowercase().ends_with(SERVER_PACK_SUFFIX) {
        return Ok(url.to_string());
    }
    Ok(format!("{url}/{}", names::SERVER_FILE))
}

/// 从清单地址取实例里要记的服务器地址
///
/// 启动时的 `server_pack_update` 自己拼 `<地址>/server.json` 与 `.sha1`，
/// 所以实例里存的是**目录**，不带清单文件名。
///
/// - `manifest`: 已经规整过的清单地址
///
/// # 返回值
///
/// 发布目录地址，末尾不带斜杠
pub fn server_pack_base_url(manifest: &str) -> String {
    if !manifest.to_ascii_lowercase().ends_with(SERVER_PACK_SUFFIX) {
        return manifest.to_string();
    }
    manifest[..manifest.len() - SERVER_PACK_SUFFIX.len()].to_string()
}

/// 下载并解析在线服务器包清单
///
/// 添加窗口填完地址后的预览与真正导入共用这一趟：地址补齐、下载、解析只写一份。
///
/// - `url`: 用户填的地址，发布目录或 `server.json` 都认
///
/// # 返回值
///
/// 实际读取的清单地址（已补 `server.json`）与解析出的清单
pub async fn fetch_server_pack(url: &str) -> CoreResult<(String, ServerPackObj)> {
    let manifest_url = normalize_server_pack_url(url)?;
    let data = mml_net::get_work_client().get_bytes(&manifest_url).await?;
    let pack = serialize_tools::json_from_bytes::<ServerPackObj>(&data)?;
    Ok((manifest_url, pack))
}

/// 从在线服务器包创建实例，并把包内容同步下来
///
/// 下载 `server.json` 读出游戏版本与加载器，登记成「在线网络整合包」后立刻同步一次：
/// 模组、配置与图标在导入时就落地，玩家拿到实例可以直接启动，不必等第一次启动再下。
///
/// - `url`: 用户填的地址，发布目录或 `server.json` 都认
/// - `name`: 实例名，覆盖清单里的名字
/// - `group`: 目标分组 uuid，`None` 表示默认分组
/// - `instance_gui`: 添加实例界面回调
/// - `pack_gui`: 安装进度回调，同步下载时用它显示进度
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 新实例 uuid；地址为空、清单解析失败、下载或解压失败返回对应错误，
/// 失败时已建的半成品实例会被删掉
pub async fn install_server_pack_from_url(
    url: &str,
    name: Option<String>,
    group: Option<Uuid>,
    instance_gui: AddInstanceGui,
    pack_gui: AddModPackGui,
    cancel: CancellationToken,
) -> CoreResult<Uuid> {
    install_server_pack_from_url_with_skip(url, name, group, &[], instance_gui, pack_gui, cancel)
        .await
}

/// 从在线服务器包创建实例，可选模组只下载其中一部分
///
/// 与 [`install_server_pack_from_url`] 同一条链路，多一个 `skip`：首次同步时玩家没勾的
/// 可选模组这次不下载，本地旧文件保持原样。
///
/// - `url`: 用户填的地址，发布目录或 `server.json` 都认
/// - `name`: 实例名，覆盖清单里的名字
/// - `group`: 目标分组 uuid，`None` 表示默认分组
/// - `skip`: 这次先不下载的可选模组路径，取清单里的相对路径
/// - `instance_gui`: 添加实例界面回调
/// - `pack_gui`: 安装进度回调
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 新实例 uuid；失败时已建的半成品实例会被删掉
pub async fn install_server_pack_from_url_with_skip(
    url: &str,
    name: Option<String>,
    group: Option<Uuid>,
    skip: &[String],
    instance_gui: AddInstanceGui,
    pack_gui: AddModPackGui,
    cancel: CancellationToken,
) -> CoreResult<Uuid> {
    // 先拉远端清单：几千上万条目的包光下载清单就要一会儿，先报"检查服务器实例信息"，
    // 别让界面停在还没开始的对比上
    if let Some(gui) = &pack_gui {
        gui.set_state(AddModPackState::CheckServerPack);
        gui.set_now(0, None);
    }
    let (manifest_url, pack) = fetch_server_pack(url).await?;
    let base_url = server_pack_base_url(&manifest_url);

    if cancel.is_cancelled() {
        return Err(ErrorType::TaskCancel);
    }

    let obj = InstanceSettingObj {
        name: name.unwrap_or(pack.name.clone()),
        version: pack.version.clone(),
        loader: pack.loader,
        loader_version: pack.loader_version.clone(),
        is_modpack: true,
        modpack_type: ModPackType::ServerPack,
        server_url: Some(base_url.clone()),
        ..Default::default()
    };
    let game = obj.create_instance_in_group(instance_gui, group).await?;
    let uuid = game.read().unwrap().uuid;
    let snapshot = game.read().unwrap().clone();

    let total = pack.online_list.len() + pack.archive_list.len();
    if let Some(gui) = &pack_gui {
        gui.set_state(AddModPackState::CheckFile);
        gui.set_now(0, Some(total));
    }

    // 图标名先取出来：清单本体要整个交给同步流程
    let icon = pack.icon.clone();
    // 进度推到添加窗口的弹窗上：整批一个下载任务，完成数从任务表同步
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
    if let Err(e) = snapshot
        .upgrade_serverpack_with(pack, cancel, Some(&report), Some(&phase), skip)
        .await
    {
        // 半成品实例留着只会让人以为导入成功，删掉重来更干净
        let _ = crate::delete_instance(&uuid);
        return Err(e);
    }

    // 图标只是外观，下不下来都不影响实例可用
    download_server_pack_icon(&uuid, &snapshot, &base_url, icon.as_deref()).await;

    if let Some(gui) = &pack_gui {
        gui.set_state(AddModPackState::Done);
        gui.set_now(total, Some(total));
    }

    Ok(uuid)
}

/// 按发布地址重新同步一个在线服务器包实例
///
/// 启动时的服务器包检查与「实例设置」里的升级按钮走同一条链路：拉清单 → 与本地清单比对 →
/// 删旧文件、下新文件、重解压配置目录 → 回写在线信息与实例的版本 / 加载器。
///
/// - `game`: 目标实例的设置快照
/// - `gui`: 安装进度回调，`None` 表示不显示进度
/// - `cancel`: 取消令牌
/// - `skip`: 这次先不更新的文件，取新清单里的相对路径；空表示全部更新
///
/// # 返回值
///
/// 同步成功返回 `Ok(())`；实例没有发布地址、清单读取失败、下载或解压失败返回对应错误
pub async fn upgrade_server_pack(
    game: &InstanceSettingObj,
    gui: AddModPackGui,
    cancel: CancellationToken,
    skip: &[String],
) -> CoreResult<()> {
    let url = game
        .server_url
        .clone()
        .filter(|item| !item.trim().is_empty())
        .ok_or(ErrorType::DataNotFound(DataNotFoundData::Info))?;

    // 先拉远端清单，再谈对比：几万个条目的包光下载清单就要一会儿
    if let Some(gui) = &gui {
        gui.set_state(AddModPackState::CheckServerPack);
        gui.set_now(0, None);
    }
    let (manifest_url, pack) = fetch_server_pack(&url).await?;
    // 图标名先取出来：清单本体要整个交给同步流程
    let icon = pack.icon.clone();

    let total = pack.online_list.len() + pack.archive_list.len();
    // 版本号没变就不必重下图标
    let same_version = game.same_pack_version(&pack.pack_version);
    if let Some(gui) = &gui {
        gui.set_state(AddModPackState::CheckFile);
        gui.set_now(0, Some(total));
    }

    let progress_gui = gui.clone();
    let report = move |done: usize, all: usize| {
        if let Some(gui) = &progress_gui {
            gui.set_now(done, Some(all));
        }
    };
    let phase_gui = gui.clone();
    let phase = move |state: AddModPackState| {
        if let Some(gui) = &phase_gui {
            gui.set_state(state);
        }
    };
    // 旧清单先挪成 `.old`：升级时按新旧两份清单算该删什么，与启动时那条链路一致
    game.move_serverpack_to_old()?;
    if let Err(err) = game
        .upgrade_serverpack_with(pack, cancel, Some(&report), Some(&phase), skip)
        .await
    {
        // 同步半途失败时清单已经被挪走：挪回来，别让实例从此没有清单可查
        let _ = game.restore_serverpack_from_old();
        return Err(err);
    }

    // 图标会随清单换，按新清单再取一次；下不下来都不影响实例可用
    if !same_version {
        let base = server_pack_base_url(&manifest_url);
        download_server_pack_icon(&game.uuid, game, &base, icon.as_deref()).await;
    }

    if let Some(gui) = &gui {
        gui.set_state(AddModPackState::Done);
        gui.set_now(total, Some(total));
    }

    Ok(())
}

/// 下载服务器包里的图标并写进实例目录
///
/// 清单里的 `Icon` 是发布目录下的文件名：取 `<发布地址>/<文件名>` 存成实例的
/// `icon.png`，与玩家自己换图标走同一条路（实例图标只认本地文件）。
///
/// - `uuid`: 实例 uuid
/// - `game`: 实例设置快照，取实例目录用
/// - `base`: 发布地址
/// - `icon`: 清单里的图标文件名；空表示没有
async fn download_server_pack_icon(
    uuid: &Uuid,
    game: &InstanceSettingObj,
    base: &str,
    icon: Option<&str>,
) {
    let Some(name) = icon.map(str::trim).filter(|item| !item.is_empty()) else {
        return;
    };
    // 只认文件名：清单里的 `../` 不能让它写到发布目录外面去
    let Some(file) = Path::new(name).file_name().and_then(|item| item.to_str()) else {
        return;
    };
    let url = format!("{base}/{file}");
    let Ok(bytes) = mml_net::get_work_client().get_bytes(&url).await else {
        return;
    };
    if std::fs::write(game.get_base_path().join(names::ICON_FILE), &bytes).is_err() {
        return;
    }
    if let Some(instance) = crate::get_instance(uuid) {
        let mut write = instance.write().unwrap();
        write.icon = Some(names::ICON_FILE.to_string());
        write.save();
    }
}
