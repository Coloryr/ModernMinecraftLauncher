//! 资源管理窗口的整合包更新：查平台上的新版本，把实例升上去
//!
//! 升级本体在内核（`mml_game::curseforge::upgrade_modpack` / `modrinth::upgrade_modpack`）：
//! 下新整合包 → 解压覆盖文件 → 比新旧 manifest 算差异 → 删旧文件、下新文件 → 写回 manifest
//! 与实例的 `pid` / `fid`。这里只做两件事：比对实例记着的版本与平台上的最新版，
//! 以及把内核那趟升级挂到共享的整合包任务表上（进度 / 取消 / 终态与「下载整合包」窗口同一套）。
//!
//! **查更新时不按当前游戏版本过滤**：整合包升版常常连游戏版本一起往上走
//! （新版要求 1.21 之类），按实例现在的版本过滤会让这类升级永远查不出来。
//! 因此取项目下发布时间最晚的那份文件，新版要求的游戏版本与加载器由内核升级时写回实例。

use mml_game::GameInstance;
use mml_game::gui_hook::AddModPackGui;
use mml_game::launcher::ModPackType;
use mml_net::curseforge_api::{self, CurseFogreArg};
use mml_net::modrinth_api;
use tauri::AppHandle;
use tokio_util::sync::CancellationToken;

use crate::dtos::ModPackUpdateDto;
use crate::err_keys;
use crate::windows::add_resource::SourceInfo;
use crate::windows::modpack_task::{RegisteredTask, finish_task, register_task};

use super::parse_instance;
use super::update::{LatestFile, newest_curseforge, newest_modrinth};

/// 查整合包有没有新版本
///
/// 实例里记着装的是哪一份（`pid` / `fid`），拿它与平台上发布时间最晚的那份比，
/// 文件号不一样就算有新版本。
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 见 [`ModPackUpdateDto`]；实例不是从平台安装的整合包（本地压缩包导入、手动建的）
/// 或实例里没记项目 / 文件编号时返回 `err.notOnlineModpack`
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_modpack_check(uuid: String) -> Result<ModPackUpdateDto, String> {
    let instance = parse_instance(&uuid)?;
    let (source, pid, fid) = modpack_of(&instance)?;

    let latest = match source {
        "curseforge" => latest_of_curseforge(&pid).await?,
        _ => latest_of_modrinth(&pid).await?,
    };

    let empty = LatestFile {
        id: String::new(),
        version: String::new(),
        file: String::new(),
        date: String::new(),
    };
    let latest = latest.unwrap_or(empty);
    Ok(ModPackUpdateDto {
        has_update: !latest.id.is_empty() && latest.id != fid,
        source: source.to_string(),
        project_id: pid,
        file_id: fid,
        new_file_id: latest.id,
        new_version: latest.version,
        new_file: latest.file,
        date: latest.date,
    })
}

/// 按平台上的新版本升级整合包
///
/// 升级挂在共享的整合包任务表上：命令登记完就返回，进度走 `add-modpack-status` 事件
/// （与「下载整合包」窗口同一套指示器与取消入口），终态会让主窗口刷新并选中这个实例。
///
/// - `app`: 应用句柄，登记任务与广播事件用
/// - `uuid`: 实例 uuid
/// - `file_id`: 要升到的那份文件编号，取检查结果里的 `new_file_id`
///
/// # 返回值
///
/// 任务登记成功即返回 `Ok(())`；实例不是从平台安装的整合包、项目 / 文件编号缺失、
/// 同项目同文件的任务已在跑时返回错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_modpack_update(
    app: AppHandle,
    uuid: String,
    file_id: String,
) -> Result<(), String> {
    let instance = parse_instance(&uuid)?;
    let (source, pid, _) = modpack_of(&instance)?;
    let (name, instance_uuid) = {
        let game = instance.read().unwrap();
        (game.name.clone(), game.uuid)
    };

    let key = SourceInfo {
        pid: pid.clone(),
        fid: file_id.clone(),
    };
    let token = CancellationToken::new();
    let RegisteredTask {
        task,
        gui: pack_gui,
    } = register_task(&app, key, source, name, "upgrade", token.clone())?;
    // 升级的是已有实例，目标从开始就记上：任务列表里点它要能回到那个实例
    *task.instance_uuid.lock().unwrap() = Some(uuid.clone());

    let app_task = app.clone();
    let update_token = token.clone();
    tauri::async_runtime::spawn(async move {
        // 升级 future 非 Send，放阻塞线程上 block_on，与「下载整合包」窗口同一做法
        let res = tauri::async_runtime::spawn_blocking(move || {
            tauri::async_runtime::block_on(upgrade_one(
                instance,
                source,
                &pid,
                &file_id,
                pack_gui,
                update_token,
            ))
        })
        .await
        .unwrap_or_else(|err| Err(err.to_string()));

        finish_task(&app_task, &task, &token, res.map(|_| instance_uuid));
    });

    Ok(())
}

