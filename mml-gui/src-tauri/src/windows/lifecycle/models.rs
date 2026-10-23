//! 窗口模型表
//!
//! 每个窗口一份模型，按实例归属；窗口销毁时释放，
//! 建窗前第一次用到时建立。

use std::{any::Any, collections::HashMap, sync::{Arc, LazyLock, Mutex, RwLock}};

use tauri::{Manager, WebviewWindow};

use uuid::Uuid;

use crate::windows::registry::{ADD_MODPACK_WINDOW_UUID, ADD_WINDOW_UUID, MAIN_WINDOW_UUID, WINDOWS_INFO, uuid_for_kind};


/// 已打开窗口的句柄表：uuid → 句柄
pub(crate) static OPEN_WINDOWS: LazyLock<RwLock<HashMap<Uuid, WebviewWindow<tauri::Wry>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 窗口模型表：uuid → 模型
///
/// 窗口模型跟随窗口生命周期：开窗时创建、关窗时销毁，不走全局 app.manage。
/// Tauri 的 `manage` / `State` 底层是同一个全局 StateManager，无法按窗口隔离，
/// 因此模型由本模块持有；命令通过窗口 label 解析 uuid 后取用。
pub(crate) static WINDOW_MODELS: LazyLock<RwLock<HashMap<Uuid, Arc<dyn Any + Send + Sync>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 确保指定窗口的模型已创建，尚无则初始化
///
/// 当前仅主窗口有模型 `MainWindowModel`，其余窗口无状态；
/// 新增窗口模型时在此按 uuid 分派。
///
/// - `uuid`: 窗口 uuid
pub(crate) fn ensure_window_model(uuid: &Uuid) {
    let mut models = WINDOW_MODELS.write().unwrap();
    if *uuid == MAIN_WINDOW_UUID {
        models
            .entry(*uuid)
            .or_insert_with(|| Arc::new(Mutex::new(crate::windows::main::MainWindowModel::new())));
    } else if *uuid == ADD_WINDOW_UUID || *uuid == ADD_MODPACK_WINDOW_UUID {
        models
            .entry(*uuid)
            .or_insert_with(|| Arc::new(Mutex::new(crate::windows::add::AddWindowModel::new())));
    }
}

/// 取窗口模型，按调用方窗口解析
///
/// 兜底：`Destroyed` 事件若在异常关闭路径上丢失，模型会残留——命令进来时
/// 发现窗口已不在，顺带清掉，保证模型严格跟随窗口生命周期。
///
/// - `window`: 调用方窗口，按它的 label 解析 uuid
///
/// # 返回值
///
/// 该窗口的模型；窗口未开、模型缺失或类型不匹配时为 `None`，
/// 由命令侧自行决定降级行为
pub(crate) fn window_model<T: Send + Sync + 'static>(window: &WebviewWindow) -> Option<Arc<T>> {
    let label = window.label().to_string();
    let kind = label.strip_prefix("mml-")?;
    let uuid = uuid_for_kind(kind)?;
    let model = WINDOW_MODELS.read().unwrap().get(&uuid)?.clone();
    if window.get_webview_window(&label).is_none() {
        remove_window_model(&uuid);
        return None;
    }
    model.downcast::<T>().ok()
}

/// 按窗口 uuid 取模型，不检查窗口是否还开着
///
/// 窗口族的模型按**派生 uuid** 登记，命令侧手里只有实例 uuid，
/// 换算出来的就是它，见 `windows::resource::model`。
///
/// - `uuid`: 窗口 uuid
///
/// # 返回值
///
/// 该 uuid 的模型；没有或类型不匹配时为 `None`
pub(crate) fn window_model_by_uuid<T: Send + Sync + 'static>(uuid: &Uuid) -> Option<Arc<T>> {
    WINDOW_MODELS
        .read()
        .unwrap()
        .get(uuid)?
        .clone()
        .downcast::<T>()
        .ok()
}

