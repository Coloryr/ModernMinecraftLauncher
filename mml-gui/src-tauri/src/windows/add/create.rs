//! 添加实例窗口的"新建 / 导入"：文件夹预览、压缩包探测、按文件夹 / 压缩包 / 网址导入
//!
//! 从 `add/mod.rs` 拆出来的。三个导入入口最后都汇到内核的安装流程，任务登记在共享的
//! `modpack_task` 注册表里。命令带分组标记，值填原始组名 add，把组键钉回 `add`，
//! 见 AGENTS.md §4。

use std::sync::{Arc, Mutex};

use tauri::{AppHandle, WebviewWindow};
use tokio_util::sync::CancellationToken;

use mml_game::add_game::{self, PackType};
use mml_game::launcher::instance_setting_obj::InstanceSettingObj;

use crate::dtos::{DetectedPackDto, DirEntry, FolderInstanceDto};
use crate::windows::add_resource::SourceInfo;
use crate::windows::modpack_task::{LOCAL_PACK_PID, finish_task, register_task};
use uuid::Uuid;

use super::loader::{parse_loader, parse_pack_type};
use super::model::{AddWindowModel, instance_gui, model, pack_gui};

/// 任务前置检查：确认窗口模型存在，即窗口已开，并返回模型与新任务的取消令牌
///
/// HTTP 客户端由 `mml_core::load` 保证就绪，这里不再重复检查；原先的注释说检查了，
/// 实际只查了模型。
///
/// - `window`: 调用方窗口，用于取本窗口模型
///
/// # 返回值
///
/// 模型缺失时返回 `err.modelMissing`
pub(super) fn prepare(
    window: &WebviewWindow,
) -> Result<(Arc<Mutex<AddWindowModel>>, CancellationToken), String> {
    let store = model(window)?;
    let token = CancellationToken::new();
    store.lock().unwrap().set_cancel(token.clone());
    Ok((store, token))
}

/// 列出目录的直接内容，目录优先，再按名称排序
///
/// 添加实例窗口选择文件夹时调用，用于预览文件夹内容树
///
/// - `path`: 目录路径
///
/// # 返回值
///
/// 直接子项列表，只含名字与是否目录；目录打不开返回 `Err`
#[gui_macros::ipc_group("add")]
#[tauri::command]
pub fn add_list_dir(path: String) -> Result<Vec<DirEntry>, String> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(&path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        entries.push(DirEntry { name, is_dir });
    }
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

/// 扫描文件夹里可导入的实例，覆盖官方启动器 `versions/*` 与 MMC `instances/*`
///
/// 选到 `.minecraft` 这类**装着若干实例**的目录时用：一个目录一个实例，
/// 由前端列出来让用户勾选要导入哪些，而不是把整个 `.minecraft` 当成一个实例。
/// 扫描规则见 `mml_game::scan_game_from_path`。
///
/// **必须异步**：扫描是纯磁盘活，要读并解析各候选目录里的 json，modded 实例的
/// `config/` 下可能几百个；同步命令会占着主线程，窗口直接卡死 ——
/// 放 `spawn_blocking` 里跑，界面照常响应。
///
/// - `path`: 待扫描的目录路径
///
/// # 返回值
///
/// 扫到的实例列表，每项含实例名与目录路径；扫描线程出错时返回 `Err`
#[gui_macros::ipc_group("add")]
#[tauri::command]
pub async fn add_scan_folder(path: String) -> Result<Vec<FolderInstanceDto>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        mml_game::scan_game::scan_game_from_path(&path)
            .into_iter()
            .map(|item| FolderInstanceDto {
                name: item
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                path: item.display().to_string(),
            })
            .collect::<Vec<FolderInstanceDto>>()
    })
    .await
    .map_err(|e| e.to_string())
}

/// 列出压缩包内的条目路径，目录以 `/` 结尾，添加实例窗口预览内容树用
///
/// **必须异步**：要打开压缩包并读出全部条目，整合包里几千个文件时会明显耗时，
/// 同步命令占主线程会把窗口冻住。
///
/// - `path`: 压缩包路径
///
/// # 返回值
///
/// 包内全部条目的名字；打不开压缩包返回 `Err`
#[gui_macros::ipc_group("add")]
#[tauri::command]
pub async fn add_list_archive(path: String) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let archive = mml_base::archives::BaseArchive::open(&path).map_err(|e| e.to_string())?;
        Ok(archive
            .entries()
            .iter()
            .map(|e| e.name.clone())
            .collect::<Vec<String>>())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 检测压缩包的整合包类型与推荐实例名，选择压缩包后自动填表用
