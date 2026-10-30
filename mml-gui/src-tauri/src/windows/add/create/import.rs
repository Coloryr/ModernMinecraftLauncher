//! 新建实例与导入
//!
//! 目录、压缩包、下载地址三条路都汇到同一份任务上；
//! 建实例是最后一步，前面任何一步失败都不落盘。

use tauri::{AppHandle, WebviewWindow};

use mml_game::add_game::{self, PackType};
use mml_game::launcher::instance_setting_obj::InstanceSettingObj;

use crate::windows::add_resource::SourceInfo;
use crate::windows::modpack_task::{LOCAL_PACK_PID, RegisteredTask, finish_task, register_task};
use uuid::Uuid;

use super::prepare::{PreparedTask, is_modpack, pack_file_stem, prepare};
use crate::dtos::ServerPackProbeDto;
use crate::windows::add::loader::{parse_loader, parse_pack_type};
use crate::windows::add::model::{instance_gui, pack_archive_gui, pack_gui};

/// 导入文件夹为实例
///
/// - `window`: 调用方窗口，用于取本窗口模型登记任务
/// - `app`: 应用句柄，用于发实例变更事件
/// - `path`: 待导入的文件夹路径
/// - `name`: 实例名，缺省由内核定
/// - `group`: 分组 uuid；空 / 非法 = 默认分组
///
/// # 返回值
///
/// 新实例的 uuid；模型缺失或导入失败返回 `Err`
#[gui_macros::ipc_group("add")]
#[gui_macros::command]
pub async fn add_import_folder(
    window: WebviewWindow,
    app: AppHandle,
    path: String,
    name: Option<String>,
    group: Option<String>,
) -> Result<String, String> {
    let PreparedTask {
        model: _store,
        token,
    } = prepare(&window)?;
    // 前端传的是分组 uuid；空 / 非法 = 默认分组
    let group = crate::windows::parse_group_id(group);
    // 用户填了名字就先挡一次：空名照旧交给内核按目录 / 包名定
    let name = name.filter(|item| !item.trim().is_empty());
    if let Some(name) = &name {
        mml_game::check_name(name).map_err(|e| e.to_string())?;
    }
    // 安装 future 非 Send：阻塞线程 block_on；读锁在块内释放后再发事件
    let gui = instance_gui(&window);
    let res = tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(async {
            add_game::add_game_folder(&path, name, group, None, gui, None, token).await
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    let uuid = res.read().unwrap().uuid.to_string();
    crate::windows::main::emit_instance_change(&app, "add");
    crate::windows::add::notify_instance_added(&app, &uuid);
    Ok(uuid)
}

/// 导入整合包压缩包为实例
///
/// CurseForge / Modrinth 两种是**真整合包**：登记成「下载整合包」那套安装任务，
/// 任务表与进度事件见 [`crate::windows::modpack_task`]；于是标题栏的整合包指示器、
/// 进度弹窗、取消、装完刷新并选中新实例全部复用同一条路径，
/// 与在线安装的差别只剩"包从哪来"，所以它没有项目 ID，用占位 pid。
/// 其余类型即 MMC / HMCL / 直接解压等行为不变：进度发到本窗口的进度弹窗。
///
/// 命令**等安装结束**再返回，新实例 uuid 得交给添加实例窗口做后续交互，
/// 期间进度由整合包任务那一套显示。
///
/// - `window`: 调用方窗口，用于取本窗口模型登记任务
/// - `app`: 应用句柄，用于登记任务并发事件
/// - `path`: 压缩包路径
/// - `pack_type`: 压缩包类型 ID，取值见 `add_get_pack_types`
/// - `name`: 实例名，缺省或空白时由内核按包内元数据定
/// - `group`: 分组 uuid；空 / 非法 = 默认分组
/// - `unselect`: 按包内完整条目名排除的文件，来自前端文件树未勾选项
///
/// # 返回值
///
/// 新实例的 uuid 字符串；模型缺失或安装失败返回 `Err`
#[gui_macros::ipc_group("add")]
#[gui_macros::command]
pub async fn add_import_archive(
    window: WebviewWindow,
    app: AppHandle,
    path: String,
    pack_type: String,
    name: Option<String>,
    group: Option<String>,
    unselect: Option<Vec<String>>,
) -> Result<String, String> {
    let PreparedTask {
        model: _store,
        token,
    } = prepare(&window)?;
    // 前端传的是分组 uuid；空 / 非法 = 默认分组
    let group = crate::windows::parse_group_id(group);
    let pack = parse_pack_type(&pack_type)?;
    let gui = instance_gui(&window);
    let name = name.filter(|item| !item.trim().is_empty());
    if let Some(name) = &name {
        mml_game::check_name(name).map_err(|e| e.to_string())?;
    }

    // 整合包：进度回调换成任务那一份 —— 本窗口的安装进度弹窗不再出现，
    // 进度改由标题栏的整合包指示器 / 进度弹窗显示，与在线安装一致
    let (progress, task) = if is_modpack(&pack) {
        let key = SourceInfo {
            pid: LOCAL_PACK_PID.to_string(),
            fid: path.clone(),
        };
        let display = name.clone().unwrap_or_else(|| pack_file_stem(&path));
        let RegisteredTask {
            task,
            gui: progress,
        } = register_task(
            &app,
            key,
            "local",
            display,
            String::new(),
            "install",
            token.clone(),
        )?;
        (progress, Some(task))
    } else {
        (pack_gui(&window), None)
    };

    // 非整合包压缩包：把解压逐文件进度桥到本窗口的安装进度；整合包任务已有同一套桥
    let archive_gui = if task.is_none() {
        pack_archive_gui(progress.clone())
    } else {
        None
    };

    // 安装 future 非 Send：放阻塞线程上 block_on，与 add_create_new 等同一条路子
    let install_token = token.clone();
    let file = path.clone();
    let res: Result<Uuid, String> = match tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(async {
            add_game::install_archive_from_file(
                &file,
                name,
                group,
                unselect,
                gui,
                progress,
                archive_gui,
                pack,
                install_token,
            )
            .await
        })
    })
    .await
    {
        Ok(Ok(uuid)) => Ok(uuid),
        Ok(Err(err)) => Err(err.to_string()),
        Err(err) => Err(err.to_string()),
    };

    // 终态：整合包任务自己记录并广播，成功时连同 instance-change 一起发；
    // 其它类型补一条 instance-change 与系统通知
    match &task {
        Some(task) => finish_task(&app, task, &token, res.clone()),
        None => {
            if let Ok(uuid) = &res {
                crate::windows::main::emit_instance_change(&app, "add");
                crate::windows::add::notify_instance_added(&app, &uuid.to_string());
            }
        }
    }

    res.map(|uuid| uuid.to_string())
}

