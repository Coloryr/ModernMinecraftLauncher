//! 实例的创建与编辑
//!
//! 新建以最新的正式版为底，改名只动显示名；
//! 更新实例读写的是实例配置，改前先确认没有任务在跑。

use tauri::{AppHandle, WebviewWindow};
use uuid::Uuid;

use mml_game::launcher::{LogEncoding, ModPackType};
use mml_game::loader::LoaderType;
use mml_game::mojang::VersionType;
use mml_sys::open_helper;

use crate::dtos::{InstanceArgsDto, InstanceInfoDto, InstancePatch};
use crate::err_keys;
use crate::windows;

use crate::windows::main::{emit_instance_change, model};
use super::group::{ensure_idle, uuid_short};


/// 创建实例
///
/// - `app`: 应用句柄，用于发变更事件
/// - `window`: 调用方窗口，用于取本窗口模型存实例
/// - `name`: 实例名，同时作为实例目录名
/// - `version`: 游戏版本号
/// - `loader`: 加载器 ID，缺省为 `normal`
/// - `loader_version`: 加载器版本，缺省为无
/// - `group`: 分组 uuid，原样存下不做解析；缺省为无分组
/// - `modpack_type`: 整合包类型 ID，缺省为非整合包
/// - `source`: 下载源标识，缺省为无
///
/// # 返回值
///
/// 新建实例的 DTO；模型缺失时返回 `Err`
#[allow(clippy::too_many_arguments)]
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_create_instance(
    app: AppHandle,
    window: WebviewWindow,
    name: String,
    version: String,
    loader: Option<String>,
    loader_version: Option<String>,
    group: Option<String>,
    modpack_type: Option<String>,
    source: Option<String>,
) -> Result<InstanceInfoDto, String> {
    let uuid = format!("mml-{}", uuid_short());
    let dir = name.clone();
    let quick_play = mml_base::version_parse::is_game_version_120(&version);
    let inst = InstanceInfoDto {
        uuid: uuid.clone(),
        name,
        group,
        version,
        version_type: Some("release".into()),
        quick_play,
        auto_save: None,
        loader: loader.unwrap_or_else(|| "normal".into()),
        loader_version,
        dir,
        running: false,
        modpack_type,
        pid: None,
        fid: None,
        server_url: None,
        lang: None,
        log_encoding: None,
        source,
        // 新建的实例排到组末：给一个足够大的值，下次整组重编号时会被压回正常区间
        order: i32::MAX,
    };
    let store = model(&window)?;
    let mut store = store.lock().unwrap();
    store.instances.insert(0, inst.clone());
    store.args.insert(uuid, InstanceArgsDto::default());
    drop(store);
    emit_instance_change(&app, "add");
    Ok(inst)
}

/// 重命名实例，核心实例走 mml-game：重名报错、实例目录跟随改名
///
/// - `app`: 应用句柄，用于发变更事件
/// - `window`: 调用方窗口，用于取本窗口模型改遗留数据
/// - `uuid`: 实例 uuid；命中核心实例时走 mml-game，否则只改本地存储
/// - `name`: 新实例名，前后空白会被去掉
///
/// # 返回值
///
/// 改成功返回 `true`；名字为空或实例不存在返回 `false`；核心侧重名返回 `Err`
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_rename_instance(
    app: AppHandle,
    window: WebviewWindow,
    uuid: String,
    name: String,
) -> Result<bool, String> {
    let n = name.trim().to_string();
    if n.is_empty() {
        return Ok(false);
    }
    mml_game::check_name(&n).map_err(|e| e.to_string())?;
    // 改名就是改实例目录名，正忙时同样不许，见 ensure_idle
    if let Ok(id) = Uuid::parse_str(&uuid) {
        ensure_idle(&id)?;
    }
    // 核心实例：mml-game 重命名，重名返回 Err 由前端提示
    if let Ok(id) = Uuid::parse_str(&uuid)
        && mml_game::get_instance(&id).is_some()
    {
        mml_game::rename_instance(&id, &n).map_err(|e| e.to_string())?;
        emit_instance_change(&app, "edit");
        return Ok(true);
    }
    // 遗留数据即假 uuid 的实例：只改本地存储
    let store = model(&window)?;
    let mut store = store.lock().unwrap();
    let Some(inst) = store.instances.iter_mut().find(|i| i.uuid == uuid) else {
        return Ok(false);
    };
    inst.name = n.clone();
    inst.dir = n;
    emit_instance_change(&app, "edit");
    Ok(true)
}

/// 复制实例：整份实例目录复制到新名字下，再登记成新实例
///
/// - `app`: 应用句柄，用于发变更事件
/// - `uuid`: 源实例 uuid
/// - `name`: 新实例名，前后空白会被去掉
///
/// # 返回值
///
/// 复制成功返回 `true`；名字为空返回 `false`；名字不合法 / 重名、实例不存在或复制失败返回 `Err`
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub async fn main_copy_instance(app: AppHandle, uuid: String, name: String) -> Result<bool, String> {
    let n = name.trim().to_string();
    if n.is_empty() {
        return Ok(false);
    }
    let id = Uuid::parse_str(&uuid).map_err(|_| err_keys::UUID.to_string())?;
    if mml_game::get_instance(&id).is_none() {
        return Err(err_keys::GAME_NOT_FOUND.to_string());
    }
    // 复制要读整份实例目录，游戏在跑 / 有任务时会复制到写了一半的文件，见 ensure_idle
    ensure_idle(&id)?;
    mml_game::check_name(&n).map_err(|e| e.to_string())?;

    // 目录可能很大，放阻塞线程里复制，别占着异步运行时
    tauri::async_runtime::spawn_blocking(move || mml_game::copy_instance(&id, &n).map(|_| ()))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    emit_instance_change(&app, "add");
    Ok(true)
}

