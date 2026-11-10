//! 铺满工作区
//!
//! 工作区口径与 XWayland 的 `_NET_WORKAREA` 读取都在这里；铺满不等于最大化。

use tauri::WebviewWindow;

/// 工作区是否与显示器整块重合
///
/// 平台不提供任务栏 / Dock 占用信息时两者完全一致（Wayland 的 GDK workarea 就是这样）
///
/// - `work`: 当前显示器的工作区
/// - `position`: 显示器在虚拟桌面里的左上角
/// - `size`: 显示器的像素尺寸
///
/// # 返回值
///
/// 两者位置与尺寸都相同时为 `true`
pub(crate) fn work_area_is_full_monitor(
    work: &tauri::PhysicalRect<i32, u32>,
    position: &tauri::PhysicalPosition<i32>,
    size: &tauri::PhysicalSize<u32>,
) -> bool {
    work.position == *position && work.size == *size
}

/// 取当前显示器真正可用的工作区
///
/// Wayland 下 GDK 只给整屏；走 XWayland 时读 `_NET_WORKAREA` 才能扣掉 Dock / 顶栏（仅单显示器）。
///
/// 原生 Wayland 不能绝对定位、也没有工作区协议；当前 Tauri 在 Linux 上只有 X11 后端。
///
/// - `window`: 调用方窗口，用来判断显示器数量
/// - `monitor`: 当前显示器
///
/// # 返回值
///
/// 工作区矩形，物理像素
#[cfg_attr(not(target_os = "linux"), allow(unused_variables))]
fn effective_work_area(
    window: &WebviewWindow,
    monitor: &tauri::Monitor,
) -> tauri::PhysicalRect<i32, u32> {
    let work = *monitor.work_area();
    if !work_area_is_full_monitor(&work, monitor.position(), monitor.size()) {
        return work;
    }
    #[cfg(target_os = "linux")]
    {
        let single = window
            .available_monitors()
            .map(|monitors| monitors.len() == 1)
            .unwrap_or(false);
        if single {
            if let Some(rect) = x11_work_area(monitor.scale_factor(), monitor.size().width) {
                return rect;
            }
        }
    }
    work
}

/// 读 XWayland 根窗口的 `_NET_WORKAREA`
///
/// - `scale`: GDK 的缩放系数，X11 坐标可能还是逻辑像素
/// - `physical_width`: 显示器物理宽度，用来判断 X11 矩形是逻辑还是物理
///
/// # 返回值
///
/// 读不到、或结果为空时返回 `None`
#[cfg(target_os = "linux")]
fn x11_work_area(scale: f64, physical_width: u32) -> Option<tauri::PhysicalRect<i32, u32>> {
    use std::os::raw::{c_int, c_long, c_uchar, c_ulong};

    use x11_dl::xlib;

    // SAFETY: 只读根窗口属性，读完立即 XFree / XCloseDisplay；失败路径逐项判空
    unsafe {
        let xlib = xlib::Xlib::open().ok()?;
        let display = (xlib.XOpenDisplay)(std::ptr::null());
        if display.is_null() {
            return None;
        }
        let root = (xlib.XDefaultRootWindow)(display);
        let atom = (xlib.XInternAtom)(display, c"_NET_WORKAREA".as_ptr(), 0);
        if atom == 0 {
            (xlib.XCloseDisplay)(display);
            return None;
        }

        let mut actual_type: xlib::Atom = 0;
        let mut actual_format: c_int = 0;
        let mut nitems: c_ulong = 0;
        let mut bytes_after: c_ulong = 0;
        let mut prop: *mut c_uchar = std::ptr::null_mut();
        let status = (xlib.XGetWindowProperty)(
            display,
            root,
            atom,
            0,
            4,
            0,
            6, // XA_CARDINAL
            &mut actual_type,
            &mut actual_format,
            &mut nitems,
            &mut bytes_after,
            &mut prop,
        );

        let mut rect = None;
        if status == 0 && !prop.is_null() && actual_format == 32 && nitems >= 4 {
            // Xlib 把 32 位 CARDINAL 存在 long 里，64 位下每个占 8 字节
            let values = std::slice::from_raw_parts(prop as *const c_long, 4);
            let width = values[2] as u32;
            let height = values[3] as u32;
            if width > 0 && height > 0 {
                // XWayland 的根窗口通常是逻辑像素；与物理宽度对不上时按物理算
                let logical = (width as f64) <= physical_width as f64 / scale + 1.0;
                let factor = if logical { scale } else { 1.0 };
                rect = Some(tauri::PhysicalRect {
                    position: tauri::PhysicalPosition::new(
                        (values[0] as f64 * factor) as i32,
                        (values[1] as f64 * factor) as i32,
                    ),
                    size: tauri::PhysicalSize::new(
                        (width as f64 * factor) as u32,
                        (height as f64 * factor) as u32,
                    ),
                });
            }
        }
        if !prop.is_null() {
            (xlib.XFree)(prop as *mut std::os::raw::c_void);
        }
        (xlib.XCloseDisplay)(display);
        rect
    }
}

/// 把窗口铺满当前显示器的工作区：客户区左上角对齐工作区原点，尺寸取工作区
///
/// 与最大化不是一回事：这里**不进入系统最大化状态**，只是把窗口摆好，之后照常可拖动 / 缩放。
/// 工作区是屏幕去掉任务栏那一条；尺寸按**客户区**算，那圈系统阴影边框从外框让出去，
/// 所以窗口内容正好贴住工作区四边，不额外含标题栏。
/// 原本全屏或最大化时先退出来，否则系统会按那两个状态把窗口改回去。
/// Wayland 下 GDK 的 workarea 是整屏，改读 XWayland 的 `_NET_WORKAREA` 补回 Dock / 面板。
///
/// - `window`: 调用方窗口，由 tauri 注入
///
/// # 返回值
///
/// 摆放成功返回 `Ok(())`，失败返回错误文案
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub fn window_fill_screen(window: WebviewWindow) -> Result<(), String> {
    if window.is_fullscreen().unwrap_or(false) {
        window
            .set_fullscreen(false)
            .map_err(|err| err.to_string())?;
    }
    let monitor = window
        .current_monitor()
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "window has no current monitor".to_string())?;
    let work = effective_work_area(&window, &monitor);

    if window.is_maximized().unwrap_or(false) {
        window.unmaximize().map_err(|err| err.to_string())?;
    }

    // 无边框窗口仍带一圈系统阴影边框：外框比客户区大，且左边、上边各是一段。
    // 按客户区对齐，把外框往左上让出这两段，客户区才正好落在工作区原点上
    let (left, top) = match (window.outer_position(), window.inner_position()) {
        (Ok(outer), Ok(inner)) => (inner.x - outer.x, inner.y - outer.y),
        _ => (0, 0),
    };

    window
        .set_position(tauri::PhysicalPosition::new(
            work.position.x - left,
            work.position.y - top,
        ))
        .map_err(|err| err.to_string())?;
    window
        .set_size(tauri::PhysicalSize::new(work.size.width, work.size.height))
        .map_err(|err| err.to_string())
}
