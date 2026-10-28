//! 建窗与初始摆放
//!
//! 尺寸与最小尺寸取自注册表，位置取自 window_save.json，
//! 都要先做一次合法性收敛。

use crate::log_text::LogText;
use std::{path::PathBuf, sync::Mutex};

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::listens;
use mml_names::names;
use uuid::Uuid;

use super::models::{OPEN_WINDOWS, WINDOW_MODELS, ensure_window_model};
use crate::windows::block;
use crate::windows::geometry::{
    Placement, RectF, SizeF, center_placement, child_placement, remember_decorated_inset,
    save_window_state, window_state_for,
};
use crate::windows::registry::{
    ADD_MODPACK_WINDOW_UUID, BLOCK_WINDOW_UUID, EXPORT_WINDOW_UUID, MAIN_WINDOW_UUID,
    SERVER_PACK_WINDOW_UUID, min_size_of_label,
};

/// 窗口内允许导航的地址：启动器自身页面与本地自定义协议
///
/// - `url`: 导航目标
///
/// # 返回值
///
/// 允许在 webview 内导航时为 `true`
fn is_app_url(url: &tauri::Url) -> bool {
    if matches!(url.scheme(), "tauri" | "about" | "data") {
        return true;
    }
    if url.scheme() != "http" && url.scheme() != "https" {
        return false;
    }
    let host = url.host_str().unwrap_or_default();
    host == "localhost"
        || host == "127.0.0.1"
        || host == "tauri.localhost"
        || host.ends_with(".localhost")
}

/// 窗口是否拒绝本次关闭
///
/// - 主窗口：没有实例在跑时关窗就是退出应用，仍有下载任务时拒绝，
///   前端弹确认框，确认后停止下载再退出
/// - 添加实例 / 下载整合包窗口：模型侧关闭保护，查询数据期间拒绝
/// - 导出 / 服务器包窗口：打包进行中拒绝
///
/// 其余窗口不保护。下载窗口本身也不拦。
///
/// - `uuid`: 窗口 uuid
///
/// # 返回值
///
/// 拒绝本次关闭时为真
pub(crate) fn close_guarded(uuid: &Uuid) -> bool {
    // 下载窗口：**不拦**。它只是个查看器，关掉不影响下载（任务在后台跑），
    // 进度从标题栏的下载指示器进来还能看；拦着只会让用户以为非关不可

    // 主窗口：有实例在跑时退回后台，下载在后台继续跑，关窗不会停它，不必拦；
    // 没有实例在跑时关窗就是退出应用，仍按老规矩先拒绝、由前端问一句
    if *uuid == MAIN_WINDOW_UUID {
        return !crate::tray::has_running_instance() && !mml_downloader::get_tasks().is_empty();
    }

    // 方块窗口：渲染进行中拒绝关闭 —— 渲染任务虽独立于窗口存活，
    // 但关窗后进度无从展示，用户也会误以为渲染被中断
    if *uuid == BLOCK_WINDOW_UUID {
        return block::render_running();
    }

    // 导出窗口：打包进行中拒绝关闭，由前端问一句"取消导出并关窗"
    if *uuid == EXPORT_WINDOW_UUID || *uuid == SERVER_PACK_WINDOW_UUID {
        return crate::windows::export::run::running();
    }

    let binding = WINDOW_MODELS.read().unwrap();
    let Some(model) = binding.get(uuid) else {
        return false;
    };
    let model = model.clone();
    drop(binding);
    let Ok(model) = model.downcast::<Mutex<crate::windows::add::AddWindowModel>>() else {
        return false;
    };
    model.lock().unwrap().close_guard()
}

/// 下载整合包窗口开 / 关时广播一次整合包总览
///
/// 总览里的 `window_open` 决定主窗口标题栏还要不要挂下载 / 整合包那两个进度指示器：
/// 那个窗口开着就只在它那边显示，关掉之后才回到主页面。
///
/// - `app`: 应用句柄，广播用它
pub(super) fn notify_modpack_window(app: &AppHandle) {
    let dto = crate::windows::modpack_task::build_status(app);
    crate::windows::modpack_task::emit_add_modpack_status(app, dto);
}

/// 关闭被拒绝事件：窗口处于关闭保护时前端弹提示说明原因
///
/// - `window`: 被拒绝关闭的窗口，事件只发给它
#[gui_macros::emit]
pub(crate) fn emit_close_blocked(window: &tauri::Window<tauri::Wry>) {
    // 负载带上被拦窗口的标签：前端只认发给自己的那一条，别的窗口不该跟着弹
    let label = window.label().to_string();
    let _ = window.emit_to(window.label(), listens::CLOSE_BLOCKED, label);
}