///
/// **必须异步**：要打开压缩包扫条目、再解析包内 manifest，同 `add_list_archive`。
///
/// - `path`: 压缩包路径
///
/// # 返回值
///
/// 探测结果；无法识别或读包失败返回 `Err`
#[gui_macros::ipc_group("add")]
#[tauri::command]
pub async fn add_detect_archive(path: String) -> Result<DetectedPackDto, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let pack = mml_game::add_game::detect_pack(&path).map_err(|e| e.to_string())?;
        Ok(DetectedPackDto {
            pack_type: pack.pack_type.id().to_string(),
            name: pack.name,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 从头新建实例：按版本与加载器创建
///
/// - `window`: 调用方窗口，用于取本窗口模型登记任务
/// - `app`: 应用句柄，用于发实例变更事件
/// - `name`: 实例名
/// - `version`: 游戏版本号
/// - `loader`: 加载器 ID，取值见 `add_get_loaders`
/// - `loader_version`: 加载器版本，缺省为无
/// - `group`: 分组 uuid；空 / 非法 = 默认分组
///
/// # 返回值
///
/// 新实例的 uuid；模型缺失或创建失败返回 `Err`
#[gui_macros::ipc_group("add")]
#[tauri::command]
pub async fn add_create_new(
    window: WebviewWindow,
    app: AppHandle,
    name: String,
    version: String,
    loader: String,
    loader_version: Option<String>,
    group: Option<String>,
) -> Result<String, String> {
    let (_store, _token) = prepare(&window)?;
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
    let uuid = { res.read().unwrap().uuid.to_string() };
    crate::windows::main::emit_instance_change(&app, "add");
    Ok(uuid)
}

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
#[tauri::command]
pub async fn add_import_folder(
    window: WebviewWindow,
    app: AppHandle,
    path: String,
    name: Option<String>,
    group: Option<String>,
) -> Result<String, String> {
    let (_store, token) = prepare(&window)?;
    // 前端传的是分组 uuid；空 / 非法 = 默认分组
    let group = crate::windows::parse_group_id(group);
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
    let uuid = { res.read().unwrap().uuid.to_string() };
    crate::windows::main::emit_instance_change(&app, "add");
    Ok(uuid)
}

/// 导入整合包压缩包为实例
///
/// CurseForge / Modrinth 两种是**真整合包**：登记成「下载整合包」那套安装任务，
/// 任务表与进度事件见 [`super::modpack_task`]；于是标题栏的整合包指示器、
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
#[tauri::command]
pub async fn add_import_archive(
    window: WebviewWindow,
    app: AppHandle,
    path: String,
    pack_type: String,
    name: Option<String>,
    group: Option<String>,
    unselect: Option<Vec<String>>,
) -> Result<String, String> {
    let (_store, token) = prepare(&window)?;
    // 前端传的是分组 uuid；空 / 非法 = 默认分组
    let group = crate::windows::parse_group_id(group);
    let pack = parse_pack_type(&pack_type)?;
    let gui = instance_gui(&window);
    let name = name.filter(|item| !item.trim().is_empty());

    // 整合包：进度回调换成任务那一份 —— 本窗口的安装进度弹窗不再出现，
    // 进度改由标题栏的整合包指示器 / 进度弹窗显示，与在线安装一致
    let (progress, task) = if is_modpack(&pack) {
        let key = SourceInfo {
            pid: LOCAL_PACK_PID.to_string(),
            fid: path.clone(),
        };
        let display = name.clone().unwrap_or_else(|| pack_file_stem(&path));
        let (task, progress) = register_task(&app, key, "local", display, token.clone())?;
        (progress, Some(task))
    } else {
        (pack_gui(&window), None)
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
                None,
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

    // 终态：整合包任务自己记录并广播，成功时连同 instance-change 一起发，
    // 其它类型跟以前一样只补一条 instance-change
    match &task {
        Some(task) => finish_task(&app, task, &token, res.clone()),
        None => {
            if res.is_ok() {
                crate::windows::main::emit_instance_change(&app, "add");
            }
        }
    }

    res.map(|uuid| uuid.to_string())
}

/// 该压缩包类型是否为整合包，即内核里走整合包安装 worker 的那两种 manifest
///
/// - `pack`: 压缩包类型
///
/// # 返回值
///
/// CurseForge 与 Modrinth 返回 `true`，其余类型为 `false`
pub(super) fn is_modpack(pack: &PackType) -> bool {
    matches!(pack, PackType::CurseForge | PackType::Modrinth)
}

/// 取压缩包文件名并去掉路径与扩展名：本地包没有项目元数据，任务显示名只能取它
///
/// - `path`: 压缩包路径
///
/// # 返回值
///
/// 文件名主干；取不到时原样返回传入的路径
pub(super) fn pack_file_stem(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .map(|item| item.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
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
#[tauri::command]
pub async fn add_import_url(
    window: WebviewWindow,
    app: AppHandle,
    url: String,
    name: Option<String>,
    group: Option<String>,
) -> Result<String, String> {
    let (_store, token) = prepare(&window)?;
    // 前端传的是分组 uuid；空 / 非法 = 默认分组
    let group = crate::windows::parse_group_id(group);
    let gui = instance_gui(&window);
    let progress = pack_gui(&window);
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
    crate::windows::main::emit_instance_change(&app, "add");
    Ok(uuid.to_string())
}