/// 按窗口 uuid 取模型，没有就用 `make` 建一个
///
/// 存的是**模型本体**（`Arc<T>`），不是 `ensure_window_model` 那套
/// `Arc<Mutex<T>>`：带内部锁的模型（如资源窗口，读写散在多个方法里，还要在
/// `spawn_blocking` 里用）这么放，命令侧拿到 `Arc<T>` 直接用，不必再套一层锁。
/// **两者存进去的类型必须与取的时候一致**，错一层就是 `downcast` 失败、报"窗口模型未初始化"。
///
/// 资源窗口是"一个实例一个窗口"的族，模型没法像静态窗口那样在 `ensure_window_model`
/// 里按常量认领 —— 由命令侧按实例 uuid 算好派生 uuid 后按需建立。
/// 释放仍走窗口销毁那条路（`Destroyed` 里 `remove_window_model`）。
///
/// - `uuid`: 窗口 uuid
/// - `make`: 模型不存在时的构造函数
///
/// # 返回值
///
/// 该 uuid 的模型；表里那条类型不匹配时为 `None`
pub(crate) fn window_model_or_insert<T: Send + Sync + 'static>(
    uuid: &Uuid,
    make: impl FnOnce() -> T,
) -> Option<Arc<T>> {
    let mut models = WINDOW_MODELS.write().unwrap();
    let model = models
        .entry(*uuid)
        .or_insert_with(|| Arc::new(make()))
        .clone();
    model.downcast::<T>().ok()
}

/// 按窗口 kind 取模型，单窗口模式下页面命令用
///
/// 单窗口模式只有一个真实主窗口，"添加实例""下载整合包"都只是应用内的页面，
/// 它们发命令时调用方窗口一律是主窗口 —— 用 [`window_model`] 会解析出主窗口的 uuid、
/// 拿到 `MainWindowModel`，页面的命令便取不到自己的模型，表现为 `err.modelMissing`。
/// 这里由命令显式声明自己属于哪个 kind。
///
/// **不创建模型**：模型仍由"该 kind 的窗口被打开"来创建，多窗口模式走 [`create_window`]，
/// 单窗口模式走前端的 [`window_ensure_model`] 命令；页面关掉时也会被释放，
/// 不会因为这里取一次就常驻下来。
///
/// - `kind`: 窗口类型
///
/// # 返回值
///
/// 模型不存在或类型不匹配时返回 `None`
pub(crate) fn model_for_kind<T: Send + Sync + 'static>(kind: &str) -> Option<Arc<T>> {
    let uuid = uuid_for_kind(kind)?;
    WINDOW_MODELS
        .read()
        .unwrap()
        .get(&uuid)?
        .clone()
        .downcast::<T>()
        .ok()
}

/// 确保某个 kind 的模型存在，单窗口模式下页面打开时由前端调用
///
/// 单窗口模式没有真实窗口可挂，模型的生命周期改由前端页面的启停驱动：
/// 页面挂载时建、切走/关闭时释放，见 [`window_drop_model`]。不这么做的话，
/// "添加实例"这类页面的模型要么取不到 `err.modelMissing`，要么只能常驻占内存。
///
/// - `kind`: 窗口类型
///
/// # 返回值
///
/// 模型已就绪返回 `Ok(())`；kind 未知返回错误文案
pub(crate) fn ensure_model_for_kind(kind: &str) -> Result<(), String> {
    let uuid = uuid_for_kind(kind).ok_or_else(|| format!("unknown window kind: {kind}"))?;
    ensure_window_model(&uuid);
    Ok(())
}

/// 释放某个 kind 的模型，单窗口模式下页面切走 / 关闭时由前端调用
///
/// - `kind`: 窗口类型
///
/// # 返回值
///
/// 模型已释放或该窗口有真实窗口开着时返回 `Ok(())`；kind 未知返回错误文案
pub(crate) fn drop_model_for_kind(kind: &str) -> Result<(), String> {
    let uuid = uuid_for_kind(kind).ok_or_else(|| format!("unknown window kind: {kind}"))?;
    // 该 kind 有真实窗口开着时不释放：多窗口模式下页面还在，模型得留着
    if let Some(label) = WINDOWS_INFO.get(&uuid).map(|e| e.label)
        && app_has_window(label)
    {
        return Ok(());
    }
    remove_window_model(&uuid);
    Ok(())
}

/// 是否存在某个 label 的真实窗口，不经过 AppHandle，查句柄表
///
/// - `label`: 窗口标签
///
/// # 返回值
///
/// 句柄表里有这个标签的真实窗口时为真
pub(crate) fn app_has_window(label: &str) -> bool {
    OPEN_WINDOWS
        .read()
        .unwrap()
        .values()
        .any(|w| w.label() == label)
}

/// 移除窗口模型，窗口销毁或逻辑页关闭时调用
///
/// - `uuid`: 窗口 uuid
pub(crate) fn remove_window_model(uuid: &Uuid) {
    let model = WINDOW_MODELS.write().unwrap().remove(uuid);
    if let Some(model) = model.and_then(|m| m.downcast::<crate::windows::lobby::model::LobbyModel>().ok()) {
        model.close();
    }
}
