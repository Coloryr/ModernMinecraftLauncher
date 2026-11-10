//! 自绘标题栏的窗口控制
//!
//! 关掉系统装饰后，拖动 / 激活装饰 / 最小化 / 最大化 / 全屏 / 设标题都由前端标题栏调用。
//! 写在应用自己的命令里，不必往 capabilities 加窗口权限；关闭走 `open::window_close_window`，
//! 那条链路才会跑关闭保护与几何保存。

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::WebviewWindow;

use crate::windows::geometry::{clamp_runtime_min_size, reconcile_min_size};
use crate::windows::registry::min_size_of_label;
use super::shared::main_window_label;
#[cfg(target_os = "macos")]
use super::decorate::{
    TRAFFIC_LIGHTS_INSET_X, TRAFFIC_LIGHTS_INSET_Y, hide_native_traffic_lights, is_april_fools,
};

pub use self::fullscreen::window_fill_screen;

mod decorate;
pub(crate) mod fullscreen;

// ---------------- 自绘标题栏的窗口控制 ----------------
//
// 关掉系统装饰后，本段的动作由前端标题栏调用：拖动 / 激活装饰 / 最小化 /
// 最大化切换 / 查最大化 / 设标题。写在应用自己的命令里，而不用 JS 侧的
// `getCurrentWindow()`，这样不必往 capabilities/default.json 加窗口权限。
// 关闭不走这里——见 `window_close_window`，那条链路才会跑关闭保护与几何保存。

/// 开始拖动窗口，标题栏按下时调用，之后由系统接管
///
/// - `window`: 调用方窗口，由 tauri 注入
///
/// # 返回值
///
/// 已交给系统拖动返回 `Ok(())`，失败返回错误文案
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub fn window_start_dragging(window: WebviewWindow) -> Result<(), String> {
    window.start_dragging().map_err(|err| err.to_string())
}

// ---------------- 启动时的显示 ----------------

/// 下一次显示主窗口是否只显示、不抢前台；被 `take_startup_quiet` 消费一次即复位
static QUIET_STARTUP: AtomicBool = AtomicBool::new(false);

/// 启动时那次"不抢前台"是否已经排过；一个进程只排一次
static STARTUP_QUIET_TAKEN: AtomicBool = AtomicBool::new(false);

/// 标记下一次显示主窗口不要抢前台，setup 阶段建主窗口之前调一次
///
/// 应用由系统、脚本或开发期的 watcher 拉起来时，把前台从用户正在用的窗口抢走会打断操作，
/// 所以**进程启动后的第一次**主窗口显示只显示、不激活，之后照常拿焦点。
/// 关窗后再从托盘唤回的主窗口不算：那次是用户主动点的，本就该拿到前台。
pub(crate) fn quiet_main_window_startup() {
    if !STARTUP_QUIET_TAKEN.swap(true, Ordering::SeqCst) {
        QUIET_STARTUP.store(true, Ordering::SeqCst);
    }
}

/// 显示窗口：启动时的第一扇主窗口只显示、不激活，其余照旧显示并拿焦点
///
/// - `window`: 要显示的窗口
///
/// # 返回值
///
/// 显示成功返回 `Ok(())`，失败返回错误文案
fn show_window(window: &WebviewWindow) -> Result<(), String> {
    if take_startup_quiet(window.label()) {
        return show_without_activate(window);
    }
    window.show().map_err(|err| err.to_string())?;
    // 显示不等于到前台：不抢一次焦点，新窗会压在发起它的那个窗口后面，看着像点了没反应
    window.set_focus().map_err(|err| err.to_string())
}

/// 这一次显示要不要走"不抢前台"
///
/// 只有**进程启动后主窗口的第一次**显示算数；问过一次就把标记消费掉，
/// 其它窗口（以及之后同一次运行里再显示的主窗口）照常拿焦点。
///
/// - `label`: 要显示的窗口标签
///
/// # 返回值
///
/// 要走"不抢前台"返回 `true`
pub(crate) fn take_startup_quiet(label: &str) -> bool {
    label == main_window_label() && QUIET_STARTUP.swap(false, Ordering::SeqCst)
}