/// 创建或聚焦一个窗口
///
/// 注意：必须由 async 命令调用，同步命令在 Windows 主线程阻塞创建会冻结应用。
///
/// - `app`: 应用句柄，建窗用
/// - `label`: 窗口标签，形如 `mml-<kind>`
/// - `uuid`: 窗口 uuid，句柄表与几何状态都按它登记
/// - `url_path`: webview 加载的路径，相对应用源，通常为 `index.html`；需要带参数时传
///   `index.html?...` —— tauri 对恰好 `index.html` 的路径走裸 app URL 分支，其余字符串经
///   `Url::join` 拼接，query 可随真实 URL 带给前端
/// - `anchor`: 发起这次打开的那个窗口，**只在没有历史几何时**用来定子窗口的落点
///   （叠在它那个角的内侧）；主窗口自己开、或没有锚点时居中
///
/// # 返回值
///
/// 建好或聚焦到的窗口；建窗失败返回错误文案
pub(crate) fn create_window(
    app: &AppHandle,
    label: &str,
    uuid: &Uuid,
    url_path: &str,
    anchor: Option<&WebviewWindow>,
) -> Result<WebviewWindow, String> {
    // 已存在则聚焦，避免重复窗口
    if let Some(win) = app.get_webview_window(label) {
        win.set_focus().map_err(|err| err.to_string())?;
        // 句柄表可能因异常退出丢失，补记一份
        OPEN_WINDOWS.write().unwrap().insert(*uuid, win.clone());
        ensure_window_model(uuid);
        return Ok(win);
    }

    // 尺寸为 0 的条目按"没有历史几何"处理：serde 对缺失字段给 0，而恢复端不再夹最小尺寸
    let geom = window_state_for(uuid).filter(|g| g.width > 0 && g.height > 0);
    // 上次是最大化：按记录里的「最大化之前」几何开窗，开完再最大化，见 save_window_state
    let restore_maximized = geom.as_ref().map(|g| g.maximized).unwrap_or(false);

    // 最小尺寸随注册表条目走，各窗口布局不同、可压缩程度不同；无历史几何时直接以最小尺寸
    // 居中打开，下限则在装饰激活后按它设，见 window_activate_decoration
    // 按标签取：窗口族的标签不在静态表里，见 min_size_of_label
    let (min_w, min_h) = min_size_of_label(label)
        .map(|m| (m.width, m.height))
        .unwrap_or((600.0, 400.0));

    // 所有窗口都走插件装饰：必须以 decorations(true) 建出来，插件才能在原生 frame 上激活，
    // 激活前先藏起来。收益是 Win11 贴靠布局（矩形由前端量好交给插件，见 decoration.ts），
    // 以及 macOS 上由插件摆位的原生红黄绿，见插件 docs/platforms.md

    // 建窗时**不设**最小尺寸：此刻窗口是带装饰口径，最小客户区加上边框带算出的下限是外框 639，
    // 比拉到最小的窗口（外框 608）还高；而 tao 的 set_min_inner_size 会把当前尺寸立刻重设一遍，
    // Windows 借这次变更按下限把窗口顶上去，于是每开一次就比用户拉到的位置高一截。
    // 下限改在装饰激活后设，见 window_activate_decoration

    // webview 铺不透明暗色底，避免加载首帧透出桌面，与暗色主题 --bg 一致
    let builder = WebviewWindowBuilder::new(app, label, WebviewUrl::App(url_path.into()))
        .title(names::MML)
        // 原生 frame 起步 + 隐藏：装饰激活成功后再显示，失败则退回原生 frame 显示
        .decorations(true)
        // 关掉框架 wry 自己那份拖放实现：它会顶掉 WebView2 的拖放目标且只认文件，网页上
        // 拖下来的卡片连 dragenter 都收不到。关掉后 DOM 拖放全功能，路径由 file_drop 补上
        .disable_drag_drop_handler()
        .visible(false);

    // macOS：标题栏用 Overlay 并藏掉标题文字，内容才铺得到窗口顶部、红黄绿能浮在自绘顶栏上。
    // 插件摆的就是这几个原生按钮（见 docs/platforms.md），不开 Overlay 就变成两条栏：
    // 原生标题栏一条 + 自绘顶栏一条
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);

    // 下载资源 / 整合包两个窗口只承载启动器页面：正文里的外链由前端拦下转给系统浏览器，
    // 这里再兜一层，任何外部导航都不允许把窗口带走
    let builder = if matches!(label, "mml-add_resource" | "mml-add_modpack") {
        builder.on_navigation(is_app_url)
    } else {
        builder
    };

    // 拖放路径桥的脚本挂在**建窗期**：initialization_script 在导航开始前就注册好，每个新建
    // 文档都会执行。放到 webview ready 回调里注入对初始文档无效 —— 那个 API 只对之后新建
    // 的文档生效，初始文档已经建好，脚本永远不执行（文件拖入会一直拿不到路径）
    #[cfg(windows)]
    let builder = builder.initialization_script(crate::windows::file_drop::INJECT_SCRIPT);

    // 不指定 WebView2 用户数据目录时 Tauri 会强制落到 `%LOCALAPPDATA%\<identifier>\`，界面数据就
    // 散在系统盘；指到 `<cache>/webview` 后跟运行目录走，cache 已由下载器 init 建好，这里只兜一次
    let builder = match webview_data_dir() {
        Some(dir) => {
            if let Err(err) = std::fs::create_dir_all(&dir) {
                // 建不出来不致命：退回 Tauri 的默认目录总比开不了窗好
                mml_log::error(
                    LogText::WebviewDataDirFailed {
                        dir: dir.display().to_string(),
                        error: err.to_string(),
                    }
                    .text(),
                );
                builder
            } else {
                builder.data_directory(dir)
            }
        }
        None => builder,
    };

    // 只打开窗口透明，**不要**设 background_color：tao 在 WM_ERASEBKGND 里用 CreateSolidBrush
    // 填客户区，alpha 会被丢掉 —— 设成 #14161a00 实际是刷一层不透明深色，把 DWM 合成在
    // 窗口背后的材质整片盖住（实测窗内留白恒为 #14161a，与壁纸无关）。
    // 界面自己的底色是半透明的（见 base.css 的 body），材质没生效时也看得清
    // 窗口透明只为材质服务，而材质只有 Windows 提供（云母 / 亚克力）：
    // 其余平台一律不透明，否则 macOS 上会没材质也开透明，界面看着像没画出来
    #[cfg(windows)]
    let transparent = crate::gui_config::get().client.window_effect != "none";
    #[cfg(not(windows))]
    let transparent = false;
    let builder = builder.transparent(transparent);
    let win = match geom {
        Some(g) => {
            // 恢复端 builder.position / inner_size 用逻辑像素，保存端是物理像素：按目标
            // 显示器的缩放系数换算，否则带缩放的屏幕上每开一次窗都乘一遍，窗口逐次变大
            let scale = app
                .monitor_from_point(g.x as f64, g.y as f64)
                .ok()
                .flatten()
                .or_else(|| app.primary_monitor().ok().flatten())
                .map(|m| m.scale_factor())
                .unwrap_or(1.0);

            // 这里**不夹**最小客户区：存的是带装饰口径，比运行时口径小一个标题栏，夹会把它
            // 顶上去。下限由 clamp_runtime_min_size 在装饰激活后按运行时口径补
            builder
                .inner_size(g.width as f64 / scale, g.height as f64 / scale)
                .position(g.x as f64 / scale, g.y as f64 / scale)
                .build()
        }
        // 没有历史几何：主窗口在屏幕中心，子窗口叠在锚点窗口那个角的内侧
        None => {
            let size = SizeF {
                width: min_w,
                height: min_h,
            };
            match initial_placement(app, uuid, anchor, &size) {
                Some(p) => builder.inner_size(min_w, min_h).position(p.x, p.y).build(),
                // 拿不到显示器（几乎不可能）：退回 tao 自己的居中
                None => builder.inner_size(min_w, min_h).center().build(),
            }
        }
    };
    let win = win.map_err(|e| e.to_string())?;

    // 任务栏与 Alt+Tab 的图标固定用程序自己的：桌面快捷方式带的是实例图标（见
    // windows::main::instance::shortcut），从它启动时 Windows 会把那个图标带进任务栏
    if let Some(icon) = app.default_window_icon() {
        let _ = win.set_icon(icon.clone());
    }

    // 材质不在这里铺：此刻窗口还是隐藏的，随后 activate_decoration 还会改窗口样式。
    // 按 demo 的做法等窗口显示出来再铺，见 window_activate_decoration

    // 必须在**这一刻**量：此时窗口还是 decorations(true)，差值包含原生标题栏；插件激活摘掉
    // 装饰后标题栏并进客户区就再也量不到它，而关窗保存正发生在那个状态
    remember_decorated_inset(uuid, &win);

    OPEN_WINDOWS.write().unwrap().insert(*uuid, win.clone());
    ensure_window_model(uuid);
    // 下载整合包窗口开出来就广播一次总览：主窗口据此把下载 / 整合包两个进度指示器让给它
    if *uuid == ADD_MODPACK_WINDOW_UUID {
        notify_modpack_window(app);
    }
    // 开窗即记录初始几何，文件始终反映当前所有窗口，后续缩放/移动会持续更新。
    // 带上 restore_maximized：下面 maximize() 触发的 Resized 会被跳过，最大化时不记几何，
    // 否则文件里的最大化标志会被这次初始保存清掉。
    let _ = save_window_state(uuid, &win, restore_maximized);
    if restore_maximized {
        let _ = win.maximize();
    }
    Ok(win)
}

