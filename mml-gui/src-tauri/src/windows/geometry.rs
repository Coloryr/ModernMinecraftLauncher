//! 窗口几何：口径换算 + `window_save.json` 读写
//!
//! 从 `windows/mod.rs` 拆出来的。这里是全仓**最贵的一段经验**：
//! 外框 / 客户区两种口径、逻辑 / 物理两种单位、以及"摘掉原生装饰会把标题栏并进
//! 客户区"导致的高度漂移 —— 都集中在这个文件里，改之前先读注释。
//!
//! 口径速记，详见 `WindowState` 的文档：
//! - 存盘 / 恢复一律按**客户区**，即 `inner_size()`，且折算回**带装饰**时的口径；
//! - `WINDOWS_INFO` 里的最小尺寸是**客户区 + 逻辑像素**，见 `super::registry`；
//! - 只有下发给 Windows 的外框最小尺寸才需要加上那圈边框带，见 [`reconcile_min_size`]；
//!   而下限**只在装饰激活后设**：建窗时窗口是带装饰口径，那时算出的下限比恢复值高一个标题栏，
//!   tao 重设尺寸时 Windows 会顺着它把窗口顶上去，实测每开一次高 31px。

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{LazyLock, OnceLock, RwLock},
};

use mml_base::serialize_tools;
use mml_config::config_save;
use mml_names::{names, uuids};
use serde::{Deserialize, Serialize};
use tauri::WebviewWindow;
use uuid::Uuid;

