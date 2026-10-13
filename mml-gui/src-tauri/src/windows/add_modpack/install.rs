//! 下载整合包窗口的安装执行：发起 / 取消 / 状态 / 清理终态
//!
//! 从 `add_modpack/mod.rs` 拆出来的，光 `add_modpack_install` 就有 104 行。安装走内核
//! `mml_game::add_game`，任务登记在共享的 `modpack_task` 注册表里，与窗口生命周期解耦。
//! 命令带分组标记，值填原始组名 add_modpack，把组键钉回 `addModpack`，见 AGENTS.md §4。

use std::sync::atomic::Ordering;

use mml_game::add_game;
use mml_game::launcher::ModPackType;
use mml_net::{curseforge_api, modrinth_api};
use tauri::{AppHandle, WebviewWindow};
use tokio_util::sync::CancellationToken;

use crate::dtos::add_modpack_dto::ModPackStatusDto;
use crate::err_keys;
use crate::windows::add::instance_gui;
use crate::windows::add_resource::SourceInfo;
use crate::windows::modpack_task::{
    DOWNLOAD_NOW, RegisteredTask, build_status, emit_add_modpack_status, finish_task, register_task,
};

use super::{CURSEFOGRE_INFO, MODRINTH_INFO};

/// 安装在线整合包，多任务：命令立即返回，任务在后台跑，进度走 `add-modpack-status` 事件
///
/// 实例名与图标取自列表缓存，缓存没有时退回项目 ID 作名字、图标留空。
///
/// - `window`: 调用方窗口，用于取本窗口模型做重名确认
/// - `app`: 应用句柄，用于登记任务并发状态事件
/// - `source`: 下载源标识，取值 `curseforge` / `modrinth`
/// - `project_id`: 项目 ID
/// - `file_id`: 文件 ID；CurseForge 是数字字符串，Modrinth 是版本 ID
/// - `group`: 分组 uuid；空 / 非法 = 默认分组
///
/// # 返回值
///
/// 任务登记成功即返回 `Ok(())`，不等安装结束；下载源不支持、同 pid+fid 已在安装返回 `Err`
#[gui_macros::ipc_group("add_modpack")]
#[tauri::command]
pub async fn add_modpack_install(
    window: WebviewWindow,
    app: AppHandle,
    source: String,
    project_id: String,
    file_id: String,
    group: Option<String>,
) -> Result<(), String> {
    // 前端传的是分组 uuid；空 / 非法 = 默认分组
    let group = crate::windows::parse_group_id(group);

    let source_type = ModPackType::from_string(&source);
    if !matches!(source_type, ModPackType::CurseForge | ModPackType::Modrinth) {
        return Err(String::from(err_keys::SOURCE_TYPE));
    }

    let key = SourceInfo {
        pid: project_id.clone(),
        fid: file_id.clone(),
    };

    // 显示名与图标都从列表缓存取，列表页已缓存过项目元数据。
    // 单窗口模式下本页会被 KeepAlive 缓存，缓存一直在；万一没了，比如多窗口下直接开安装命令、
    // 或窗口关了再装，退回项目 ID 作名字、图标留空 —— 都只是显示层面的事，不影响安装
    let (name, icon) = match source_type {
        ModPackType::CurseForge => {
            let cache = CURSEFOGRE_INFO.read().await;
            match cache.get(&project_id) {
                Some(item) => (item.name.clone(), item.logo.url.clone()),
                None => (project_id.clone(), None),
            }
        }
        ModPackType::Modrinth => {
            let cache = MODRINTH_INFO.read().await;
            match cache.get(&project_id) {
                Some(item) => (item.title.clone(), item.icon_url.clone()),
                None => (project_id.clone(), None),
            }
        }
        _ => (project_id.clone(), None),
    };

    // 登记任务：同 pid+fid 不允许重复安装，进度回调与任务一起给出
    let token = CancellationToken::new();
    let RegisteredTask {
        task,
        gui: pack_gui,
    } = register_task(&app, key, &source, name, token.clone())?;

    let gui = instance_gui(&window);

    // 后台安装：命令不等安装结束，安装 future 非 Send，放阻塞线程上 block_on
    let app_task = app.clone();
    let install_token = token.clone();
    tauri::async_runtime::spawn(async move {
        let res = tauri::async_runtime::spawn_blocking(move || {
            tauri::async_runtime::block_on(async {
                match source.as_str() {
                    "curseforge" => {
                        let fid: u64 = file_id
                            .parse()
                            .map_err(|_| err_keys::BAD_FILE_ID.to_string())?;
                        let list = curseforge_api::get_files(vec![fid])
                            .await
                            .map_err(|e| e.to_string())?;
                        let mut data = list
                            .into_iter()
                            .next()
                            .ok_or_else(|| err_keys::FILE_NOT_FOUND.to_string())?;
                        add_game::install_curseforge(
                            &mut data,
                            group,
                            icon,
                            gui,
                            pack_gui,
                            None,
                            install_token,
                        )
                        .await
                        .map_err(|e| e.to_string())
                    }
                    "modrinth" => {
                        let data = modrinth_api::get_version(&project_id, &file_id)
                            .await
                            .map_err(|e| e.to_string())?;
                        add_game::install_modrinth(
                            &data,
                            group,
                            icon,
                            gui,
                            pack_gui,
                            None,
                            install_token,
                        )
                        .await
                        .map_err(|e| e.to_string())
                    }
                    _ => Err(String::from(err_keys::UNKNOWN_SOURCE)),
                }
            })
        })
        .await
        .unwrap_or_else(|e| Err(e.to_string()));

        finish_task(&app_task, &task, &token, res);
    });

    Ok(())
}