/// 更新实例元信息，补丁式；核心实例直接写配置并保存，遗留数据只改本地存储
///
/// - `app`: 应用句柄，用于发变更事件
/// - `window`: 调用方窗口，用于取本窗口模型改遗留数据
/// - `uuid`: 实例 uuid；命中核心实例时走 mml-game，否则只改本地存储
/// - `patch`: 要改的字段，缺失即不改；`lang` 无核心字段，仅前端显示
///
/// # 返回值
///
/// 处理完成返回 `true`；模型缺失或核心侧改名失败返回 `Err`
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_update_instance(
    app: AppHandle,
    window: WebviewWindow,
    uuid: String,
    patch: InstancePatch,
) -> Result<bool, String> {
    // 核心实例：写真实配置
    let core = Uuid::parse_str(&uuid)
        .ok()
        .and_then(|id| mml_game::get_instance(&id).map(|inst| (id, inst)));
    if let Some((id, instance)) = core {
        // 名字改动走 rename：实例目录跟随改名，重名报错
        if let Some(v) = &patch.name {
            let n = v.trim();
            if !n.is_empty() && instance.read().unwrap().name != *n {
                mml_game::rename_instance(&id, n).map_err(|e| e.to_string())?;
            }
        }
        // 分组切换走 mml-game 的分组表，追加到目标组末尾并发事件
        if let Some(v) = patch.group.clone() {
            // 前端传的是分组 uuid；空 / 非法 = 默认分组
            mml_game::move_group(vec![id], windows::parse_group_id(v));
        }
        {
            let mut obj = instance.write().unwrap();
            if let Some(v) = &patch.version {
                obj.version = v.clone();
            }
            if let Some(v) = &patch.version_type {
                obj.game_type = VersionType::from_id(v);
            }
            if let Some(v) = &patch.loader
                && let Some(l) = LoaderType::from_string(v)
            {
                obj.loader = l;
            }
            if let Some(v) = patch.loader_version.clone() {
                obj.loader_version = v;
            }
            if let Some(v) = patch.modpack_type.clone() {
                obj.modpack_type = ModPackType::from_string(v.as_deref().unwrap_or("none"));
                obj.is_modpack = obj.modpack_type != ModPackType::None;
            }
            if let Some(v) = patch.pid.clone() {
                obj.pid = v;
            }
            if let Some(v) = patch.fid.clone() {
                obj.fid = v;
            }
            if let Some(v) = patch.server_url.clone() {
                obj.server_url = v;
            }
            if let Some(v) = patch.auto_save.clone() {
                // 空串 / 只有空白都按取消处理，别把空目录名当成有效配置
                obj.auto_save = v.filter(|dir| !dir.trim().is_empty());
            }
            if let Some(v) = &patch.log_encoding {
                obj.encoding = match v.as_str() {
                    "gbk" => LogEncoding::GBK,
                    _ => LogEncoding::UTF8,
                };
            }
            // lang 无核心字段，仅前端显示
            obj.save();
        }
    }
    let store = model(&window)?;
    let mut store = store.lock().unwrap();
    if let Some(inst) = store.instances.iter_mut().find(|i| i.uuid == uuid) {
        if let Some(v) = patch.group {
            inst.group = v;
        }
        if let Some(v) = patch.name {
            inst.name = v;
        }
        if let Some(v) = patch.version {
            inst.version = v;
        }
        if let Some(v) = patch.version_type {
            inst.version_type = Some(v);
        }
        if let Some(v) = patch.loader {
            inst.loader = v;
        }
        if let Some(v) = patch.loader_version {
            inst.loader_version = v;
        }
        if let Some(v) = patch.modpack_type {
            inst.modpack_type = v;
        }
        if let Some(v) = patch.pid {
            inst.pid = v;
        }
        if let Some(v) = patch.fid {
            inst.fid = v;
        }
        if let Some(v) = patch.server_url {
            inst.server_url = v;
        }
        if let Some(v) = patch.auto_save {
            inst.auto_save = v.filter(|dir| !dir.trim().is_empty());
        }
        if let Some(v) = patch.lang {
            inst.lang = Some(v);
        }
        if let Some(v) = patch.log_encoding {
            inst.log_encoding = Some(v);
        }
    }
    drop(store);
    emit_instance_change(&app, "edit");
    Ok(true)
}

/// 在文件管理器里打开这个实例的目录
///
/// 落点是**实例目录**（`instances/<实例名>/`，里面有 game.json 等实例文件），
/// 不是游戏目录 `.minecraft` —— 要开游戏目录用资源管理窗口的「打开文件夹」。
///
/// 打开的是目录**本身**：进到里面直接看到那些文件，不是在上层 `instances/` 里选中它。
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// uuid 非法返回 `err.uuid`，实例不存在或目录已不在返回 `err.gameNotFound`
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_open_instance_dir(uuid: String) -> Result<(), String> {
    let Ok(id) = Uuid::parse_str(&uuid) else {
        return Err(err_keys::UUID.to_string());
    };
    let Some(instance) = mml_game::get_instance(&id) else {
        return Err(err_keys::GAME_NOT_FOUND.to_string());
    };

    // 目录建实例时就建好了，这里兜一次：实例目录被手动删掉后别点下去毫无反应
    let dir = instance.read().unwrap().get_base_path();
    if !dir.exists() {
        return Err(err_keys::GAME_NOT_FOUND.to_string());
    }

    open_helper::open_dir(&dir);
    Ok(())
}
