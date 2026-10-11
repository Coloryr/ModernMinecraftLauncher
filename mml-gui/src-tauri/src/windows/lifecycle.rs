//! 窗口生命周期：句柄表 / 模型表 / 建窗 / 开关窗 / 窗口事件
//!
//! 从 `windows/mod.rs` 拆出来的。这里管"窗口从生到死"：建窗，含几何恢复 + 装饰时机 +
//! 最小尺寸校正；句柄与模型的登记清理、关闭保护，以及 `on_window_event` 里的
//! 关闭 / 销毁 / 缩放事件。
//!
//! **`window_*` 命令留在 `mod.rs`**：`bindings.ts` 的组键取"来源 .rs 模块最后一段"，
//! 命令搬文件会让组键从 `windows` 变成别的名字，前端会断，见 AGENTS.md §4。

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
    min_inner_physical, reconcile_min_size, remember_decorated_inset, save_window_state,
    window_state_for,
};
use super::registry::{
    ADD_MODPACK_WINDOW_UUID, ADD_WINDOW_UUID, BLOCK_WINDOW_UUID, DOWNLOAD_WINDOW_UUID,
    MAIN_WINDOW_UUID, MinSize, WINDOWS_INFO, uuid_for_kind,
};
use super::{add_modpack, block};

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
    // 下载窗口：任务未清空时不让直接关，避免后台下载被静默中断
    if *uuid == DOWNLOAD_WINDOW_UUID {
        return !mml_downloader::get_tasks().is_empty();
    }

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

/// 关闭被拒绝事件：窗口处于关闭保护时前端弹提示说明原因
///
/// - `window`: 被拒绝关闭的窗口，事件只发给它
#[gui_macros::emit]
pub(crate) fn emit_close_blocked(window: &tauri::Window<tauri::Wry>) {
    let _ = window.emit_to(window.label(), listens::CLOSE_BLOCKED, ());
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
///
/// # 返回值
///
/// 建好或聚焦到的窗口；建窗失败返回错误文案
pub(super) fn create_window(
    app: &AppHandle,
    label: &str,
    uuid: &Uuid,
    url_path: &str,
) -> Result<WebviewWindow, String> {
    // 已存在则聚焦，避免重复窗口
    if let Some(win) = app.get_webview_window(label) {
        win.set_focus().map_err(|err| err.to_string())?;
        // 句柄表可能因异常退出丢失，补记一份
        OPEN_WINDOWS.write().unwrap().insert(*uuid, win.clone());
        ensure_window_model(uuid);
        return Ok(win);
    }

    let geom = window_state_for(uuid);
    // 上次是最大化：按记录里的「最大化之前」几何开窗，开完再最大化，见 save_window_state
    let restore_maximized = geom.as_ref().map(|g| g.maximized).unwrap_or(false);

    // 最小尺寸随注册表条目走，各窗口内容布局不同、可压缩程度不同；
    // 无历史几何时直接以最小尺寸居中打开，注册表即窗口尺寸的唯一来源
    let binding = &*WINDOWS_INFO;
    let (min_w, min_h) = binding
        .get(uuid)
        .map(|e| (e.min_width, e.min_height))
        .unwrap_or((600.0, 400.0));

    // 所有窗口都走插件装饰：必须以 decorations(true) 建出来，插件才能在原生 frame 上激活
    // 自绘装饰，激活前先藏起来。收益是 Win11 贴靠布局，矩形由前端量好交给插件，见 decoration.ts

    // webview 铺不透明暗色底，避免加载首帧透出桌面，与暗色主题 --bg 一致
    let builder = WebviewWindowBuilder::new(app, label, WebviewUrl::App(url_path.into()))
        .title(names::MML)
        .min_inner_size(min_w, min_h)
        // 原生 frame 起步 + 隐藏：装饰激活成功后再显示，失败则退回原生 frame 显示
        .decorations(true)
        // 关掉框架 wry 自己那份拖放实现：它会顶掉 WebView2 的拖放目标且只认文件，网页上
        // 拖下来的卡片连 dragenter 都收不到。关掉后 DOM 拖放全功能，路径由 file_drop 补上
        .disable_drag_drop_handler()
        .visible(false);

    // 不指定 WebView2 用户数据目录时 Tauri 会强制落到 `%LOCALAPPDATA%\<identifier>\`，界面数据就
    // 散在系统盘；指到 `<cache>/webview` 后跟运行目录走，cache 已由下载器 init 建好，这里只兜一次
    let builder = match webview_data_dir() {
        Some(dir) => {
            if let Err(err) = std::fs::create_dir_all(&dir) {
                // 建不出来不致命：退回 Tauri 的默认目录总比开不了窗好
                mml_log::error(format!(
                    "[window] 创建 webview 数据目录失败，退回默认位置：{} ({err})",
                    dir.display()
                ));
                builder
            } else {
                builder.data_directory(dir)
            }
        }
        None => builder,
    };

    let builder = builder.background_color(tauri::window::Color(0x14, 0x16, 0x1a, 0xff));
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

            // 恢复前先夹一次最小客户区：历史几何可能小于当前最小尺寸，旧版本存的或最小尺寸
            // 后来调大过；直接创建会得到一个比最小值还小的窗口，而这个非法值又会被写回文件
            let min = min_inner_physical(
                MinSize {
                    width: min_w,
                    height: min_h,
                },
                scale,
            );
            builder
                .inner_size(
                    g.width.max(min.width) as f64 / scale,
                    g.height.max(min.height) as f64 / scale,
                )
                .position(g.x as f64 / scale, g.y as f64 / scale)
                .build()
        }
        None => builder.inner_size(min_w, min_h).center().build(),
    };
    let win = win.map_err(|e| e.to_string())?;

    // 必须在**这一刻**量：此时窗口还是 decorations(true)，差值包含原生标题栏；插件激活摘掉
    // 装饰后标题栏并进客户区就再也量不到它，而关窗保存正发生在那个状态
    remember_decorated_inset(uuid, &win);

    // 无边框 + 系统阴影时客户区比外框小一圈，而 WM_GETMINMAXINFO 约束的是外框：不把这圈
    // 带宽补上，min_inner_size 实际会把客户区压得比注册表里设定的更小
    reconcile_min_size(&win, min_w, min_h);

    OPEN_WINDOWS.write().unwrap().insert(*uuid, win.clone());
    ensure_window_model(uuid);
    // 开窗即记录初始几何，文件始终反映当前所有窗口，后续缩放/移动会持续更新。
    // 带上 restore_maximized：下面 maximize() 触发的 Resized 会被跳过，最大化时不记几何，
    // 否则文件里的最大化标志会被这次初始保存清掉。
    let _ = save_window_state(uuid, &win, restore_maximized);
    if restore_maximized {
        let _ = win.maximize();
    }
    Ok(win)
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

