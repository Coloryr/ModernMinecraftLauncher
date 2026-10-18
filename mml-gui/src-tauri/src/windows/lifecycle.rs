//! 窗口生命周期：句柄表 / 模型表 / 建窗 / 开关窗 / 窗口事件
//!
//! 从 `windows/mod.rs` 拆出来的。这里管"窗口从生到死"：建窗，含几何恢复 + 装饰时机；
//! 句柄与模型的登记清理、关闭保护，以及 `on_window_event` 里的关闭 / 销毁 / 缩放事件。
//! 最小尺寸不在这里设，原因见 [`create_window`] 的说明。
//!
//! **`window_*` 命令留在 `mod.rs`**：`bindings.ts` 的组键取"来源 .rs 模块最后一段"，
//! 命令搬文件会让组键从 `windows` 变成别的名字，前端会断，见 AGENTS.md §4。

use crate::log_text::LogText;
use std::{
    any::Any,
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, LazyLock, Mutex, RwLock},
};

use tauri::{
    AppHandle, Emitter, Error::WindowNotFound, Manager, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

use crate::listens;
use mml_names::names;
use uuid::Uuid;

use super::geometry::{
    Placement, RectF, SizeF, center_placement, child_placement, remember_decorated_inset,
    save_window_state, window_state_for,
};
use super::registry::{
    ADD_MODPACK_WINDOW_UUID, ADD_WINDOW_UUID, BLOCK_WINDOW_UUID, MAIN_WINDOW_UUID,
    WINDOWS_INFO, family_of_label, min_size_of_label, uuid_for_kind, uuid_of_label,
};
use super::{add_modpack, block, resource};

/// 已打开窗口的句柄表：uuid → 句柄
pub(super) static OPEN_WINDOWS: LazyLock<RwLock<HashMap<Uuid, WebviewWindow<tauri::Wry>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 窗口模型表：uuid → 模型
///
/// 窗口模型跟随窗口生命周期：开窗时创建、关窗时销毁，不走全局 app.manage。
/// Tauri 的 `manage` / `State` 底层是同一个全局 StateManager，无法按窗口隔离，
/// 因此模型由本模块持有；命令通过窗口 label 解析 uuid 后取用。
pub(super) static WINDOW_MODELS: LazyLock<RwLock<HashMap<Uuid, Arc<dyn Any + Send + Sync>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 确保指定窗口的模型已创建，尚无则初始化
///
/// 当前仅主窗口有模型 `MainWindowModel`，其余窗口无状态；
/// 新增窗口模型时在此按 uuid 分派。
///
/// - `uuid`: 窗口 uuid
pub(super) fn ensure_window_model(uuid: &Uuid) {
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
pub(super) fn window_model_by_uuid<T: Send + Sync + 'static>(uuid: &Uuid) -> Option<Arc<T>> {
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
pub(super) fn window_model_or_insert<T: Send + Sync + 'static>(
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
pub(super) fn app_has_window(label: &str) -> bool {
    OPEN_WINDOWS
        .read()
        .unwrap()
        .values()
        .any(|w| w.label() == label)
}

/// 移除窗口模型，窗口销毁或逻辑页关闭时调用
///
/// - `uuid`: 窗口 uuid
pub(super) fn remove_window_model(uuid: &Uuid) {
    WINDOW_MODELS.write().unwrap().remove(uuid);
}

/// 窗口是否拒绝本次关闭
///
/// - 下载窗口 / 主窗口：仍有下载任务时拒绝；前端弹确认框，确认后停止下载再关窗 / 退出
/// - 添加实例 / 下载整合包窗口：模型侧关闭保护，查询数据期间拒绝
///
/// 其余窗口不保护。
///
/// - `uuid`: 窗口 uuid
///
/// # 返回值
///
/// 拒绝本次关闭时为真
pub(super) fn close_guarded(uuid: &Uuid) -> bool {
    // 下载窗口：**不拦**。它只是个查看器，关掉不影响下载（任务在后台跑），
    // 进度从标题栏的下载指示器进来还能看；拦着只会让用户以为非关不可

    // 主窗口：关它就是退出应用。单窗口模式下主窗口是唯一的真实窗口，任何页面的 ✕
    // 最后都落到这里；多窗口模式下关主窗口同样是退出。两种情况都会连带停掉下载，
    // 所以与下载窗口同一套处理：先拒绝、由前端问一句，确认后停下载再关
    if *uuid == MAIN_WINDOW_UUID {
        return !mml_downloader::get_tasks().is_empty();
    }

    // 方块窗口：渲染进行中拒绝关闭 —— 渲染任务虽独立于窗口存活，
    // 但关窗后进度无从展示，用户也会误以为渲染被中断
    if *uuid == BLOCK_WINDOW_UUID {
        return block::render_running();
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
fn notify_modpack_window(app: &AppHandle) {
    let dto = super::modpack_task::build_status(app);
    super::modpack_task::emit_add_modpack_status(app, dto);
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
pub(super) fn create_window(
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

    // 所有窗口都走插件装饰：必须以 decorations(true) 建出来，插件才能在原生 frame 上激活
    // 自绘装饰，激活前先藏起来。收益是 Win11 贴靠布局，矩形由前端量好交给插件，见 decoration.ts

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

    // 拖放路径桥的脚本挂在**建窗期**：initialization_script 在导航开始前就注册好，每个新建
    // 文档都会执行。放到 webview ready 回调里注入对初始文档无效 —— 那个 API 只对之后新建
    // 的文档生效，初始文档已经建好，脚本永远不执行（文件拖入会一直拿不到路径）
    #[cfg(windows)]
    let builder = builder.initialization_script(super::file_drop::INJECT_SCRIPT);

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

    // 底色走透明、并打开窗口透明：毛玻璃是系统在窗口背后合成的，底色不透明就把它整个盖住。
    // 界面自己的底色是半透明的（见 themes.css 的 mortar 规则），所以毛玻璃没生效时也看得清
    let builder = builder
        .transparent(true)
        .background_color(tauri::window::Color(0x14, 0x16, 0x1a, 0x00));
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

    // 系统材质：云母 / 亚克力 / 模糊按能力逐个降级，见 effects；挑哪一种由客户端设置决定
    super::effects::apply(&win, &crate::gui_config::get().client.window_effect);

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
fn initial_placement(
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
fn window_rect(win: &WebviewWindow, scale: f64) -> Option<RectF> {
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
pub(super) fn webview_data_dir() -> Option<PathBuf> {
    let cache = mml_downloader::get_cache_path();
    Some(cache.join(names::WEBVIEW_DIR))
}

/// 关掉挂在某个实例名下的所有窗口
///
/// 实例被删掉之后调用：那些窗口的模型还指着已经不存在的实例，留着只会点出错误。
/// 资源管理 / 日志 / 添加资源 / 配置文件四族一起关，走 [`close_window`] 的正常流程
/// （先存几何、各自的关闭保护与取消照旧）。
///
/// - `app`: 应用句柄
/// - `instance`: 被删掉的实例 uuid
pub(crate) fn close_instance_windows(app: &AppHandle, instance: &Uuid) {
    for family in super::registry::families() {
        let id = super::registry::family_window_id(family, instance);
        // 没开的窗口返回"找不到"，逐个关、失败忽略
        let _ = close_window(app, &id.uuid, &id.label);
    }
}

/// 关闭指定窗口：先保存几何，再真正关闭窗口
///
/// - `app`: 应用句柄，按标签补查窗口用
/// - `uuid`: 窗口 uuid
/// - `label`: 与 `uuid` 对应的窗口标签；静态窗口取自注册表，日志窗口族由实例派生
///
/// # 返回值
///
/// 关闭请求已发出返回 `Ok(())`；窗口已不存在返回错误文案
pub(crate) fn close_window(app: &AppHandle, uuid: &Uuid, label: &str) -> Result<(), String> {
    // 句柄优先取句柄表，缺失时按标签现查，覆盖外部创建的窗口。这里**只克隆、不提前摘表**：
    // `close()` 会先发 `CloseRequested`，关闭保护完全可能拦下它，摘了表就与真实窗口脱节
    let window = OPEN_WINDOWS
        .read()
        .unwrap()
        .get(uuid)
        .cloned()
        .or_else(|| app.get_webview_window(label));
    let Some(window) = window else {
        return Err(WindowNotFound.to_string());
    };

    // 句柄可能已失效，窗口被外部关闭：标签已不存在则清理句柄表并视为已关闭
    if app.get_webview_window(label).is_none() {
        OPEN_WINDOWS.write().unwrap().remove(uuid);
        return Ok(());
    }

    // 最大化 / 全屏时只记标志，几何保留最大化之前的值，见 save_window_state
    let maximized =
        window.is_maximized().unwrap_or(false) || window.is_fullscreen().unwrap_or(false);
    save_window_state(uuid, &window, maximized)?;
    window.close().map_err(|err| err.to_string())?;
    Ok(())
}

/// 关闭注册表里静态登记的窗口
///
/// - `app`: 应用句柄，按标签补查窗口用
/// - `uuid`: 窗口 uuid
///
/// # 返回值
///
/// 关闭请求已发出返回 `Ok(())`；uuid 未注册或窗口已不存在返回错误文案
pub(crate) fn close_window_from_uuid(app: &AppHandle, uuid: &Uuid) -> Result<(), String> {
    let Some(entry) = WINDOWS_INFO.get(uuid) else {
        return Err(WindowNotFound.to_string());
    };
    close_window(app, uuid, entry.label)
}

/// 打开或聚焦一个窗口
///
/// - `app`: 应用句柄，建窗用
/// - `uuid`: 窗口 uuid
/// - `label`: 与 `uuid` 对应的窗口标签
/// - `url_path`: 传给 [`create_window`] 的 webview 路径
/// - `anchor`: 发起这次打开的那个窗口，见 [`create_window`]
///
/// # 返回值
///
/// 窗口已打开或已聚焦返回 `Ok(())`；建窗失败返回错误文案
pub(crate) fn open_window(
    app: &AppHandle,
    uuid: &Uuid,
    label: &str,
    url_path: &str,
    anchor: Option<&WebviewWindow>,
) -> Result<(), String> {
    create_window(app, label, uuid, url_path, anchor)?;
    Ok(())
}

/// 打开或聚焦注册表里静态登记的窗口
///
/// - `app`: 应用句柄，建窗用
/// - `uuid`: 窗口 uuid
/// - `url_path`: 传给 [`create_window`] 的 webview 路径
/// - `anchor`: 发起这次打开的那个窗口，见 [`create_window`]
///
/// # 返回值
///
/// 窗口已打开或已聚焦返回 `Ok(())`；uuid 未注册或建窗失败返回错误文案
pub(crate) fn open_window_from_uuid(
    app: &AppHandle,
    uuid: &Uuid,
    url_path: &str,
    anchor: Option<&WebviewWindow>,
) -> Result<(), String> {
    let Some(entry) = WINDOWS_INFO.get(uuid) else {
        return Err(WindowNotFound.to_string());
    };
    open_window(app, uuid, entry.label, url_path, anchor)
}

/// 创建 / 聚焦主窗口，setup 阶段调用
///
/// - `app`: 应用句柄，建窗用
///
/// # 返回值
///
/// 主窗口已打开返回 `Ok(())`，建窗失败返回错误文案
pub fn show_main_window(app: &AppHandle) -> Result<(), String> {
    // 主窗口是顶层：没有历史几何时走"屏幕中心"，见 initial_placement
    open_window_from_uuid(app, &MAIN_WINDOW_UUID, "index.html", None)
}

/// 窗口事件处理，注册于 `Builder::on_window_event`
///
/// 原生标题栏 X、JS API 直接 `close()` 都不经过 `window_close_window` 命令，
/// 在 `CloseRequested` 阶段统一保存几何，此时窗口仍存活、位置有效；
/// `Destroyed` 阶段清理句柄表并销毁跟随窗口的模型。
///
/// - `window`: 触发事件的窗口
/// - `event`: 窗口事件，只处理关闭请求 / 销毁 / 缩放与移动
pub fn on_window_event(window: &tauri::Window<tauri::Wry>, event: &tauri::WindowEvent) {
    let label = window.label().to_string();
    // uuid 按标签反查：静态窗口走注册表，日志窗口族按 `mml-log-<实例uuid>` 派生
    let Some(uuid) = uuid_of_label(&label) else {
        return;
    };
    match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            // 窗口模型开启关闭保护时，如正在查询数据，拒绝本次关闭请求并通知前端提示
            if close_guarded(&uuid) {
                api.prevent_close();
                emit_close_blocked(window);
                return;
            }
            // 关闭已放行：终止该窗口还在跑的操作 —— 整合包搜索、模组扫描 / 更新检查 / 下载更新
            add_modpack::cancel_search(&label);
            resource::cancel_window(&label);
            // 下载整合包窗口关掉：那两个进度指示器要回到主页面，广播一次让 `window_open` 变 false
            if uuid == ADD_MODPACK_WINDOW_UUID {
                notify_modpack_window(window.app_handle());
            }
            if let Some(win) = window.app_handle().get_webview_window(&label) {
                // 最大化 / 全屏：只记标志、不覆盖几何，否则下次会开在另一块屏、而且不最大化
                let maximized =
                    win.is_maximized().unwrap_or(false) || win.is_fullscreen().unwrap_or(false);
                let _ = save_window_state(&uuid, &win, maximized);
            }
            if uuid == MAIN_WINDOW_UUID {
                mml_core::stop();
                let others: Vec<(Uuid, WebviewWindow<tauri::Wry>)> = OPEN_WINDOWS
                    .write()
                    .unwrap()
                    .iter()
                    .filter(|(u, _)| **u != MAIN_WINDOW_UUID)
                    .map(|(u, w)| (*u, w.clone()))
                    .collect();
                for (other_uuid, win) in others {
                    // 主窗口退出是最终退出：destroy 强制关闭，绕过各窗口的关闭保护。但 destroy
                    // **不发 CloseRequested**，那条链路的收尾不会执行 —— 这里手动补上取消搜索、
                    // 记下最大化标志与几何，否则下次开窗既不是最大化、几何也停在旧值。
                    add_modpack::cancel_search(win.label());
                    resource::cancel_window(win.label());
                    let maximized =
                        win.is_maximized().unwrap_or(false) || win.is_fullscreen().unwrap_or(false);
                    let _ = save_window_state(&other_uuid, &win, maximized);
                    let _ = win.destroy();
                }
            }
        }
        // 窗口已销毁：清理句柄与跟随窗口的模型，下次开窗重新创建
        tauri::WindowEvent::Destroyed => {
            // 关窗时把还在等应答的重名确认对话框按"拒绝"收尾：用户没答就关窗的话，
            // 安装 future 会永久 pending、任务卡在"进行中"，发送端也一直留在表里
            if let Some(kind) = label.strip_prefix("mml-")
                && let Some(model) =
                    model_for_kind::<Mutex<crate::windows::add::AddWindowModel>>(kind)
            {
                model.lock().unwrap().reject_all_dialogs();
            }
            // 配置窗口关了，它打开过的 NBT 文件也该放手：句柄不能留到下次开窗。
            // 配置窗口是一个实例一个，只清本实例那一份 —— 别的实例的编辑器还开着
            if let Some((family, instance)) = family_of_label(&label)
                && family.kind == "config"
            {
                super::nbt_session::close_all_of(&instance);
            }
            // 模型这就被移出表了，先把它里面在跑的任务取消掉：
            // 令牌被丢弃**不会**触发取消，任务会一直跑到自己结束
            resource::cancel_window(&label);
            OPEN_WINDOWS.write().unwrap().remove(&uuid);
            remove_window_model(&uuid);
        }
        // 缩放 / 移动：实时记录几何，不必等关窗，配置保存按文件去重合并写入
        // 最大化 / 全屏时不记，否则还原后会以最大化尺寸打开
        tauri::WindowEvent::Resized(_) | tauri::WindowEvent::Moved(_) => {
            if let Some(win) = window.app_handle().get_webview_window(&label) {
                if win.is_maximized().unwrap_or(false) || win.is_fullscreen().unwrap_or(false) {
                    return;
                }
                // 这里已确定不是最大化 / 全屏；取消最大化时也会走到这，顺手把标志清掉
                let _ = save_window_state(&uuid, &win, false);
            }
        }
        _ => {}
    }
}
