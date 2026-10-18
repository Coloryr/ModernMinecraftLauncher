//! 主窗口的实例与分组写操作：新建 / 改名 / 改配置 / 删除 / 移动
//!
//! 从 `main/mod.rs` 拆出来的，光 `main_update_instance` 一个人就 106 行。命令带
//! 分组标记，值填原始组名 main，把组键钉回 `main`，见 AGENTS.md §4。

use tauri::{AppHandle, WebviewWindow};
use uuid::Uuid;

use mml_game::launcher::{LogEncoding, ModPackType};
use mml_game::loader::LoaderType;
use mml_game::mojang::VersionType;
use mml_sys::open_helper;

use crate::dtos::{InstanceArgsDto, InstanceInfoDto, InstancePatch};
use crate::err_keys;
use crate::windows;

use super::{emit_instance_change, model};

/// 新建分组
///
/// 分组表在 mml-game 的 `group_save.json`，GUI 不再自己另存一份 ——
/// 之前模型里那份 `extra_groups` 既不落盘、又与内核表各说各话，
/// 新建的空分组下一次 `getGroups` 就被内核列表覆盖掉了。
///
/// - `app`: 应用句柄，用于发变更事件
/// - `name`: 分组名；空名或重名都不建组
///
/// # 返回值
///
/// 返回新分组的 uuid；名字为空或重名返回 `None`，前端据此提示"已存在"
#[gui_macros::ipc_group("main")]
#[tauri::command]
pub fn main_add_group(app: AppHandle, name: String) -> Result<Option<String>, String> {
    let Some(uuid) = mml_game::add_group(&name) else {
        return Ok(None);
    };
    emit_instance_change(&app, "group");
    Ok(Some(uuid.to_string()))
}

/// 删除分组，组内实例移入默认分组
///
/// - `app`: 应用句柄，用于发变更事件
/// - `uuid`: 分组 uuid；空 / 非法即默认分组
///
/// # 返回值
///
/// 真删掉了返回 `true`；默认分组或分组不存在返回 `false`
#[gui_macros::ipc_group("main")]
#[tauri::command]
pub fn main_remove_group(app: AppHandle, uuid: String) -> Result<bool, String> {
    // 空白 uuid 就是默认分组，不允许删除
    let Some(uuid) = windows::parse_group_id(Some(uuid)) else {
        return Ok(false);
    };
    // 核心分组表：组内实例移入默认分组，并逐个发变更通知
    if !mml_game::remove_group(&uuid) {
        return Ok(false);
    }
    emit_instance_change(&app, "group");
    Ok(true)
}

/// 调整分组显示顺序，默认分组只是初始排首位，同样可以换位置
///
/// 顺序由内核分组表保存：拿当前顺序、把该组挪到 `index` 位，再整表提交。
/// 只改内存里的数组是不行的 —— 文件里存的还是旧顺序，重启就回来了。
///
/// - `app`: 应用句柄，用于发变更事件
/// - `uuid`: 要挪的分组 uuid
/// - `index`: 目标位置，从 0 起；负数按 0、超出表长按表尾
///
/// # 返回值
///
/// 挪成功返回 `true`；uuid 为空或不是 uuid、该组不在表里时返回 `Err`
#[gui_macros::ipc_group("main")]
#[tauri::command]
pub fn main_move_group(app: AppHandle, uuid: String, index: i64) -> Result<bool, String> {
    // 这里**不**按 `parse_group_id` 的"非法 = 默认分组"兜底：空 / 非法的 uuid 指不到
    // 任何组，不像 `main_move_instance` 那样有默认分组这个有意义的落点，直接报错更安全
    let Some(uuid) = windows::parse_group_id(Some(uuid)) else {
        return Err(err_keys::UUID.to_string());
    };

    let mut list: Vec<Uuid> = mml_game::get_group_list()
        .into_iter()
        .map(|g| g.uuid)
        .collect();
    let from = list
        .iter()
        .position(|g| *g == uuid)
        .ok_or_else(|| err_keys::GROUP_NOT_FOUND.to_string())?;
    list.remove(from);
    let at = (index.max(0) as usize).min(list.len());
    list.insert(at, uuid);

    mml_game::reorder_groups(&list);
    emit_instance_change(&app, "group");
    Ok(true)
}

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
#[tauri::command]
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