/// 显示窗口但不激活：先记下当前前台，显示之后还回去
///
/// **必须走 tao 的 `show()`**：直接 `ShowWindow` 只改原生可见性，tao 内部仍认为窗口不可见，
/// 之后任何一次窗口标志变更（最小化 / 最大化 / 设为无装饰）都会顺手把窗口藏起来 ——
/// 表现为任务栏图标消失、进程还在。
///
/// - `window`: 要显示的窗口
///
/// # 返回值
///
/// 排进窗口线程失败返回错误文案；显示本身在窗口线程上执行
#[cfg(windows)]
fn show_without_activate(window: &WebviewWindow) -> Result<(), String> {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SetForegroundWindow};

    let win = window.clone();
    // 放窗口线程上做：show() 在那里同步生效，紧接着还前台，中间插不进别的窗口操作
    window
        .run_on_main_thread(move || {
            // SAFETY: 只读当前前台句柄，不解引用；调用期间本窗口仍存活
            let previous = unsafe { GetForegroundWindow() };
            if win.show().is_err() {
                return;
            }
            // show() 会先激活本窗口；启动时不打断用户正在用的窗口，把前台还回去
            if !previous.is_invalid() {
                // SAFETY: 句柄由系统给出；本进程刚拿到前台，此刻改回原窗口是允许的
                unsafe {
                    let _ = SetForegroundWindow(previous);
                }
            }
        })
        .map_err(|err| {
            // 排不进窗口线程：退回普通显示，宁可抢一次前台也不能让主窗口不显示
            let _ = window.show();
            err.to_string()
        })
}

/// 其它平台没有"显示但不激活"的等价开关，退化成只显示、不主动拿焦点
///
/// - `window`: 要显示的窗口
///
/// # 返回值
///
/// 显示成功返回 `Ok(())`，失败返回错误文案
#[cfg(not(windows))]
fn show_without_activate(window: &WebviewWindow) -> Result<(), String> {
    window.show().map_err(|err| err.to_string())
}

/// 激活插件装饰并显示窗口；所有窗口都走插件装饰，建窗逻辑见 `lifecycle/create.rs`
///
/// 窗口建出来时是 `decorations: true` + 隐藏的。Linux 在建窗后先切到客户端装饰，
/// 见 `create.rs` 的 `force_client_side_decoration`；插件激活仍负责控件与生命周期。