/// 从网址安装实例，默认按直接解压处理
///
/// - `window`: 调用方窗口，用于取本窗口模型登记任务
/// - `app`: 应用句柄，用于发实例变更事件
/// - `url`: 压缩包下载地址
/// - `name`: 实例名，缺省由内核定
/// - `group`: 分组 uuid；空 / 非法 = 默认分组
///
/// # 返回值
///
/// 新实例的 uuid 字符串；模型缺失或安装失败返回 `Err`
#[gui_macros::ipc_group("add")]
#[gui_macros::command]
pub async fn add_import_url(
    window: WebviewWindow,
    app: AppHandle,
    url: String,
    name: Option<String>,
    group: Option<String>,
) -> Result<String, String> {
    let PreparedTask {
        model: _store,
        token,
    } = prepare(&window)?;
    // 前端传的是分组 uuid；空 / 非法 = 默认分组
    let group = crate::windows::parse_group_id(group);
    let gui = instance_gui(&window);
    let progress = pack_gui(&window);
    let name = name.filter(|item| !item.trim().is_empty());
    if let Some(name) = &name {
        mml_game::check_name(name).map_err(|e| e.to_string())?;
    }
    let uuid = tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(async {
            add_game::install_archive_from_url(
                &url,
                name,
                group,
                None,
                gui,
                progress,
                None,
                PackType::ArchivePack,
                token,
            )
            .await
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    let uuid = uuid.to_string();
    crate::windows::main::emit_instance_change(&app, "add");
    crate::windows::add::notify_instance_added(&app, &uuid);
    Ok(uuid)
}

/// 从头新建实例：按版本与加载器创建
///
/// - `window`: 调用方窗口，用于取本窗口模型登记任务
/// - `app`: 应用句柄，用于发实例变更事件
/// - `name`: 实例名，校验失败直接返回 `Err`
/// - `version`: 游戏版本号
/// - `loader`: 加载器 ID，取值见 `add_get_loaders`
/// - `loader_version`: 加载器版本，缺省为无
/// - `group`: 分组 uuid；空 / 非法 = 默认分组
///
/// # 返回值
///
/// 新实例的 uuid；模型缺失或创建失败返回 `Err`
#[gui_macros::ipc_group("add")]
#[gui_macros::command]
pub async fn add_create_new(
    window: WebviewWindow,
    app: AppHandle,
    name: String,
    version: String,
    loader: String,
    loader_version: Option<String>,
    group: Option<String>,
) -> Result<String, String> {
    mml_game::check_name(&name).map_err(|e| e.to_string())?;

    let PreparedTask {
        model: _store,
        token: _token,
    } = prepare(&window)?;
    // 前端传的是分组 uuid；空 / 非法 = 默认分组
    let group = crate::windows::parse_group_id(group);
    let loader = parse_loader(&loader)?;
    let obj = InstanceSettingObj {
        uuid: Uuid::new_v4(),
        name,
        version,
        loader,
        loader_version,
        ..Default::default()
    };
    // mml-game 的安装 future 非 Send，放阻塞线程上 block_on 执行
    let gui = instance_gui(&window);
    let res = tauri::async_runtime::spawn_blocking(move || {
        // 分组不在实例配置里，由 kernel 的 group_save.json 管，创建时一并指定
        tauri::async_runtime::block_on(obj.create_instance_in_group(gui, group))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    let uuid = res.read().unwrap().uuid.to_string();
    crate::windows::main::emit_instance_change(&app, "add");
    crate::windows::add::notify_instance_added(&app, &uuid);
    Ok(uuid)
}

/// 从在线服务器包（server.json）添加实例
///
/// 与 add_import_url 的区别：那个下载的是压缩包，这个只取清单 —— 实例登记成
/// 「在线网络整合包」，包内容在启动时按清单同步，导入这一步不下载。
///
/// - `window`: 调用方窗口，用于取本窗口模型登记任务
/// - `app`: 应用句柄，用于发实例变更事件
/// - `url`: 服务器实例地址，发布目录或 server.json 都认
/// - `name`: 实例名，缺省用清单里的名字
/// - `group`: 分组 uuid；空 / 非法 = 默认分组
///
/// # 返回值
///
/// 新实例的 uuid；模型缺失、地址为空或清单取不到返回错误文案
#[gui_macros::ipc_group("add")]
#[gui_macros::command]
pub async fn add_import_server_pack(
    window: WebviewWindow,
    app: AppHandle,
    url: String,
    name: Option<String>,
    group: Option<String>,
) -> Result<String, String> {
    let PreparedTask {
        model: _store,
        token,
    } = prepare(&window)?;
    // 前端传的是分组 uuid；空 / 非法 = 默认分组
    let group = crate::windows::parse_group_id(group);
    let gui = instance_gui(&window);
    let name = name.filter(|item| !item.trim().is_empty());
    if let Some(name) = &name {
        mml_game::check_name(name).map_err(|e| e.to_string())?;
    }
    let progress = pack_gui(&window);
    let uuid = tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(async {
            add_game::install_server_pack_from_url(&url, name, group, gui, progress, token).await
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    let uuid = uuid.to_string();
    crate::windows::main::emit_instance_change(&app, "add");
    crate::windows::add::notify_instance_added(&app, &uuid);
    Ok(uuid)
}

/// 探测在线实例地址，返回清单摘要给添加窗口预览
///
/// 填完地址后自动调用：既是"这个地址通不通"的校验，也把清单里的名字 / 版本 / 加载器
/// 显示出来，用户不必下载整包就知道会装成什么。
///
/// - `url`: 用户填的地址，发布目录或 server.json 都认
///
/// # 返回值
///
/// 见 [`ServerPackProbeDto`]；地址为空、下载失败或清单解析失败返回错误文案
#[gui_macros::ipc_group("add")]
#[gui_macros::command]
pub async fn add_probe_server_pack(url: String) -> Result<ServerPackProbeDto, String> {
    let (manifest, pack) = add_game::fetch_server_pack(&url)
        .await
        .map_err(|e| e.to_string())?;
    Ok(ServerPackProbeDto {
        url: manifest,
        name: pack.name,
        version: pack.version,
        loader: pack.loader.id().to_string(),
        loader_version: pack.loader_version,
        pack_version: pack.pack_version,
        text: pack.text,
        file_count: pack.online_list.len() as u64,
        archive_count: pack.archive_list.len() as u64,
    })
}