/// 实例正忙时给出要报给前端的错误
///
/// 删除与重命名都要过这一关：两种操作都动实例目录，而正在写的东西会跟着写到
/// 已经删掉 / 改名的路径上，留下对不上的目录与注册表。
///
/// - `instance`: 实例 uuid
///
/// # 返回值
///
/// 忙时是给前端翻的错误键；空闲是 `Ok(())`
fn ensure_idle(instance: &Uuid) -> Result<(), String> {
    // 游戏进程还在跑
    if mml_game::is_running(instance) {
        return Err(err_keys::INSTANCE_RUNNING.to_string());
    }
    // 还有文件在往这个实例的目录里下（模组更新、资源下载等）
    if let Some(game) = mml_game::get_instance(instance) {
        let dir = game.read().unwrap().get_base_path();
        if mml_downloader::is_busy_under(&dir) {
            return Err(err_keys::INSTANCE_BUSY.to_string());
        }
    }
    // 添加资源窗口还在往这个实例里下资源
    if crate::windows::add_resource::instance_busy(instance) {
        return Err(err_keys::INSTANCE_BUSY.to_string());
    }
    // 资源窗口里还有操作在跑：读取各分类、检查更新、下载更新、自动标记…
    if crate::windows::resource::instance_busy(instance) {
        return Err(err_keys::INSTANCE_BUSY.to_string());
    }
    Ok(())
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
#[tauri::command]
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
#[tauri::command]
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

/// 删除实例：挪回收站较慢，放后台线程执行避免卡住 UI，前端显示滚动进度条
/// 核心实例走 mml-game：删除实例与文件；同时清理本地运行态缓存
///
/// - `app`: 应用句柄，用于发变更事件
/// - `window`: 调用方窗口，用于取本窗口模型清理遗留数据
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 核心实例或本地存储任一删掉了就返回 `true`；都没有则 `false`
#[gui_macros::ipc_group("main")]
#[tauri::command]
pub async fn main_delete_instance(
    app: AppHandle,
    window: WebviewWindow,
    uuid: String,
) -> Result<bool, String> {
    // 正忙时不许删，见 ensure_idle
    if let Ok(id) = Uuid::parse_str(&uuid) {
        ensure_idle(&id)?;
    }

    // 删完之后要按它关窗口；uuid 马上会被 move 进闭包，先算好
    let close_id = Uuid::parse_str(&uuid).ok();

    let ok = tauri::async_runtime::spawn_blocking(move || -> Result<bool, String> {
        // 核心实例：删除实例数据与文件
        let mut ok = false;
        if let Ok(id) = Uuid::parse_str(&uuid)
            && mml_game::get_instance(&id).is_some()
        {
            mml_game::delete_instance(&id).map_err(|e| e.to_string())?;
            ok = true;
        }
        // 本地存储：清理遗留实例项与运行态缓存，含运行状态 / 日志 / 启动参数
        let store = model(&window)?;
        let mut store = store.lock().unwrap();
        let before = store.instances.len();
        store.instances.retain(|i| i.uuid != uuid);
        store.args.remove(&uuid);
        store.running.remove(&uuid);
        if store.instances.len() < before {
            ok = true;
        }
        drop(store);
        Ok(ok)
    })
    .await
    .map_err(|e| e.to_string())??;

    if ok {
        emit_instance_change(&app, "remove");
        // 实例没了，挂在它名下的窗口一并关掉：资源管理 / 日志 / 添加资源 / 配置文件
        if let Some(id) = close_id {
            crate::windows::lifecycle::close_instance_windows(&app, &id);
        }
    }
    Ok(ok)
}

/// 移动实例到指定分组与组内位置：支持同组排序与跨组移动
///
/// 归属与组内次序都在内核分组表的 `group_save.json`：内核一次完成"落组 + 插到 index 位"，
/// 前端下次拉列表就按表的数组顺序拿到新的 `order`。
///
/// 这里不再改各实例的 `gui_setting.json` —— 那个 `Order` 已经是旧机制，
/// 两份顺序各写各的正是"拖完看着对了、重启就乱"的来源。
///
/// - `app`: 应用句柄，用于发变更事件
/// - `uuid`: 实例 uuid
/// - `group`: 目标分组 uuid；空 / 非法 = 默认分组
/// - `index`: 目标组内位置，从 0 起；负数按 0 处理
///
/// # 返回值
///
/// 移动成功返回 `true`；uuid 非法或实例不存在返回 `Err`
#[gui_macros::ipc_group("main")]
#[tauri::command]
pub fn main_move_instance(
    app: AppHandle,
    uuid: String,
    group: Option<String>,
    index: i64,
) -> Result<bool, String> {
    let Ok(id) = Uuid::parse_str(&uuid) else {
        return Err(err_keys::UUID.to_string());
    };
    if mml_game::get_instance(&id).is_none() {
        return Err(err_keys::GAME_NOT_FOUND.to_string());
    }

    // 前端传的是分组 uuid；空 / 非法 = 默认分组
    let group = windows::parse_group_id(group);

    mml_game::move_instance(&id, group, index.max(0) as usize);

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
#[tauri::command]
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

/// 生成短 uuid，形如 `mml-<16 位十六进制>`，给不落内核的遗留实例用
///
/// 原来是 `subsec_nanos() ^ pid`：Windows 的时钟粒度远粗于 1ns，**同一个 tick 内
/// 连续创建两个实例会拿到同一个 id**，而本地存储正是按 uuid 索引的，会互相覆盖。
/// 这里改成完整纳秒 + 进程内自增序号：同 tick 靠序号区分，跨 tick 靠纳秒。
///
/// # 返回值
///
/// 16 位十六进制字符串，不含 `mml-` 前缀，由调用方拼
fn uuid_short() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    /// 进程内自增序号，乘一个奇数常量后异或，保证 seq 不同则结果必不同
    static SEQ: AtomicU64 = AtomicU64::new(0);

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or_default();
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);

    format!(
        "{:016x}",
        nanos ^ ((std::process::id() as u64) << 32) ^ seq.wrapping_mul(0x9E37_79B9_7F4A_7C15)
    )
}