///
/// - Windows：自绘装饰接管标题栏右侧，Win11 贴靠布局挂在最大化按钮上
/// - macOS：插件摆的是**原生红黄绿**，位置要在激活之后、显示之前用
///   `set_traffic_lights_inset` 定好，见插件 docs/platforms.md
///
/// 失败时退回原生 frame 并照样显示窗口：宁可露出系统标题栏，也不能让主窗口打不开。
///
/// - `window`: 调用方窗口，由 tauri 注入
///
/// # 返回值
///
/// 成功返回 `"custom"`，表示装饰已由插件接管；失败返回带原因的 `Err`
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub async fn window_activate_decoration(window: WebviewWindow) -> Result<&'static str, String> {
    use tauri_plugin_decoration::WebviewWindowExt;

    if let Err(error) = window.activate_decoration().await {
        // 回退：先确保原生 frame 还在，再收起它
        let _ = window.restore_decoration().await;
        // 原生标题栏不能留：它与自绘顶栏会叠成两条。
        // 装饰没接管就去掉原生 frame，红黄绿改由前端画，见 useWindowDecoration
        let _ = window.set_decorations(false);
        // 换成无边框后量到的差值只剩边框带，正好是窗口该有的下限
        if let Some(min) = min_size_of_label(window.label()) {
            reconcile_min_size(&window, min.width, min.height);
        }
        mml_log::error(
            crate::log_text::LogText::DecorationActivateFailed {
                label: window.label().to_string(),
                error: error.to_string(),
            }
            .text(),
        );
        let _ = show_window(&window);
        crate::windows::effects::apply(&window, &crate::gui_config::get().client.window_effect);
        return Err(format!("decoration activate failed: {error}"));
    }

    // macOS 的红黄绿是 AppKit 原生按钮，位置必须在激活之后、显示之前定好，
    // 见插件 docs/platforms.md
    #[cfg(target_os = "macos")]
    {
        if is_april_fools() {
            // 标题栏按另一套样式展示时窗口按钮也换边：原生红黄绿要藏掉，
            // 否则会和前端画的那套并存
            if let Err(error) = hide_native_traffic_lights(&window) {
                mml_log::info(crate::log_text::LogText::TrafficLightsFailed { error }.text());
            }
        } else if let Err(error) = window
            .set_traffic_lights_inset(TRAFFIC_LIGHTS_INSET_X, TRAFFIC_LIGHTS_INSET_Y)
            .await
        {
            mml_log::info(
                crate::log_text::LogText::TrafficLightsFailed {
                    error: error.to_string(),
                }
                .text(),
            );
        }
    }

    // 装饰已摘掉，此刻量到的「外框 − 客户区」才是那圈边框带；
    // 建窗时量到的是边框带 + 标题栏，拿它当客户区下限会把窗口顶高，所以建窗时**不设**下限，
    // 到这里才设；激活失败退回原生 frame 的那条分支另设一次
    if let Some(min) = min_size_of_label(window.label()) {
        reconcile_min_size(&window, min.width, min.height);
        // 存盘口径是带装饰的客户区，比运行时口径小一个标题栏，恢复几何时夹不了这个下限，
        // 只能等装饰摘掉、inner_size() 变成运行时口径后在这里补一次
        clamp_runtime_min_size(&window, min);
    }

    show_window(&window)?;
    // 装饰激活里的"设下限"会落到 tao 的 set_inner_size，而它会把最大化标志清掉并把窗口还原：
    // "上次是最大化"的窗口开出来就只有那块尺寸、并不是最大化状态，标题栏图标对不上，
    // 双击标题栏也像没反应。这里按存盘状态补一次
    if let Some(uuid) = crate::windows::registry::uuid_of_label(window.label())
        && crate::windows::geometry::window_state_for(&uuid).is_some_and(|state| state.maximized)
    {
        let _ = window.maximize();
    }
    // 材质要在窗口显示之后铺：隐藏状态下铺的会被随后 activate_decoration 改窗口样式一起丢掉
    crate::windows::effects::apply(&window, &crate::gui_config::get().client.window_effect);
    Ok("custom")
}

/// 打开开发者工具
///
/// 测试版与预发布版可用；正式发布没有把 DevTools 编进去（见 Cargo.toml 的
/// `prerelease = ["tauri/devtools"]`），这里返回错误。
///
/// 三个平台都是**独立窗口**：Windows 是 WebView2 的 DevTools 窗口，macOS 是
/// Web Inspector 窗口，Linux 是 WebKitGTK 的 inspector（默认不贴靠）。
///
/// - `window`: 调用方窗口，由 tauri 注入
///
/// # 返回值
///
/// 打开成功返回 `Ok(())`；正式发布返回错误文案
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub fn window_open_devtools(window: WebviewWindow) -> Result<(), String> {
    #[cfg(any(debug_assertions, feature = "prerelease"))]
    {
        window.open_devtools();
        Ok(())
    }
    #[cfg(not(any(debug_assertions, feature = "prerelease")))]
    {
        let _ = window;
        Err("DevTools are only available in debug and prerelease builds".to_string())
    }
}