/// 关闭指定 uuid 的窗口：先保存几何，再真正关闭窗口
///
/// - `app`: 应用句柄，按标签补查窗口用
/// - `uuid`: 窗口 uuid
///
/// # 返回值
///
/// 关闭请求已发出返回 `Ok(())`；uuid 未注册或窗口已不存在返回错误文案
pub(crate) fn close_window_from_uuid(app: &AppHandle, uuid: &Uuid) -> Result<(), String> {
    let binding = &*WINDOWS_INFO;
    let Some(entry) = binding.get(uuid) else {
        return Err(WindowNotFound.to_string());
    };
    let label = entry.label;

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

/// 打开或聚焦指定 uuid 的窗口
///
/// - `app`: 应用句柄，建窗用
/// - `uuid`: 窗口 uuid
/// - `url_path`: 传给 [`create_window`] 的 webview 路径
///
/// # 返回值
///
/// 窗口已打开或已聚焦返回 `Ok(())`；uuid 未注册或建窗失败返回错误文案
pub(crate) fn open_window_from_uuid(
    app: &AppHandle,
    uuid: &Uuid,
    url_path: &str,
) -> Result<(), String> {
    let binding = &*WINDOWS_INFO;
    let Some(entry) = binding.get(uuid) else {
        return Err(WindowNotFound.to_string());
    };
    let label = entry.label;

    create_window(app, label, uuid, url_path)?;
    Ok(())
}

/// 创建 / 聚焦主窗口，setup 阶段调用
///
/// - `app`: 应用句柄，建窗用
///
/// # 返回值
///
/// 主窗口已打开返回 `Ok(())`，建窗失败返回错误文案
pub fn show_main_window(app: &AppHandle) -> Result<(), String> {
    open_window_from_uuid(app, &MAIN_WINDOW_UUID, "index.html")
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
    let binding = &*WINDOWS_INFO;
    let Some((uuid, _)) = binding.iter().find(|(_, e)| e.label == label) else {
        return;
    };
    match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            // 窗口模型开启关闭保护时，如正在查询数据，拒绝本次关闭请求并通知前端提示
            if close_guarded(uuid) {
                api.prevent_close();
                emit_close_blocked(window);
                return;
            }
            // 关闭已放行：取消该窗口还在跑的整合包搜索
            add_modpack::cancel_search(&label);
            if let Some(win) = window.app_handle().get_webview_window(&label) {
                // 最大化 / 全屏：只记标志、不覆盖几何，否则下次会开在另一块屏、而且不最大化
                let maximized =
                    win.is_maximized().unwrap_or(false) || win.is_fullscreen().unwrap_or(false);
                let _ = save_window_state(uuid, &win, maximized);
            }
            if *uuid == MAIN_WINDOW_UUID {
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
            OPEN_WINDOWS.write().unwrap().remove(uuid);
            remove_window_model(uuid);
        }
        // 缩放 / 移动：实时记录几何，不必等关窗，配置保存按文件去重合并写入
        // 最大化 / 全屏时不记，否则还原后会以最大化尺寸打开
        tauri::WindowEvent::Resized(_) | tauri::WindowEvent::Moved(_) => {
            if let Some(win) = window.app_handle().get_webview_window(&label) {
                if win.is_maximized().unwrap_or(false) || win.is_fullscreen().unwrap_or(false) {
                    return;
                }
                // 这里已确定不是最大化 / 全屏；取消最大化时也会走到这，顺手把标志清掉
                let _ = save_window_state(uuid, &win, false);
            }
        }
        _ => {}
    }
}