use super::registry::{MinSize, min_size_of_label};

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
#[serde(default)]
pub(super) struct WindowState {
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
pub(super) static WINDOWS_STATE: LazyLock<RwLock<HashMap<Uuid, WindowState>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 窗口状态文件路径，即 window_save.json
pub(super) static STATE_FILE: OnceLock<PathBuf> = OnceLock::new();

/// 「带装饰」状态下「外框 − 客户区」的差值缓存：键是 uuid，值见 [`PhysicalDelta`]
///
/// 用途见 [`decoration_growth`]：存盘要把无装饰时被撑大的客户区折算回带装饰口径，
/// 需要知道标题栏有多高；而关窗保存时窗口已无装饰、量不到它，只能在**建窗那一刻**
/// 仍是 `decorations(true)` 时先量下来。
pub(super) static DECORATED_INSET: LazyLock<RwLock<HashMap<Uuid, PhysicalDelta>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 量并缓存「带装饰」状态下的外框 − 客户区差值，建窗后立即调用
///
/// - `uuid`: 窗口 uuid
/// - `window`: 刚建好的窗口，此刻仍是原生装饰
pub(super) fn remember_decorated_inset(uuid: &Uuid, window: &WebviewWindow) {
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
pub(super) fn save() {
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
pub(super) fn window_state_for(uuid: &Uuid) -> Option<WindowState> {
    WINDOWS_STATE.read().unwrap().get(uuid).cloned()
}

/// 设置窗口状态
///
/// - `uuid`: 窗口 uuid
/// - `state`: 新的几何状态，写进内存表后立即落盘
pub(super) fn window_state_set(uuid: &Uuid, state: WindowState) {
    WINDOWS_STATE.write().unwrap().insert(*uuid, state);
    save();
}

/// 窗口客户区的宽高，单位物理像素，由注册表里的最小尺寸换算而来
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct PhysicalSize {
    /// 宽，单位物理像素
    pub(super) width: u32,
    /// 高，单位物理像素
    pub(super) height: u32,
}

/// 宽高两向的物理像素差值，用于「外框 − 客户区」这类差值在两处之间传递
///
/// 与 [`PhysicalSize`] 的区别在语义：那个是**尺寸**，这个是**差值** —— 为 0 只表示
/// 两处量到的一样，不表示宽高是 0。
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct PhysicalDelta {
    /// 宽向差值，单位物理像素；两边相同时为 0
    pub(super) width: u32,
    /// 高向差值，单位物理像素；两边相同时为 0
    pub(super) height: u32,
}

/// 注册表里的最小客户区尺寸由逻辑像素换算成物理像素
///
/// 存盘与恢复都按客户区口径，夹取时用它。**不要在这里加边框带**：那圈只影响下发给 Windows 的
/// **外框**最小尺寸，见 [`reconcile_min_size`]，客户区本身不含它。
///
/// - `min`: 注册表里的最小客户区尺寸，逻辑像素
/// - `scale`: 目标显示器的缩放系数，非法值按 1.0 处理
///
/// # 返回值
///
/// 见 [`PhysicalSize`]，宽高各不小于 1
pub(super) fn min_inner_physical(min: MinSize, scale: f64) -> PhysicalSize {
    let scale = if scale > 0.0 { scale } else { 1.0 };
    PhysicalSize {
        width: (min.width * scale).round().max(1.0) as u32,
        height: (min.height * scale).round().max(1.0) as u32,
    }
}

/// 把注册表里的最小**客户区**尺寸换算成窗口的**外框**最小尺寸，下发给 Windows
///
/// 注册表写的是 webview 能拿到多少排版空间，而 Windows 的最小尺寸走 `WM_GETMINMAXINFO`、
/// 约束的是外框。无边框 + 系统阴影时「外框 − 客户区」就是那圈边框带：96 DPI 下左右下各 8px、
/// 顶部 1px，且只有 Windows 有。这里量出真实差值加上去，随 DPI 自动变化，不用自己算
/// SM_CXSIZEFRAME —— 那双指标也不是按 DPI 倍数缩的：96→8、120→9、144→11。
/// 边框带为 0 时该项退化为 0，**照样要下发**：建窗时已不设下限，跳过会让窗口完全没有下限。
///
/// - `win`: 目标窗口，量它当前的外框与客户区差值
/// - `min_w`: 最小客户区宽度，逻辑像素
/// - `min_h`: 最小客户区高度，逻辑像素
pub(super) fn reconcile_min_size(win: &WebviewWindow, min_w: f64, min_h: f64) {
    let (Ok(inner), Ok(outer)) = (win.inner_size(), win.outer_size()) else {
        return;
    };
    let dx = outer.width.saturating_sub(inner.width);
    let dy = outer.height.saturating_sub(inner.height);
    // 量到的差值是物理像素，换算回逻辑像素后与 min_w / min_h 一起设回去。
    // 必须用逻辑单位：tauri/tao 把它存成 `PixelUnit::Logical`，换到别的 DPI 显示器时会跟着
    // 缩放；物理像素则不会缩放
    let scale = win.scale_factor().unwrap_or(1.0);
    if scale <= 0.0 {
        return;
    }
    let width = min_w + dx as f64 / scale;
    let height = min_h + dy as f64 / scale;
    let _ = win.set_min_size(Some(tauri::LogicalSize::new(width, height)));
}

/// 把窗口的**运行时**客户区补到不小于注册表里的最小客户区
///
/// 存盘口径比运行时口径小一个标题栏，恢复几何时夹不了这个下限，只能在装饰激活之后补一次。
/// 调用点在显示窗口之前，用户看不到跳变。
///
/// - `win`: 目标窗口
/// - `min`: 注册表里的最小客户区尺寸，逻辑像素
pub(super) fn clamp_runtime_min_size(win: &WebviewWindow, min: MinSize) {
    let Ok(inner) = win.inner_size() else {
        return;
    };
    let want = min_inner_physical(min, win.scale_factor().unwrap_or(1.0));
    if inner.width >= want.width && inner.height >= want.height {
        return;
    }
    // set_size 落到 tao 的 set_inner_size，给的就是客户区尺寸，不用自己补边框带
    let _ = win.set_size(tauri::PhysicalSize::new(
        inner.width.max(want.width),
        inner.height.max(want.height),
    ));
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
pub(super) fn save_window_state(
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

/// 当前窗口因「摘掉原生装饰」而多出来、需要从 `inner` 里减掉的客户区尺寸，单位物理像素
///
/// 原理：`set_decorations(false)` 会**保持外框不变**、把标题栏那一段并进客户区。
/// 实测同一窗口外框恒为 984×678，`inset` 从 16×39 变成 16×9，客户区 +30。
/// 要存的口径是**带装饰时的客户区**，与建窗时 `.inner_size()` 一致，
/// 所以无装饰时得把这个增量减掉。
///
/// 增量 = 当前 `(外框 − 客户区)` 与**带装饰时** `(外框 − 客户区)` 的差：
/// 带装饰时该差值含标题栏，无装饰时不含，两者相减正好是标题栏那一段。
/// 带装饰的值从 [`DECORATED_INSET`] 取，建窗那一刻量的。
///
/// 所以：
/// - 带装饰时：当前 == 缓存 → 增量 0，本来就是对的口径，不动
/// - 无装饰时：当前比缓存小一个标题栏 → 增量 = 标题栏高度，减掉它
///
/// 不用写死 30：标题栏与边框带都随 DPI 变，这里全部现量。
///
/// - `uuid`: 窗口 uuid，用它取建窗时缓存下来的带装饰差值
/// - `window`: 目标窗口，量它当前的外框与客户区差值
///
/// # 返回值
///
/// 见 [`PhysicalDelta`]，即需要从 `inner` 里减掉的宽高增量，单位物理像素；
/// 量不到或没有缓存时宽高都为 0
pub(super) fn decoration_growth(uuid: &Uuid, window: &WebviewWindow) -> PhysicalDelta {
    let (Ok(inner), Ok(outer)) = (window.inner_size(), window.outer_size()) else {
        return PhysicalDelta::default();
    };
    let now_inset = PhysicalDelta {
        width: outer.width.saturating_sub(inner.width),
        height: outer.height.saturating_sub(inner.height),
    };

    let cached = DECORATED_INSET.read().unwrap().get(uuid).copied();
    let Some(decorated) = cached else {
        // 没缓存到，本进程没经过建窗路径：不折算，保持原值
        return PhysicalDelta::default();
    };

    PhysicalDelta {
        width: decorated.width.saturating_sub(now_inset.width),
        height: decorated.height.saturating_sub(now_inset.height),
    }
}

/// 子窗口贴角时与锚点窗口边缘留的间距，单位逻辑像素
const CHILD_GAP: f64 = 32.0;

/// 逻辑像素的尺寸
pub(super) struct SizeF {
    /// 宽
    pub(super) width: f64,
    /// 高
    pub(super) height: f64,
}

/// 逻辑像素的矩形
pub(super) struct RectF {
    /// 左边缘
    pub(super) x: f64,
    /// 上边缘
    pub(super) y: f64,
    /// 宽
    pub(super) width: f64,
    /// 高
    pub(super) height: f64,
}

/// 窗口落点，逻辑像素
pub(super) struct Placement {
    /// 左边缘
    pub(super) x: f64,
    /// 上边缘
    pub(super) y: f64,
}

/// 锚点窗口的四个角，按这个顺序试
#[derive(Clone, Copy)]
enum Corner {
    /// 左上
    LeftTop,
    /// 左下
    LeftBottom,
    /// 右上
    RightTop,
    /// 右下
    RightBottom,
}

/// 子窗口没有历史几何时的落点：叠在锚点窗口某个角的**内侧**
///
/// 依次试左上 / 左下 / 右上 / 右下，取第一个整窗都落在工作区内的；四个都放不下就用左上
/// 夹进工作区 —— 宁可贴边也不能把窗口开到屏幕外。间距见 `CHILD_GAP`，
/// 子窗口比锚点窗口大时会从那个角探出去。
///
/// - `anchor`: 锚点窗口的外框，逻辑像素
/// - `child`: 子窗口尺寸，逻辑像素
/// - `area`: 锚点所在显示器的工作区，逻辑像素，已排掉任务栏
///
/// # 返回值
///
/// 见 [`Placement`]
pub(super) fn child_placement(anchor: RectF, child: SizeF, area: RectF) -> Placement {
    const ORDER: [Corner; 4] = [
        Corner::LeftTop,
        Corner::LeftBottom,
        Corner::RightTop,
        Corner::RightBottom,
    ];

    let mut fallback = None;
    for corner in ORDER {
        let placement = corner_placement(corner, &anchor, &child);
        if fits(&placement, &child, &area) {
            return placement;
        }
        fallback.get_or_insert(placement);
    }

    let placement = fallback.unwrap_or(Placement {
        x: area.x,
        y: area.y,
    });
    // 子窗口比工作区还大时 min > max，clamp 会 panic，所以下限反过来兜一层
    Placement {
        x: placement
            .x
            .clamp(area.x, (area.x + area.width - child.width).max(area.x)),
        y: placement
            .y
            .clamp(area.y, (area.y + area.height - child.height).max(area.y)),
    }
}

/// 屏幕中心：主窗口没有历史几何时的落点
///
/// - `area`: 目标显示器的工作区，逻辑像素
/// - `child`: 窗口尺寸，逻辑像素
///
/// # 返回值
///
/// 见 [`Placement`]
pub(super) fn center_placement(area: RectF, child: SizeF) -> Placement {
    Placement {
        x: area.x + (area.width - child.width) / 2.0,
        y: area.y + (area.height - child.height) / 2.0,
    }
}

/// 某个角的内侧落点
fn corner_placement(corner: Corner, anchor: &RectF, child: &SizeF) -> Placement {
    let left = anchor.x + CHILD_GAP;
    let right = anchor.x + anchor.width - child.width - CHILD_GAP;
    let top = anchor.y + CHILD_GAP;
    let bottom = anchor.y + anchor.height - child.height - CHILD_GAP;

    match corner {
        Corner::LeftTop => Placement { x: left, y: top },
        Corner::LeftBottom => Placement { x: left, y: bottom },
        Corner::RightTop => Placement { x: right, y: top },
        Corner::RightBottom => Placement {
            x: right,
            y: bottom,
        },
    }
}

/// 整窗是否落在工作区内
fn fits(placement: &Placement, child: &SizeF, area: &RectF) -> bool {
    placement.x >= area.x
        && placement.y >= area.y
        && placement.x + child.width <= area.x + area.width
        && placement.y + child.height <= area.y + area.height
}

#[cfg(test)]
mod placement_tests {
    use super::*;

    /// 一个工作区矩形
    fn area() -> RectF {
        RectF {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        }
    }

    /// 主窗口居中：工作区与窗口都算一半
    #[test]
    fn center_uses_work_area() {
        let p = center_placement(
            area(),
            SizeF {
                width: 400.0,
                height: 300.0,
            },
        );
        assert_eq!((p.x, p.y), (760.0, 390.0));
    }

    /// 锚点在屏幕中间时走左上，间距是 CHILD_GAP
    #[test]
    fn child_prefers_left_top() {
        let anchor = RectF {
            x: 600.0,
            y: 300.0,
            width: 800.0,
            height: 400.0,
        };
        let p = child_placement(
            anchor,
            SizeF {
                width: 400.0,
                height: 300.0,
            },
            area(),
        );
        assert_eq!((p.x, p.y), (600.0 + CHILD_GAP, 300.0 + CHILD_GAP));
    }

    /// 左上会探出工作区时依次退：这个例子里只有右下放得下
    #[test]
    fn child_falls_back_to_a_corner_that_fits() {
        let anchor = RectF {
            x: 1500.0,
            y: 800.0,
            width: 400.0,
            height: 250.0,
        };
        let child = SizeF {
            width: 400.0,
            height: 300.0,
        };

        let p = child_placement(anchor, child, area());
        // 右下：右边 = 锚点右边 − 子窗口宽 − 间距，下边同理（这里下边会被夹到工作区里）
        assert!(p.x + 400.0 <= 1920.0, "右边不能探出工作区");
        assert!(p.y + 300.0 <= 1080.0, "下边不能探出工作区");
        assert_eq!(p.x, 1500.0 + 400.0 - 400.0 - CHILD_GAP);
    }

    /// 工作区比窗口还小时夹进工作区，不 panic、也不开到屏幕外
    #[test]
    fn child_clamps_into_work_area() {
        let small = RectF {
            x: 100.0,
            y: 100.0,
            width: 800.0,
            height: 600.0,
        };
        let p = child_placement(
            RectF {
                x: 200.0,
                y: 200.0,
                width: 300.0,
                height: 200.0,
            },
            SizeF {
                width: 1000.0,
                height: 800.0,
            },
            small,
        );
        assert_eq!((p.x, p.y), (100.0, 100.0));
    }
}