/// 取消一个进行中的整合包安装任务，按 pid 与 fid 定位
///
/// - `app`: 应用句柄，用于广播状态事件
/// - `pid`: 项目 ID
/// - `fid`: 文件 ID
///
/// # 返回值
///
/// 标记取消后返回 `Ok(())`；找不到该任务返回 `Err`
#[gui_macros::ipc_group("add_modpack")]
#[tauri::command]
pub async fn add_modpack_cancel(app: AppHandle, pid: String, fid: String) -> Result<(), String> {
    let key = SourceInfo { pid, fid };
    let task = DOWNLOAD_NOW.read().unwrap().get(&key).cloned();
    let Some(task) = task else {
        return Err(String::from(err_keys::TASK_NOT_FOUND));
    };
    task.cancelled.store(true, Ordering::Release);
    task.cancel.cancel();
    emit_add_modpack_status(&app, build_status(&app));
    Ok(())
}

/// 查询整合包安装任务总览，窗口挂载时同步一次，之后靠事件
///
/// - `app`: 应用句柄，用于算各任务的窗口开关状态
///
/// # 返回值
///
/// 当前全部安装任务的快照
#[gui_macros::ipc_group("add_modpack")]
#[tauri::command]
pub async fn add_modpack_status(app: AppHandle) -> Result<ModPackStatusDto, String> {
    Ok(build_status(&app))
}

/// 清除已结束的安装任务，即完成 / 失败 / 取消三态
///
/// - `app`: 应用句柄，用于广播状态事件
///
/// # 返回值
///
/// 清完即返回 `Ok(())`
#[gui_macros::ipc_group("add_modpack")]
#[tauri::command]
pub async fn add_modpack_clear_done(app: AppHandle) -> Result<(), String> {
    DOWNLOAD_NOW.write().unwrap().retain(|_, task| {
        !(task.done.load(Ordering::Acquire)
            || task.failed.load(Ordering::Acquire)
            || task.cancelled.load(Ordering::Acquire))
    });
    emit_add_modpack_status(&app, build_status(&app));
    Ok(())
}