/// 读出实例的整合包身份：来源平台、项目编号、当前装着的那份文件编号
///
/// - `instance`: 目标实例
///
/// # 返回值
///
/// `(来源平台, 项目编号, 文件编号)`；实例不是从平台安装的整合包、或项目 / 文件编号
/// 缺一项时返回 `err.notOnlineModpack`
fn modpack_of(instance: &GameInstance) -> Result<(&'static str, String, String), String> {
    let game = instance.read().unwrap();
    if !game.is_modpack {
        return Err(err_keys::NOT_ONLINE_MODPACK.to_string());
    }
    let pid = game.pid.clone().filter(|id| !id.is_empty());
    let fid = game.fid.clone().filter(|id| !id.is_empty());
    match (game.modpack_type, pid, fid) {
        (ModPackType::CurseForge, Some(pid), Some(fid)) => Ok(("curseforge", pid, fid)),
        (ModPackType::Modrinth, Some(pid), Some(fid)) => Ok(("modrinth", pid, fid)),
        // 本地压缩包导入的实例也是整合包，但没有项目 / 文件编号，问不了平台
        _ => Err(err_keys::NOT_ONLINE_MODPACK.to_string()),
    }
}

/// 取 CurseForge 上项目下发布时间最晚的那份文件
///
/// 不带游戏版本与加载器过滤，理由见文件头。
///
/// - `pid`: 整合包项目编号
///
/// # 返回值
///
/// 见 [`LatestFile`]；接口一个文件都没返回时是 `None`
async fn latest_of_curseforge(pid: &str) -> Result<Option<LatestFile>, String> {
    let list = curseforge_api::get_files_page(CurseFogreArg {
        id: Some(pid.to_string()),
        ..Default::default()
    })
    .await
    .map_err(|err| err.to_string())?;

    Ok(newest_curseforge(list.data))
}

/// 取 Modrinth 上项目下发布时间最晚的那个版本
///
/// 不带游戏版本与加载器过滤，理由见文件头。
///
/// - `pid`: 整合包项目编号
///
/// # 返回值
///
/// 见 [`LatestFile`]；接口一个版本都没返回时是 `None`
async fn latest_of_modrinth(pid: &str) -> Result<Option<LatestFile>, String> {
    let list = modrinth_api::get_file_versions(pid, None, None)
        .await
        .map_err(|err| err.to_string())?;

    Ok(newest_modrinth(list))
}

/// 升级一个整合包实例：问平台要新版本的文件信息，交给内核升级
///
/// - `instance`: 目标实例
/// - `source`: 下载源，取值 `curseforge` / `modrinth`
/// - `pid`: 整合包项目编号
/// - `fid`: 要升到的那份文件编号
/// - `pack_gui`: 升级进度回调
/// - `token`: 取消令牌
///
/// # 返回值
///
/// 成功返回 `Ok(())`；平台查不到文件、下载 / 解包 / 写清单失败返回错误串
async fn upgrade_one(
    instance: GameInstance,
    source: &str,
    pid: &str,
    fid: &str,
    pack_gui: AddModPackGui,
    token: CancellationToken,
) -> Result<(), String> {
    match source {
        "curseforge" => {
            let id: u64 = fid.parse().map_err(|_| err_keys::BAD_FILE_ID.to_string())?;
            let list = curseforge_api::get_files(vec![id])
                .await
                .map_err(|err| err.to_string())?;
            let mut data = list
                .into_iter()
                .next()
                .ok_or_else(|| err_keys::FILE_NOT_FOUND.to_string())?;
            mml_game::curseforge::upgrade_modpack(&instance, &mut data, pack_gui, None, token)
                .await
                .map_err(|err| err.to_string())
        }
        "modrinth" => {
            let mut data = modrinth_api::get_version(pid, fid)
                .await
                .map_err(|err| err.to_string())?;
            mml_game::modrinth::upgrade_modpack(&instance, &mut data, pack_gui, None, token)
                .await
                .map_err(|err| err.to_string())
        }
        _ => Err(err_keys::SOURCE_TYPE.to_string()),
    }
}
