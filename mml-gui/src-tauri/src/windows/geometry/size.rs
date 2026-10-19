//! 尺寸口径与收敛
//!
//! 装饰边框会随标题栏样式变化，历史几何与注册表里的最小尺寸
//! 都要按当前装饰差重新核对，否则会出现「改了最小尺寸而窗口纹丝不动」。


use tauri::WebviewWindow;
use uuid::Uuid;

use crate::windows::registry::{MinSize};
use super::state::{DECORATED_INSET};


/// 窗口客户区的宽高，单位物理像素，由注册表里的最小尺寸换算而来
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct PhysicalSize {
    /// 宽，单位物理像素
    pub(crate) width: u32,
    /// 高，单位物理像素
    pub(crate) height: u32,
}

/// 宽高两向的物理像素差值，用于「外框 − 客户区」这类差值在两处之间传递
///
/// 与 [`PhysicalSize`] 的区别在语义：那个是**尺寸**，这个是**差值** —— 为 0 只表示
/// 两处量到的一样，不表示宽高是 0。
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PhysicalDelta {
    /// 宽向差值，单位物理像素；两边相同时为 0
    pub(crate) width: u32,
    /// 高向差值，单位物理像素；两边相同时为 0
    pub(crate) height: u32,
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
pub(crate) fn min_inner_physical(min: MinSize, scale: f64) -> PhysicalSize {
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
pub(crate) fn reconcile_min_size(win: &WebviewWindow, min_w: f64, min_h: f64) {
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
pub(crate) fn clamp_runtime_min_size(win: &WebviewWindow, min: MinSize) {
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
pub(crate) fn decoration_growth(uuid: &Uuid, window: &WebviewWindow) -> PhysicalDelta {
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