/// 没有历史几何时的落点
///
/// 主窗口取所在显示器的**屏幕中心**；子窗口叠在锚点窗口某个角的**内侧**，见
/// [`child_placement`]。保存过位置的窗口根本不走这里，见 [`create_window`]。
///
/// - `app`: 应用句柄，取显示器用
/// - `uuid`: 目标窗口 uuid，用它认出主窗口
/// - `anchor`: 发起这次打开的那个窗口；没有它（主窗口自己开、或窗口已不在）就居中
/// - `size`: 目标窗口尺寸，逻辑像素
///
/// # 返回值
///
/// 见 [`Placement`]；拿不到显示器时返回 `None`，调用方退回居中
pub(super) fn initial_placement(
    app: &AppHandle,
    uuid: &Uuid,
    anchor: Option<&WebviewWindow>,
    size: &SizeF,
) -> Option<Placement> {
    // 换算用锚点所在显示器的缩放系数：窗口就开在那儿，与恢复历史几何那条路同一口径
    let monitor = anchor
        .and_then(|win| win.current_monitor().ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten())?;
    let scale = monitor.scale_factor();
    let work = monitor.work_area();
    let area = RectF {
        x: work.position.x as f64 / scale,
        y: work.position.y as f64 / scale,
        width: work.size.width as f64 / scale,
        height: work.size.height as f64 / scale,
    };
    let size = SizeF {
        width: size.width,
        height: size.height,
    };

    if *uuid == MAIN_WINDOW_UUID {
        return Some(center_placement(area, size));
    }

    // 锚点窗口的外框也换到逻辑像素；取不到就居中
    let Some(anchor_rect) = anchor.and_then(|win| window_rect(win, scale)) else {
        return Some(center_placement(area, size));
    };
    Some(child_placement(anchor_rect, size, area))
}

