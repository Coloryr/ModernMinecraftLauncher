//! 窗口几何的持久化
//!
//! 一份 window_save.json 记全部窗口的位置与大小，
//! 关窗时存、开窗时按合法性收敛后取用。

use std::{collections::HashMap, path::{Path, PathBuf}, sync::{LazyLock, OnceLock, RwLock}};

use mml_base::serialize_tools;
use mml_config::config_save;
use mml_names::{names, uuids};
use serde::{Deserialize, Serialize};
use tauri::WebviewWindow;
use uuid::Uuid;

use crate::windows::registry::{min_size_of_label};
use super::size::{PhysicalDelta, min_inner_physical, decoration_growth};


/// 窗口几何状态，落盘为 window_save.json
///
/// 全仓统一这一套口径：
/// - `x / y`：**外框**位置，单位物理像素，与 `outer_position()` 同源
/// - `width / height`：**带装饰口径的客户区**尺寸，单位物理像素，与 `inner_size()` 同源，
///   下限见 [`save_window_state`]
///
/// 注册表 `WINDOWS_INFO` 里的最小 / 默认尺寸则是**客户区 + 逻辑像素**，两者之间只差显示器的
/// 缩放系数一次换算，见 [`min_inner_physical`]；只有下发给 Windows 的外框最小尺寸
/// 才需要再加上那圈边框带，见 [`reconcile_min_size`]。
///
/// ⚠ `inner_size()` 的含义**随装饰状态变化**：有原生 frame 时是客户区，
/// 插件 `set_decorations(false)` 之后同一块窗口会把标题栏并进客户区，外框不变。
/// 而保存动作在两种状态下都会发生，直接存它会让窗口高度每次开关都涨一截。
/// 所以存盘前统一折算回**带装饰口径**，见 [`decoration_growth`]。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub(crate) struct WindowState {
    /// 外框 X，单位物理像素，与 `outer_position()` 同源
    pub x: i32,
    /// 外框 Y，单位物理像素，与 `outer_position()` 同源
    pub y: i32,
    /// 客户区宽，单位物理像素，与 `inner_size()` 同源
    pub width: u32,
    /// 客户区高，单位物理像素，与 `inner_size()` 同源
    pub height: u32,
    /// 上次退出时是否处于最大化，全屏也算
    ///
    /// 为 true 时 `x / y / width / height` 保留的是**最大化之前**的几何：开窗先按它开、
    /// 再最大化，这样既回到原来那块屏，也能恢复最大化状态。
    pub maximized: bool,
}

/// 窗口几何状态，键是 uuid，内存中的唯一数据源
pub(crate) static WINDOWS_STATE: LazyLock<RwLock<HashMap<Uuid, WindowState>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 窗口状态文件路径，即 window_save.json
pub(crate) static STATE_FILE: OnceLock<PathBuf> = OnceLock::new();

/// 「带装饰」状态下「外框 − 客户区」的差值缓存：键是 uuid，值见 [`PhysicalDelta`]
///
/// 用途见 [`decoration_growth`]：存盘要把无装饰时被撑大的客户区折算回带装饰口径，
/// 需要知道标题栏有多高；而关窗保存时窗口已无装饰、量不到它，只能在**建窗那一刻**
/// 仍是 `decorations(true)` 时先量下来。
pub(crate) static DECORATED_INSET: LazyLock<RwLock<HashMap<Uuid, PhysicalDelta>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 量并缓存「带装饰」状态下的外框 − 客户区差值，建窗后立即调用
///
/// - `uuid`: 窗口 uuid
/// - `window`: 刚建好的窗口，此刻仍是原生装饰
pub(crate) fn remember_decorated_inset(uuid: &Uuid, window: &WebviewWindow) {
    let (Ok(inner), Ok(outer)) = (window.inner_size(), window.outer_size()) else {
        return;
    };
    let inset = PhysicalDelta {
        width: outer.width.saturating_sub(inner.width),
        height: outer.height.saturating_sub(inner.height),
    };
    DECORATED_INSET.write().unwrap().insert(*uuid, inset);
}

/// 读取窗口状态文件，启动时调用，文件位于运行路径下
///
/// - `path`: 运行目录，文件名由 `names::WINDOW_SAVE_FILE` 补上
pub fn init<P: AsRef<Path>>(path: P) {
    let file = STATE_FILE.get_or_init(|| path.as_ref().join(names::WINDOW_SAVE_FILE));

    if file.exists()
        && file.is_file()
        && let Ok(data) = serialize_tools::json_from_file::<HashMap<Uuid, WindowState>>(file)
    {
        WINDOWS_STATE.write().unwrap().extend(data);
    }
}