/// 最小化窗口
///
/// - `window`: 调用方窗口，由 tauri 注入
///
/// # 返回值
///
/// 成功返回 `Ok(())`，失败返回错误文案
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub fn window_minimize(window: WebviewWindow) -> Result<(), String> {
    window.minimize().map_err(|err| err.to_string())
}

/// 最大化 / 还原取反，返回切换后的状态
///
/// - `window`: 调用方窗口，由 tauri 注入
///
/// # 返回值
///
/// 切换**之后**的最大化状态，`true` 表示已最大化；查询或切换失败返回错误文案
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub fn window_toggle_maximize(window: WebviewWindow) -> Result<bool, String> {
    let maximized = window.is_maximized().map_err(|err| err.to_string())?;
    if maximized {
        window.unmaximize().map_err(|err| err.to_string())?;
    } else {
        window.maximize().map_err(|err| err.to_string())?;
    }
    Ok(!maximized)
}

/// 当前是否最大化，标题栏换图标用；`Win+↑`、系统贴靠等外部操作也会改它
///
/// - `window`: 调用方窗口，由 tauri 注入
///
/// # 返回值
///
/// 是否最大化；查询失败按未最大化处理
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub fn window_is_maximized(window: WebviewWindow) -> bool {
    window.is_maximized().unwrap_or(false)
}

/// 设置窗口全屏，标题栏右键菜单的「全屏」用
///
/// 全屏盖住任务栏；退出时由系统恢复进入全屏之前的摆放，那时原本最大化的话仍最大化。
///
/// - `window`: 调用方窗口，由 tauri 注入
/// - `fullscreen`: 目标状态，true 进入全屏，false 退出
///
/// # 返回值
///
/// 设置成功返回 `Ok(())`，失败返回错误文案
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub fn window_set_fullscreen(window: WebviewWindow, fullscreen: bool) -> Result<(), String> {
    window
        .set_fullscreen(fullscreen)
        .map_err(|err| err.to_string())
}

/// 当前是否全屏，标题栏换图标用
///
/// - `window`: 调用方窗口，由 tauri 注入
///
/// # 返回值
///
/// 是否全屏；查询失败按未全屏处理
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub fn window_is_fullscreen(window: WebviewWindow) -> bool {
    window.is_fullscreen().unwrap_or(false)
}

/// 设置调用方窗口的原生标题，任务栏 / Alt+Tab 显示
///
/// 窗口创建时统一叫 `names::MML`；自绘标题栏文案由前端持有，已 i18n，
/// 因此由前端在标题变化时同步过来，语言切换后原生标题也能跟随
///
/// macOS 上还要补一次红黄绿的位置。位置只在窗口激活时下发过一次，而任何让 AppKit
/// 重算标题栏容器的操作（改标题就是其中之一）之后，原生按钮会停回系统默认位置，
/// 直到下一次窗口尺寸变化或重新获得焦点；这里补发的成本只有一次主线程往返。
///
/// - `window`: 调用方窗口，由 tauri 注入
/// - `title`: 新的原生标题，用前端已 i18n 好的文案
///
/// # 返回值
///
/// 设置成功返回 `Ok(())`，失败返回错误文案
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub async fn window_set_title(window: WebviewWindow, title: String) -> Result<(), String> {
    window.set_title(&title).map_err(|err| err.to_string())?;

    // 见上：标题一变就把原生按钮摆回自绘标题栏里（另一套标题栏样式下原生按钮是藏掉的，不用摆）
    #[cfg(target_os = "macos")]
    {
        use tauri_plugin_decoration::WebviewWindowExt;

        if !is_april_fools()
            && let Err(error) = window
                .set_traffic_lights_inset(TRAFFIC_LIGHTS_INSET_X, TRAFFIC_LIGHTS_INSET_Y)
                .await
        {
            mml_log::info(
                crate::log_text::LogText::TrafficLightsFailed {
                    error: error.to_string(),
                }
                .text(),
            );
        }
    }

    Ok(())
}