/// 取窗口外框，换算成逻辑像素
///
/// - `win`: 目标窗口
/// - `scale`: 该窗口所在显示器的缩放系数
///
/// # 返回值
///
/// 见 [`RectF`]；查询失败返回 `None`
pub(super) fn window_rect(win: &WebviewWindow, scale: f64) -> Option<RectF> {
    let position = win.outer_position().ok()?;
    let size = win.outer_size().ok()?;
    Some(RectF {
        x: position.x as f64 / scale,
        y: position.y as f64 / scale,
        width: size.width as f64 / scale,
        height: size.height as f64 / scale,
    })
}

/// WebView2 用户数据目录，EBWebView 的落点
///
/// 路径是 `<cache>/webview`，`<cache>` 即下载器的临时目录
/// `mml_downloader::get_cache_path()` = `<运行目录>/cache`，见 names::CACHE_DIR；
/// 缓存路径的唯一来源是下载器，这里**不**自己拼运行目录。
///
/// # 返回值
///
/// 该目录；签名上的 `Option` 只是沿用调用方"拿不到就退回 Tauri 默认目录"的分支，
/// 实际上 `mml_downloader::init` 由 `mml_core::init` 在 `main` 里先跑完，窗口一律在
/// `run()` 的 setup 之后才建，所以**总是** `Some`。注意 `get_cache_path()` 自己在未
/// 初始化时是 `unwrap()` **panic**，见 mml-downloader/src/lib.rs —— 那条兜底分支
/// 真到了也接不住。
pub(crate) fn webview_data_dir() -> Option<PathBuf> {
    let cache = mml_downloader::get_cache_path();
    Some(cache.join(names::WEBVIEW_DIR))
}