/// 保存窗口状态
pub(crate) fn save() {
    let Some(file) = STATE_FILE.get() else {
        return;
    };
    let data = WINDOWS_STATE.read().unwrap();
    config_save::save(uuids::WINDOW_FILE_UUID, &*data, file);
}

/// 读取指定 uuid 的窗口几何，用于启动时恢复位置大小
///
/// - `uuid`: 窗口 uuid
///
/// # 返回值
///
/// 没有历史几何时返回 `None`，默认尺寸由调用方给
pub(crate) fn window_state_for(uuid: &Uuid) -> Option<WindowState> {
    WINDOWS_STATE.read().unwrap().get(uuid).cloned()
}

/// 设置窗口状态
///
/// - `uuid`: 窗口 uuid
/// - `state`: 新的几何状态，写进内存表后立即落盘
pub(crate) fn window_state_set(uuid: &Uuid, state: WindowState) {
    WINDOWS_STATE.write().unwrap().insert(*uuid, state);
    save();
}

/// 保存窗口几何到状态表
///
/// **位置按外框、尺寸按客户区**，各自与恢复端同源：
/// - `.position()` 设置的是外框位置 → 存 `outer_position()`
/// - 尺寸存客户区 `inner_size()`，再**折算回「带装饰」口径**存下去，
///   折算量见 [`decoration_growth`]
///
/// 为什么尺寸不用 `outer_size()`：注册表里的最小尺寸、恢复端的 `.inner_size()`
/// 都是**客户区**口径，存外框的话每一处都得再减一次边框带；而 `inner_size()` 的
/// **含义会随装饰状态变化** —— 有原生 frame 时它是"客户区"，插件激活调
/// `set_decorations(false)` 之后，同一块窗口的客户区会多出标题栏那一段。本函数在
/// 两种状态下都会被调用，建窗后立刻存一次、关窗时再存一次，于是存进去的高度会在
/// 600 / 621 / 630 之间来回漂，用户看到的就是"每次开关高度都变"；
/// 实测主窗口注册表定义 600，文件里出现过 621 与 630。折算就是为了消掉这个漂移。
///
/// 两条统一规则：
/// - **无装饰**时把客户区夹到不小于注册表里的最小客户区，见 [`min_inner_physical`]；
///   带装饰时不夹，下限由 [`clamp_runtime_min_size`] 在激活后补；
/// - `maximized` 为真时**不覆盖几何**，只记下这个标志，最大化与全屏都算。最大化窗口的
///   `outer_position()` 带着框外偏移，常见是 −8，当成普通位置存下来，下次开窗
///   `monitor_from_point` 可能解析到**另一块显示器**，而且开出来还不是最大化。
///
/// - `uuid`: 窗口 uuid
/// - `window`: 目标窗口，取它的位置与尺寸
/// - `maximized`: 是否处于最大化或全屏，为真时只记标志、不覆盖几何
///
/// # 返回值
///
/// 位置或尺寸取不到时返回错误文案
pub(crate) fn save_window_state(
    uuid: &Uuid,
    window: &WebviewWindow,
    maximized: bool,
) -> Result<(), String> {
    let pos = window.outer_position().map_err(|err| err.to_string())?;
    let size = window.inner_size().map_err(|err| err.to_string())?;

    let mut geom = window_state_for(uuid).unwrap_or_default();

    let scale = window.scale_factor().unwrap_or(1.0);
    // 物理像素的最小客户区，夹取用。必须夹：窗口停在「外框 = 最小尺寸」上时，主窗口
    // 920×600 的客户区只有 904×600，不收的话这个偏小的值会被一直写回、不收敛。
    // **按标签取而不是按 uuid**：窗口族的 uuid 是派生出来的，静态表里没有它，
    // 按 uuid 查会静默退化成"没有下限"
    let min = min_inner_physical(
        min_size_of_label(window.label()).unwrap_or_default(),
        scale,
    );

    if !maximized {
        // 无装饰时标题栏被并进客户区，直接存 inner 每轮净增约 30px，即 639 → 669 → …；
        // 折算回带装饰口径即收敛，折算量见 decoration_growth
        let growth = decoration_growth(uuid, window);
        // 夹取只在**无装饰**时做：那时 inner 是运行时口径。带装饰时 inner 是带装饰口径，
        // 比运行时口径小一个标题栏，用最小客户区夹会把它顶高，窗口每开一次就高一截
        let (width, height) = if growth.width > 0 || growth.height > 0 {
            (size.width.max(min.width), size.height.max(min.height))
        } else {
            (size.width, size.height)
        };
        geom.x = pos.x;
        geom.y = pos.y;
        geom.width = width.saturating_sub(growth.width);
        geom.height = height.saturating_sub(growth.height);
    }
    geom.maximized = maximized;
    window_state_set(uuid, geom);

    Ok(())
}
