//! 窗口模块
//!
//! 每个窗口一个 rs 文件，包含：
//! - 窗口规格与模型：标签 / 标题 / 尺寸常量
//! - 窗口专属的数据模型与方法，如账户 / 新闻 / 游戏事件
//! - 窗口按钮调用的方法，即 IPC 命令，如 list_dir
//!
//! 所有窗口的创建、聚焦、关闭统一在本模块处理：
//! - 主窗口在 `setup` 阶段通过 [`show_main_window`] 创建，并恢复上次几何
//! - 功能窗口通过 [`create_window`] 创建或聚焦；`window_open_window` / `window_close_window`
//!   命令供前端调用，用于多窗口模式；参数为窗口 kind，与前端 `registry.ts` 对应
//! - 窗口几何状态 `window_save.json` 与 GUI 配置 `gui_config.json` 也在此维护

pub mod account;
pub mod add;
pub mod add_modpack;
pub mod add_resource;
pub mod block;
pub mod collect;
pub mod colormc;
pub mod config;
pub mod custom_home;
pub mod custom_ui;
pub mod download;
pub mod export;
pub mod java_download;
pub mod log;
pub mod lobby;
pub mod main;
pub mod resource;
pub mod settings;
pub mod stats;

// 内部子模块：**只放实现细节**，IPC 命令一律留在本文件。
// `bindings.ts` 的命令分组键取"来源 .rs 模块最后一段"，命令一搬组键就变，前端会断。
//
// 注意：仓库根的 `ipc-gen` 是**裸文本搜索**命令属性，见 ipc-gen/src/scan.rs 的 commands_in；
// 注释里写出那个属性字面量会让它把**紧随其后的函数**当成命令 —— 别在注释里写它。
mod effects;
mod geometry;
mod lifecycle;
mod modpack_task;
mod registry;
// 自定义整页站点的公共层：整包句柄、越界防护与协议响应，custom_home / custom_ui 共用
mod web_assets;
// 三种换皮包共用的类型判定与坏包校验，见 theme/mod.rs
mod theme;

// 打开着的 NBT 文件：句柄与解析结果留在会话里，配置窗口关了才放手。
// `pub(crate)`：配置窗口的读 / 写命令与窗口销毁时的兜底清理都要用它
pub(crate) mod nbt_session;

// Windows 文件拖放的"路径桥"：DOM 拖放 + 真实文件路径，见该模块头部说明。
// pub(crate)：lib.rs 注册 on_webview_ready 时要调它
#[cfg(windows)]
pub(crate) mod file_drop;

// 发布模式下关掉 WebView2 自带的浏览器快捷键与 DevTools，如 F5 / F12
#[cfg(windows)]
pub(crate) mod webview_keys;

use mml_game::GameInstance;
use mml_game::launcher::instance_setting_obj::InstanceSettingObj;
use mml_names::names;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

pub use self::geometry::init;
pub use self::lifecycle::{on_window_event, show_main_window};
// `add.rs` 用 `windows::model_for_kind` 取窗口模型，按原路径再导出
pub(crate) use self::lifecycle::{model_for_kind, window_model};

use self::lifecycle::{
    close_window, close_window_from_uuid, drop_model_for_kind, ensure_model_for_kind, open_window,
    open_window_from_uuid,
};

use self::geometry::{clamp_runtime_min_size, reconcile_min_size};
use self::registry::{
    FamilyWindowId, MAIN_WINDOW_UUID, WINDOWS_INFO, family_of_kind, family_window_id,
    min_size_of_label, uuid_for_kind,
};

use crate::listens;
use uuid::Uuid;

use crate::dtos::{DropPathDto, GuiConfigDto, LogFocusDto, WindowSizeDto};
use crate::err_keys;
use crate::log_text::LogText;

/// 主窗口的标签，托盘与新实例唤回时按它找窗口
///
/// # 返回值
///
/// 形如 `mml-main`，取自窗口注册表，别处不要另写字面量
pub(crate) fn main_window_label() -> &'static str {
    WINDOWS_INFO
        .get(&MAIN_WINDOW_UUID)
        .map(|entry| entry.label)
        .unwrap_or("mml-main")
}

/// 唤出主窗口：还在就显示并聚焦，已经关掉就按上次几何重建一个
///
/// 托盘点击与新实例请求「显示主窗口」都走这里。建窗是阻塞操作，不能在主线程
/// 直接做（托盘事件就在主线程），所以丢给异步运行时。
///
/// - `app`: 应用句柄
pub(crate) fn summon_main_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(main_window_label()) {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
        return;
    }

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(err) = show_main_window(&app) {
            mml_log::error(LogText::TraySummonFailed { error: err }.text());
        }
    });
}

/// 解析前端传来的分组 uuid，空白 / 非法一律按"默认分组"处理
///
/// 分组以 uuid 为身份，见 mml-game 的 `game_group`；但 IPC 参数照例是字符串，
/// 所以各处统一在这里转一次。宁可落到默认分组，也不要因为一个过期 id 让整条命令失败。
///
/// - `group`: 前端传来的分组 uuid 字符串
///
/// # 返回值
///
/// 空白 / 非法 uuid 返回 `None`，调用方按默认分组处理
pub fn parse_group_id(group: Option<String>) -> Option<Uuid> {
    group
        .filter(|g| !g.trim().is_empty())
        .and_then(|g| Uuid::parse_str(&g).ok())
}

/// 解析实例 uuid，拿实例句柄
///
/// 要用实例上的方法（尤其异步的扫存档 / 读模组）时走它。
///
/// - `uuid`: 实例 uuid 字符串
///
/// # 返回值
///
/// uuid 非法返回 `err.uuid`，实例不存在返回 `err.gameNotFound`
pub(crate) fn instance_handle(uuid: &str) -> Result<GameInstance, String> {
    let uuid = Uuid::parse_str(uuid).map_err(|_| String::from(err_keys::UUID))?;
    mml_game::get_instance(&uuid).ok_or_else(|| String::from(err_keys::GAME_NOT_FOUND))
}

/// 取实例设置的快照，路径类方法都在这份克隆上
///
/// 紧接着要读盘（遍历目录、扫 level.dat）时走它：持着读锁走一圈会把别的窗口一起卡住。
///
/// - `uuid`: 实例 uuid 字符串
///
/// # 返回值
///
/// uuid 非法返回 `err.uuid`，实例不存在返回 `err.gameNotFound`
pub(crate) fn instance_snapshot(uuid: &str) -> Result<InstanceSettingObj, String> {
    let instance = instance_handle(uuid)?;
    let snapshot = instance.read().unwrap().clone();
    Ok(snapshot)
}

/// 取各窗口的默认宽高，无历史几何时创建窗口就用它，与 WINDOWS_INFO 同源
///
/// 前端 JS 回退路径 `createViaJs` 从这里取尺寸，前端注册表不再自带宽高。
///
/// # 返回值
///
/// 每个已注册窗口一条，含 `kind`、默认宽与默认高；`kind` 是窗口标签去掉 `mml-` 前缀，
/// 宽高单位是逻辑像素的客户区尺寸
#[gui_macros::command]
pub fn window_get_window_sizes() -> Vec<WindowSizeDto> {
    WINDOWS_INFO
        .values()
        .map(|e| WindowSizeDto {
            kind: e.label.strip_prefix("mml-").unwrap_or(e.label).to_string(),
            width: e.min_width,
            height: e.min_height,
        })
        .collect()
}

/// 打开一个功能窗口，多窗口模式下由窗口按钮调用
///
/// 必须保持 async：同步命令在 Windows 上跑在主线程，而窗口创建会阻塞等待主线程，
/// 导致整个应用冻结：新窗口白屏、无法点击。
///
/// - `app`: 应用句柄，建窗与发事件都用它
/// - `window`: 调用方窗口，由 tauri 注入；新窗口没有历史几何时叠在它那个角的内侧
/// - `kind`: 窗口类型
/// - `instance`: 目标实例 UUID；窗口族，即日志 / 资源管理 / 添加资源 / 配置文件必须有它，
///   窗口身份由实例 uuid 派生，同一实例只会有一个该族窗口，重复打开只是聚焦；
///   窗口已存在时目标实例**不会**再切换
/// - `query`: 新窗口 URL 上的 query，**不含 `?`**
///
/// `query` 由前端 `windowManager` 组装 —— 目标窗口的实例与"要打开哪个项目"
/// 的 `uuid` / `psource` / `ppid` … 都只能靠 URL 送达，而参数名在前端
/// `PROJ_PARAMS` 里只有一份，读那边也用它，所以不在 Rust 侧另立一套。
/// 传空 / 不传时退回下面按 kind 拼的老路子，只有窗口族与 export 带 uuid。
///
/// # 返回值
///
/// 窗口已打开或已聚焦时返回 `Ok(())`；kind 未知、参数不匹配、query 畸形都返回错误文案
#[gui_macros::command]
pub async fn window_open_window(
    app: AppHandle,
    window: WebviewWindow,
    kind: String,
    instance: Option<String>,
    query: Option<String>,
) -> Result<(), String> {
    kind_parse_check(&kind, &instance)?;
    // 新窗口的目标参数只能靠 URL 送达：前端组装好了就用它，里面带 uuid / 项目参数；
    // 没给就退回老路子 —— 只有窗口族与 export 把 uuid 拼进去
    let url_path = match query.as_deref().filter(|q| !q.is_empty()) {
        Some(query) => {
            // 前端那份 query 是 URLSearchParams 编出来的，不会出现裸 # / 控制字符；
            // 这里只做一次兜底，别把畸形串拼进 WebviewUrl
            if query.contains('#') || query.chars().any(char::is_control) {
                return Err(String::from(err_keys::WINDOW_QUERY));
            }
            format!("index.html?{query}")
        }
        // 只有窗口族与 export 能带 instance，上面校验过了；这里把实例写进 URL，
        // 因为新窗口的实例身份只能靠 URL 送到前端
        None => match &instance {
            Some(instance) => format!("index.html?window={kind}&uuid={instance}"),
            None => String::from("index.html"),
        },
    };

    // 「下载资源」允许**不带实例**打开：那是"还没选实例"的浏览窗口，静态标签
    // `mml-add_resource`，实例在窗口标题栏里选（见 AddResourceWindow）。
    // 带实例时仍走窗口族，一实例一窗。
    if kind == "add_resource" && instance.is_none() {
        let Some(uuid) = uuid_for_kind(&kind) else {
            return Err(format!("unknown window kind: {kind}"));
        };
        return open_window_from_uuid(&app, &uuid, &url_path, Some(&window));
    }

    // 窗口族是"一个实例一个"：身份由实例 uuid 派生，没有实例就无从开窗
    if let Some(family) = family_of_kind(&kind) {
        let Some(instance) = instance.as_deref() else {
            return Err(err_keys::WINDOW_INSTANCE_MISSING.to_string());
        };
        let instance = Uuid::parse_str(instance).map_err(|_| err_keys::UUID.to_string())?;
        let id = family_window_id(family, &instance);
        // 目标实例已经写进窗口身份，已开的窗口不必、也不能再换实例
        return open_window(&app, &id.uuid, &id.label, &url_path, Some(&window));
    }

    let Some(uuid) = uuid_for_kind(&kind) else {
        return Err(format!("unknown window kind: {kind}"));
    };
    if kind == "lobby" { lobby::lobby_prepare(app.clone()).await?; }
    if let Err(error) = open_window_from_uuid(&app, &uuid, &url_path, Some(&window)) {
        if kind == "lobby" { lobby::lobby_close(); }
        return Err(error);
    }
    // 新创建的窗口靠 URL query 拿目标；已存在的窗口 URL 不变，靠事件切换
    if let Some(instance) = &instance
        && kind == "export"
    {
        export::emit_export_focus(
            &app,
            LogFocusDto {
                uuid: instance.clone(),
            },
        );
    }
    Ok(())
}

/// 打开 / 关闭窗口时的实例参数校验：kind 与 instance 要匹配
///
/// - `kind`: 窗口类型，只有窗口族与 export 接受 instance
/// - `instance`: 目标实例 UUID，其余 kind 传了即报错
///
/// # 返回值
///
/// 校验不通过返回 `err_keys` 里的键：kind 不匹配为 `err.windowQuery`，非法 uuid 为 `err.uuid`
fn kind_parse_check(kind: &str, instance: &Option<String>) -> Result<(), String> {
    let accepts_instance = kind == "export" || family_of_kind(kind).is_some();
    if !accepts_instance && instance.is_some() {
        return Err(err_keys::WINDOW_QUERY.to_string());
    }
    if accepts_instance
        && let Some(instance) = instance
    {
        Uuid::parse_str(instance).map_err(|_| err_keys::UUID.to_string())?;
    }
    Ok(())
}

/// 关闭一个窗口，多窗口模式下由窗口关闭按钮调用
///
/// - `app`: 应用句柄，取窗口与关闭都用它
/// - `kind`: 窗口类型
/// - `instance`: 目标实例 UUID；窗口族必须有它，窗口身份由实例 uuid 派生，见 [`family_window_id`]
///
/// # 返回值
///
/// 关闭请求已发出即返回 `Ok(())`，是否真关由关闭保护决定；kind 未知或窗口不存在返回错误文案
#[gui_macros::command]
pub fn window_close_window(
    app: AppHandle,
    kind: String,
    instance: Option<String>,
) -> Result<(), String> {
    kind_parse_check(&kind, &instance)?;
    // 不带实例的「下载资源」窗口是静态标签那个，与 window_open_window 的分支对应
    if kind == "add_resource" && instance.is_none() {
        let Some(uuid) = uuid_for_kind(&kind) else {
            return Err(format!("unknown window kind: {kind}"));
        };
        return close_window_from_uuid(&app, &uuid);
    }
    if let Some(family) = family_of_kind(&kind) {
        let Some(instance) = instance.as_deref() else {
            return Err(err_keys::WINDOW_INSTANCE_MISSING.to_string());
        };
        let instance = Uuid::parse_str(instance).map_err(|_| err_keys::UUID.to_string())?;
        let id = family_window_id(family, &instance);
        return close_window(&app, &id.uuid, &id.label);
    }
    let Some(uuid) = uuid_for_kind(&kind) else {
        return Err(format!("unknown window kind: {kind}"));
    };
    close_window_from_uuid(&app, &uuid)
}

/// 打开一个功能窗口；**已经在时按新 query 重新加载**，带新参数重开时用
///
/// 与 [`window_open_window`] 的区别全在"窗口已存在"那一支：那个只聚焦，
/// 页面状态要保留，目标实例靠 `export-focus` 这类事件推送；而这里的 query 本身就是
/// "要填进页面的东西"，如主窗口拖入的文件路径，不重新加载就送不进去，
/// 所以直接 `navigate` 到新 URL —— 页面重新挂载，`onMounted` 里即可读到新参数。
///
/// - `app`: 应用句柄，查已有窗口与建新窗口都用它
/// - `window`: 调用方窗口，由 tauri 注入；新建时没有历史几何就叠在它那个角的内侧
/// - `kind`: 窗口类型
/// - `instance`: 目标实例 UUID；窗口族必须有它，窗口身份由实例 uuid 派生，见 [`family_window_id`]
/// - `query`: 新窗口 URL 上的 query，**不含 `?`**，空则退回裸 `index.html`
///
/// # 返回值
///
/// 导航或建窗成功返回 `Ok(())`；kind 未知、参数不匹配、query 畸形、导航失败都返回错误文案
#[gui_macros::command]
pub async fn window_reopen_window(
    app: AppHandle,
    window: WebviewWindow,
    kind: String,
    instance: Option<String>,
    query: Option<String>,
) -> Result<(), String> {
    kind_parse_check(&kind, &instance)?;
    if kind == "lobby" {
        return window_open_window(app, window, kind, instance, query).await;
    }
    let query = query.unwrap_or_default();
    // 前端那份 query 是 URLSearchParams 编出来的，不会出现裸 # / 控制字符；
    // 这里只做一次兜底，别把畸形串拼进 URL，同 window_open_window
    if query.contains('#') || query.chars().any(char::is_control) {
        return Err(String::from(err_keys::WINDOW_QUERY));
    }
    let url_path = if query.is_empty() {
        String::from("index.html")
    } else {
        format!("index.html?{query}")
    };

    // 窗口族按实例定位到族里那一个；静态窗口按 kind 查表
    let id = match family_of_kind(&kind) {
        Some(family) => {
            let Some(instance) = instance.as_deref() else {
                return Err(err_keys::WINDOW_INSTANCE_MISSING.to_string());
            };
            let instance = Uuid::parse_str(instance).map_err(|_| err_keys::UUID.to_string())?;
            family_window_id(family, &instance)
        }
        None => {
            let Some(uuid) = uuid_for_kind(&kind) else {
                return Err(format!("unknown window kind: {kind}"));
            };
            FamilyWindowId {
                label: WINDOWS_INFO
                    .get(&uuid)
                    .map(|entry| entry.label.to_string())
                    .unwrap_or_default(),
                uuid,
            }
        }
    };

    if let Some(win) = app.get_webview_window(&id.label) {
        // 基准 URL 取窗口当前的：origin / devUrl 都一致，join 一个相对路径即可
        let base = win.url().map_err(|err| err.to_string())?;
        let url = base.join(&url_path).map_err(|err| err.to_string())?;
        win.navigate(url).map_err(|err| err.to_string())?;
        win.set_focus().map_err(|err| err.to_string())?;
        return Ok(());
    }
    open_window(&app, &id.uuid, &id.label, &url_path, Some(&window))
}

/// 拖入文件的真实路径，由 DOM 拖放的"路径桥"上报，见 [`file_drop`]
///
/// 只发给**目标窗口**：拖到哪个窗口就只该那个窗口响应。前端把它与 DOM 的 `drop`
/// 里的文本 / 链接合成同一个"拖入"回调。
///
/// - `app`: 应用句柄，发事件用它
/// - `label`: 目标窗口标签
/// - `payload`: 拖入文件的真实绝对路径
#[gui_macros::emit]
fn emit_file_drop(app: &AppHandle, label: &str, payload: crate::dtos::FileDropDto) {
    let _ = app.emit_to(label, listens::FILE_DROP, payload);
}

/// 从拖入的**文本**里取地址，网页卡片那种：`text/uri-list` / `text/plain` / `text/html`
///
/// DOM 的 `drop` 里文本是现成的，但"哪个片段才是地址"这套规则与 [`window_probe_drop`]
/// 解析快捷方式文件时是同一套：去空白 / 去尾斜杠 / 只认 http(s) 且有主机；
/// 所以放在 Rust 侧共用一份，前端别另写一遍。
///
/// - `text`: DOM `drop` 里拿到的文本片段
///
/// # 返回值
///
/// 规范化的 http(s) 地址；取不到返回空串
#[gui_macros::command]
pub fn window_url_of_text(text: String) -> String {
    let direct = clean_url(&text);
    if !direct.is_empty() {
        return direct;
    }
    find_http(&text).unwrap_or_default()
}

/// 拖入路径的探测结果，见 [`DropPathDto`]
///
/// 三个拖拽落点共用：
/// - 主窗口据此决定"开「添加实例」还是开「账户管理」"；
/// - 资源窗口据此认出"拖进来的是目录 / 登录快捷方式"，这两类它都不接；
/// - 「添加实例」窗口自己也拖入时同样先问这里。
///
/// 判定只看文件系统元数据、后缀与文件内容；进了对应窗口后，读条目 / 识别类型
/// 由那边再做一次，见前端 `applyArchivePath` / `pickFolder`。
///
/// 注意：**网页上的"卡片"直接拖进窗口走的是另一条路**：DOM 拖放 + 路径桥，见
/// [`file_drop`] —— 卡片是文本，不会经过这里；本命令只处理**拖进来的文件**。
///
/// - `path`: 拖入文件的绝对路径
///
/// # 返回值
///
/// `kind` 取 `folder` / `archive` / `shortcut` / `other`；`shortcut` 时 `url` 为解析出的地址
#[gui_macros::command]
pub fn window_probe_drop(path: String) -> DropPathDto {
    let other = |kind: &str| DropPathDto {
        kind: kind.to_string(),
        url: String::new(),
    };

    if std::fs::metadata(&path)
        .map(|meta| meta.is_dir())
        .unwrap_or(false)
    {
        return other("folder");
    }

    match path
        .to_ascii_lowercase()
        .rsplit_once('.')
        .map(|(_, ext)| ext)
    {
        Some("zip") if export::server_pack::is_server_pack(std::path::Path::new(&path)) => other("serverPack"),
        Some("zip" | "mrpack" | "modpack") => other("archive"),
        Some("url" | "webloc" | "website") => shortcut_of(&path),
        // 卡片拖到桌面后不一定是 `.url`：有的站点给的是 `.json` / `.txt`。
        // 这两类**只在"整份内容就只有一个地址"时才认**，见 single_url_of_text ——
        // 否则 modpack 的 manifest.json 这类里面也含地址的文件会被误判成登录快捷方式。
        Some("json" | "txt") if std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) <= 4096 => {
            let url = read_whole_text(&path)
                .map(|text| single_url_of_text(&text))
                .unwrap_or_default();
            if url.is_empty() {
                other("other")
            } else {
                DropPathDto {
                    kind: String::from("shortcut"),
                    url,
                }
            }
        }
        _ => other("other"),
    }
}

/// 快捷方式文件 `.url` / `.webloc` / `.website` → 解析出的地址
fn shortcut_of(path: &str) -> DropPathDto {
    let url = read_shortcut_url(path);
    if url.is_empty() {
        // 快捷方式坏了、读不出地址：当不认识的文件处理，别让上层弹个空表单
        DropPathDto {
            kind: String::from("other"),
            url: String::new(),
        }
    } else {
        DropPathDto {
            kind: String::from("shortcut"),
            url,
        }
    }
}

/// 整份读成文本，限小文件；读不出来返回 None
fn read_whole_text(path: &str) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(String::from_utf8_lossy(&bytes).to_string())
}

/// 文本里**只有一个地址**时取它；卡片拖到桌面可能是 `.json` / `.txt`
///
/// 判据：整份内容里只有一个 http(s) 地址，且除它以外的字符不超过 64 个 ——
/// 这样 `"https://…"`、`{"url":"https://…"}` 都认，而 modpack 的 manifest.json
/// 这种一堆地址的、或夹着大段说明的文本不会被误判成登录快捷方式。
fn single_url_of_text(text: &str) -> String {
    let urls = all_urls(text);
    if urls.len() != 1 {
        return String::new();
    }
    let url = &urls[0];
    if text.len().saturating_sub(url.len()) > 64 {
        return String::new();
    }
    url.clone()
}

/// 文本里出现的全部 http(s) 地址
fn all_urls(text: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let mut rest = text;
    while let Some(url) = find_http(rest) {
        let offset = rest.find(&url).unwrap_or(0);
        rest = &rest[offset + url.len()..];
        urls.push(url);
    }
    urls
}

/// 读快捷方式文件里的地址：`.url` / `.website` 是 INI 的 `URL=`，`.webloc` 是 plist
///
/// plist 有 XML 与**二进制**两种：二进制那种字符串也是原样存的，所以兜底一路
/// "在文件里找第一个 http(s) 地址"两种情况都能捞到，不必引 plist 解析库。
fn read_shortcut_url(path: &str) -> String {
    let Ok(bytes) = std::fs::read(path) else {
        return String::new();
    };
    // 快捷方式可能有 BOM / UTF-16：统一按 lossy 文本处理，地址都是 ASCII，够用
    let text = String::from_utf8_lossy(&bytes);

    for line in text.lines() {
        let line = line.trim();
        // 大小写不敏感：Windows 自己写的是 URL=，但别人生成的可能是 url=
        if let Some((key, value)) = line.split_once('=')
            && key.trim().eq_ignore_ascii_case("URL")
        {
            let url = clean_url(value);
            if !url.is_empty() {
                return url;
            }
        }
    }

    find_http(&text).unwrap_or_default()
}

/// 规范化快捷方式里的地址：去掉首尾空白与结尾的斜杠，后端认证时自己会补 `/`
///
/// 只有协议头、没有主机的，如 `https://`，算无效，返回空串 —— 免得拿一个残地址
/// 去弹账户表单。
fn clean_url(value: &str) -> String {
    let url = value.trim().trim_end_matches(['/', '\\']);
    let host = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"));
    match host {
        Some(host) if !host.is_empty() => url.to_string(),
        _ => String::new(),
    }
}

/// 在文本里找第一个 http(s) 地址，到空白 / 引号 / 尖括号 / NUL 为止
fn find_http(text: &str) -> Option<String> {
    let start = match (text.find("https://"), text.find("http://")) {
        (Some(a), Some(b)) => a.min(b),
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => return None,
    };
    let rest = &text[start..];
    let end = rest
        .find(|item: char| {
            item.is_whitespace() || item == '"' || item == '\'' || item == '<' || item == '\0'
        })
        .unwrap_or(rest.len());
    let url = clean_url(&rest[..end]);
    if url.is_empty() { None } else { Some(url) }
}

#[cfg(test)]
mod tests {
    use super::{clean_url, find_http, read_shortcut_url, window_probe_drop};
    use crate::test_support::{cleanup, temp_dir};

    /// 写一个临时文件并读回它的地址
    fn url_of(name: &str, content: &[u8]) -> String {
        let dir = temp_dir("probe-shortcut");
        let file = dir.join(name);
        std::fs::write(&file, content).unwrap();
        let url = read_shortcut_url(&file.to_string_lossy());
        cleanup(&[&dir]);
        url
    }

    /// Windows 互联网快捷方式 INI：读 `URL=`，忽略其它键
    #[test]
    fn shortcut_url_from_ini() {
        let text =
            "[InternetShortcut]\r\nURL=https://littleskin.cn/api/yggdrasil\r\nIconIndex=0\r\n";
        assert_eq!(
            url_of("LittleSkin.url", text.as_bytes()),
            "https://littleskin.cn/api/yggdrasil"
        );
    }

    /// 键名大小写不敏感；结尾斜杠去掉，后端认证时会自己补
    #[test]
    fn shortcut_url_is_normalized() {
        assert_eq!(
            url_of(
                "a.url",
                b"[InternetShortcut]\nurl=https://example.com/api/yggdrasil/\n"
            ),
            "https://example.com/api/yggdrasil"
        );
    }

    /// macOS 的 .webloc 是 plist：XML 那种直接扫地址，二进制那种也一样能捞到
    #[test]
    fn shortcut_url_from_plist() {
        let xml = br#"<?xml version="1.0"?><plist version="1.0"><dict><key>URL</key><string>https://example.com/api/yggdrasil</string></dict></plist>"#;
        assert_eq!(url_of("a.webloc", xml), "https://example.com/api/yggdrasil");

        // 二进制 plist：字符串原样存在文件里，兜底扫描要能捞出来
        let mut bin = b"bplist00\xd1\x01\x02X$versionYURL\x00".to_vec();
        bin.extend_from_slice(b"https://bin.example.com/api/yggdrasil");
        bin.push(0);
        assert_eq!(
            url_of("b.webloc", &bin),
            "https://bin.example.com/api/yggdrasil"
        );
    }

    /// 没有地址 / 只有协议头：都返回空串，上层当"不认识的文件"处理
    #[test]
    fn shortcut_without_url_is_empty() {
        assert_eq!(url_of("a.url", b"[InternetShortcut]\nIconIndex=0\n"), "");
        assert_eq!(url_of("b.url", b"[InternetShortcut]\nURL=https://\n"), "");
    }

    /// 探测：`.url` → shortcut；"只有一个地址"的 `.txt` / `.json` → 也认
    #[test]
    fn probe_shortcut_files() {
        assert_eq!(
            probe(
                "a.url",
                b"[InternetShortcut]\nURL=https://littleskin.cn/api/yggdrasil\n"
            ),
            "shortcut"
        );
        // 卡片拖到桌面有时是这些后缀：内容可能是裸地址、带引号、或包一层 json
        assert_eq!(
            probe("card.txt", b"  https://littleskin.cn/api/yggdrasil  \n"),
            "shortcut"
        );
        assert_eq!(
            probe("quoted.json", b"\"https://littleskin.cn/api/yggdrasil\""),
            "shortcut"
        );
        assert_eq!(
            probe(
                "wrapped.json",
                br#"{"url":"https://littleskin.cn/api/yggdrasil"}"#
            ),
            "shortcut"
        );
        // 里面夹着别的东西，如 modpack 的 manifest.json 有多个地址 + 大段内容，不能当登录快捷方式
        assert_eq!(
            probe(
                "manifest.json",
                br#"{"files":[{"url":"https://a.com/1.jar"},{"url":"https://a.com/2.jar"}]}"#
            ),
            "other"
        );
        assert_eq!(probe("readme.txt", b"hello"), "other");
        // 太长的小文本也不认：只有一个地址但夹着一大段说明
        let long = format!("{}https://a.com/x", "说明文字".repeat(30));
        assert_eq!(probe("note.txt", long.as_bytes()), "other");
    }

    /// 写法统一：建临时文件 → 探测 → 拿 kind
    fn probe(name: &str, content: &[u8]) -> String {
        let dir = temp_dir("probe-kind");
        let file = dir.join(name);
        std::fs::write(&file, content).unwrap();
        let kind = window_probe_drop(file.to_string_lossy().to_string()).kind;
        cleanup(&[&dir]);
        kind
    }

    /// 地址截断点：到空白 / 引号 / 尖括号为止，不会把后面的内容一起吞掉
    #[test]
    fn find_http_stops_at_delimiters() {
        assert_eq!(
            find_http("<string>https://a.com/x</string>"),
            Some(String::from("https://a.com/x"))
        );
        assert_eq!(
            find_http("see https://a.com/x and more"),
            Some(String::from("https://a.com/x"))
        );
        assert_eq!(find_http("no url here"), None);
    }

    /// 规范化：去空白、去尾斜杠、拒绝无主机
    #[test]
    fn clean_url_rules() {
        assert_eq!(clean_url("  https://a.com/  "), "https://a.com");
        assert_eq!(clean_url("http://a.com/x/"), "http://a.com/x");
        assert_eq!(clean_url("https://"), "");
        assert_eq!(clean_url("ftp://a.com"), "");
    }
}

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
#[gui_macros::command]
pub fn window_start_dragging(window: WebviewWindow) -> Result<(), String> {
    window.start_dragging().map_err(|err| err.to_string())
}

/// 激活插件装饰并显示窗口；所有窗口都走插件装饰，见 [`create_window`]
///
/// 窗口建出来时是 `decorations: true` + 隐藏的，见 `create_window`；
/// 这里在原生 frame 上激活自绘装饰、再显示。激活成功即由**插件的控件**接管标题栏右侧，
/// Windows 11 的贴靠布局 flyout 挂在它的最大化按钮上 —— 这正是本命令存在的理由。
///
/// 失败时退回原生 frame 并照样显示窗口：宁可露出系统标题栏，也不能让主窗口打不开。
///
/// - `window`: 调用方窗口，由 tauri 注入
///
/// # 返回值
///
/// 成功返回 `"custom"`，表示自绘装饰已接管；失败返回带原因的 `Err`
#[gui_macros::command]
pub async fn window_activate_decoration(window: WebviewWindow) -> Result<&'static str, String> {
    use tauri_plugin_decoration::WebviewWindowExt;

    if let Err(error) = window.activate_decoration().await {
        // 回退：先确保原生 frame 还在，再显示，最后把原因带回前端
        let _ = window.restore_decoration().await;
        // 退回原生 frame：此刻量到的差值含标题栏，正好是带装饰窗口该有的下限
        if let Some(min) = min_size_of_label(window.label()) {
            reconcile_min_size(&window, min.width, min.height);
        }
        let _ = window.show();
        // 显示不等于到前台：不抢一次焦点，新窗会压在发起它的那个窗口后面，看着像点了没反应
        let _ = window.set_focus();
        self::effects::apply(&window, &crate::gui_config::get().client.window_effect);
        return Err(format!("decoration activate failed: {error}"));
    }

    // 装饰已摘掉，此刻量到的「外框 − 客户区」才是那圈 Windows 边框带；
    // 建窗时量到的是边框带 + 标题栏，拿它当客户区下限会把窗口顶高，所以建窗时**不设**下限，
    // 到这里才设；激活失败退回原生 frame 的那条分支另设一次
    if let Some(min) = min_size_of_label(window.label()) {
        reconcile_min_size(&window, min.width, min.height);
        // 存盘口径是带装饰的客户区，比运行时口径小一个标题栏，恢复几何时夹不了这个下限，
        // 只能等装饰摘掉、inner_size() 变成运行时口径后在这里补一次
        clamp_runtime_min_size(&window, min);
    }

    window.show().map_err(|err| err.to_string())?;
    // 同上：show 之后补一次焦点，新窗才不会开在发起窗口后面
    window.set_focus().map_err(|err| err.to_string())?;
    // 材质要在窗口显示之后铺：demo 也是拿到可见窗口才调（examples/tauri 的 main.rs），
    // 隐藏状态下铺的会被随后 activate_decoration 改窗口样式一起丢掉
    self::effects::apply(&window, &crate::gui_config::get().client.window_effect);
    Ok("custom")
}

/// 最小化窗口
///
/// - `window`: 调用方窗口，由 tauri 注入
///
/// # 返回值
///
/// 成功返回 `Ok(())`，失败返回错误文案
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
#[gui_macros::command]
pub fn window_set_fullscreen(window: WebviewWindow, fullscreen: bool) -> Result<(), String> {
    window.set_fullscreen(fullscreen).map_err(|err| err.to_string())
}

/// 当前是否全屏，标题栏换图标用
///
/// - `window`: 调用方窗口，由 tauri 注入
///
/// # 返回值
///
/// 是否全屏；查询失败按未全屏处理
#[gui_macros::command]
pub fn window_is_fullscreen(window: WebviewWindow) -> bool {
    window.is_fullscreen().unwrap_or(false)
}

/// 把窗口铺满当前显示器的工作区：客户区左上角对齐工作区原点，尺寸取工作区
///
/// 与最大化不是一回事：这里**不进入系统最大化状态**，只是把窗口摆好，之后照常可拖动 / 缩放。
/// 工作区是屏幕去掉任务栏那一条；尺寸按**客户区**算，那圈系统阴影边框从外框让出去，
/// 所以窗口内容正好贴住工作区四边，不额外含标题栏。
/// 原本全屏或最大化时先退出来，否则系统会按那两个状态把窗口改回去。
///
/// - `window`: 调用方窗口，由 tauri 注入
///
/// # 返回值
///
/// 摆放成功返回 `Ok(())`，失败返回错误文案
#[gui_macros::command]
pub fn window_fill_screen(window: WebviewWindow) -> Result<(), String> {
    if window.is_fullscreen().unwrap_or(false) {
        window.set_fullscreen(false).map_err(|err| err.to_string())?;
    }
    if window.is_maximized().unwrap_or(false) {
        window.unmaximize().map_err(|err| err.to_string())?;
    }
    let monitor = window
        .current_monitor()
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "window has no current monitor".to_string())?;
    let work = monitor.work_area();

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

/// 设置调用方窗口的原生标题，任务栏 / Alt+Tab 显示
///
/// 窗口创建时统一叫 `names::MML`；自绘标题栏文案由前端持有，已 i18n，
/// 因此由前端在标题变化时同步过来，语言切换后原生标题也能跟随
///
/// - `window`: 调用方窗口，由 tauri 注入
/// - `title`: 新的原生标题，用前端已 i18n 好的文案
///
/// # 返回值
///
/// 设置成功返回 `Ok(())`，失败返回错误文案
#[gui_macros::command]
pub fn window_set_title(window: WebviewWindow, title: String) -> Result<(), String> {
    window.set_title(&title).map_err(|err| err.to_string())
}

/// 确保某个窗口 kind 的模型存在，单窗口模式下前端页面挂载时调用
///
/// 多窗口模式下模型本来就跟着真实窗口创建，这里重复调用无副作用。
///
/// - `kind`: 窗口类型
///
/// # 返回值
///
/// 模型已就绪返回 `Ok(())`；kind 未知返回错误文案
#[gui_macros::command]
pub fn window_ensure_model(kind: String) -> Result<(), String> {
    ensure_model_for_kind(&kind)
}

/// 释放某个窗口 kind 的模型，单窗口模式下前端页面切走 / 关闭时调用
///
/// 单窗口模式没有"窗口关闭"这个事件可依赖 —— 页面切走时组件进 KeepAlive，
/// 后端收不到任何通知。不显式释放的话模型会一直挂着，常驻内存，
/// 而页面上的取消令牌 / 关闭保护 / 重名应答通道都是**就地失效**的：
/// 下次回到该页会重新 ensure 一份干净的。
///
/// - `kind`: 窗口类型
///
/// # 返回值
///
/// 模型已释放或本来就没有时返回 `Ok(())`；kind 未知返回错误文案
#[gui_macros::command]
pub fn window_drop_model(kind: String) -> Result<(), String> {
    drop_model_for_kind(&kind)
}

/// 重启启动器：切换窗口模式这类"必须重启才干净生效"的设置改完后，由前端调用
///
/// 顺序有讲究：
/// 1. **先落盘**：`mml_core::stop()` 触发停止事件，其中 `config_save::stop` 是同步 join，
///    返回即代表队列写完。若放到拉起新进程之后，新进程可能读到旧配置、仍旧按旧模式打开；
/// 2. **再拉起一份自己**：`current_exe()` 就是当前可执行文件；
/// 3. **最后退出本进程**：放在后台线程里延迟一下再退——开发模式下 `tauri dev` 的 vite 会随本进程
///    退出而关闭，新进程得趁它活着把页面拉起来；release 没有这个依赖，稍等让新窗口先出现即可。
///
/// 本命令一定结束当前进程，是否提示由调用方决定。
///
/// - `app`: 应用句柄，退出当前进程用它
#[gui_macros::command]
pub fn window_restart_app(app: AppHandle) {
    mml_core::stop();
    mml_game::game_count::stop();

    // 先让出端口文件：新进程带 --restart 直接锁它，晚一步就会锁不上
    crate::single_instance::release_port_file();

    std::thread::spawn(move || {
        match std::env::current_exe() {
            Ok(exe) => {
                // 带上 --restart：新进程跳过单开检查，否则会把请求交回本进程（马上要退的那个）
                if let Err(err) = std::process::Command::new(exe)
                    .arg(names::COMMAND_RESTART)
                    .arg("--run-dir")
                    .arg(crate::gui_config::dir().unwrap())
                    .spawn()
                {
                    mml_log::error(
                        LogText::RestartLauncherFailed {
                            error: err.to_string(),
                        }
                        .text(),
                    );
                }
            }
            Err(err) => {
                mml_log::error(LogText::RestartLauncherError { error: err.to_string() }.text());
            }
        }

        // debug 即 tauri dev 时，等新进程把页面从 vite 拉起来；release 下没有 dev server 这回事
        let wait = if cfg!(debug_assertions) { 2000 } else { 150 };
        std::thread::sleep(std::time::Duration::from_millis(wait));
        app.exit(0);
    });
}

/// 获取 GUI 状态；无文件时返回默认值。前端 wire 为 DTO，TS 命名 camelCase
///
/// 首次启动、无配置文件时默认主题跟随系统深浅色——WebView2 的窗口主题
/// 即系统的应用模式。此时不落盘，用户第一次改设置才会把当时的主题存进文件
///
/// - `window`: 调用方窗口，读它的主题判断系统深浅色
///
/// # 返回值
///
/// 当前 GUI 状态；首次启动时 `theme` 已按系统深浅色填好
#[gui_macros::command]
pub fn window_get_gui_config(window: WebviewWindow) -> GuiConfigDto {
    let mut config = crate::gui_config::get();
    if crate::gui_config::is_fresh() {
        config.theme = match window.theme() {
            Ok(tauri::Theme::Light) => crate::gui_config::Theme::Light,
            _ => crate::gui_config::Theme::Dark,
        };
    }
    config.into()
}

/// 保存 GUI 状态到 gui_config.json，前端 DTO 转内部 GuiConfig
///
/// 皮肤显示模式 / 头像配置有变化时广播 `skin-config-change`：渲染缓存键里含这两项，
/// 各窗口收到后按新模式重取头像 / 皮肤图；实例列表显示模式有变化时广播
/// `gui-view-mode-change`，主窗口据此立即换视图；背景图的不透明度 / 模糊有变化时
/// 广播 `bg-change`，各窗口重取背景状态；主题 / 强调色有变化时广播
/// `gui-theme-change`，各窗口重新应用主题；窗口材质变化时直接在这里重铺所有窗口，
/// 透明度是前端 CSS 变量，随 `client-config-change` 由各窗口自己跟上
///
/// - `app`: 应用句柄，广播变更事件用它
/// - `config`: 前端提交的完整 GUI 状态，内部转成落盘用的 `GuiConfig`
///
/// # 返回值
///
/// 保存成功返回 `Ok(())`；失败返回错误文案
#[gui_macros::command]
pub async fn window_save_gui_config(app: AppHandle, config: GuiConfigDto) -> Result<(), String> {
    let mut new_config: crate::gui_config::GuiConfig = config.into();
    // 窗口浏览偏好不属于设置页：从旧配置整份带过来，免得存一次设置就把它们清掉
    new_config.resource_window = crate::gui_config::get().resource_window;
    new_config.config_window = crate::gui_config::get().config_window;
    let skin_changed = {
        let old = crate::gui_config::get();
        old.skin != new_config.skin || old.head != new_config.head || old.cape != new_config.cape
    };
    let client_changed = {
        let old = crate::gui_config::get();
        old.client != new_config.client
    };
    let view_mode_changed = {
        let old = crate::gui_config::get();
        old.main_window.view_mode != new_config.main_window.view_mode
    };
    // 背景图的不透明度 / 模糊没有专门的命令，只经这里落盘：变了就广播背景变更，
    // 否则别的窗口要等下次启动才跟着变
    let bg_view_changed = {
        let old = crate::gui_config::get();
        old.bg_opacity != new_config.bg_opacity || old.bg_blur != new_config.bg_blur
    };
    // 主题 / 强调色同理，而且它连 localStorage 都靠不住：多窗口模式下各窗口是不同 webview，
    // storage 事件在它们之间不发，只有设置窗口自己会变
    let theme_changed = {
        let old = crate::gui_config::get();
        old.theme != new_config.theme
            || old.accent != new_config.accent
            || old.custom_accent != new_config.custom_accent
    };
    let new_theme = new_config.theme;
    let new_accent = new_config.accent.clone();
    let new_custom_accent = new_config.custom_accent.clone();
    let new_client = new_config.client.clone();
    if client_changed {
        let uuid = new_client.lock_instance.clone();
        if !uuid.is_empty()
            && let Ok(uuid) = Uuid::parse_str(&uuid)
            && mml_game::have_instance_uuid(&uuid)
        {
            new_config.main_window.selected_instance = new_client.lock_instance.clone();
        }
    }
    let new_view_mode = new_config.main_window.view_mode;
    let locale_changed = crate::gui_config::get().locale != new_config.locale;
    let effect_before = crate::gui_config::get().client.window_effect;
    crate::gui_config::set(new_config);
    // 材质是建窗那一刻铺上去的：设置里换了就地重铺一遍，否则得等下次开窗才看得到
    let effect_now = crate::gui_config::get().client.window_effect;
    if effect_before != effect_now {
        for (_, win) in app.webview_windows() {
            crate::windows::effects::apply(&win, &effect_now);
        }
    }
    if skin_changed {
        emit_skin_config_change(&app);
    }
    if client_changed {
        emit_client_config_change(&app, new_client.into());
    }
    if view_mode_changed {
        emit_gui_view_mode_change(&app, new_view_mode);
    }
    if bg_view_changed {
        crate::windows::settings::bg::emit_bg_change(&app);
    }
    if theme_changed {
        emit_gui_theme_change(&app, new_theme, new_accent, new_custom_accent);
    }
    if locale_changed {
        // 托盘是原生菜单，前端切语言带不动它，只能在这里重建一遍
        crate::tray::refresh_text(&app);
    }
    Ok(())
}

/// 皮肤 / 头像显示配置变更事件，跨窗口同步重取渲染图
///
/// - `app`: 应用句柄，广播用它
#[gui_macros::emit]
fn emit_skin_config_change(app: &AppHandle) {
    let _ = app.emit(listens::SKIN_CONFIG_CHANGE, ());
}

/// 客户端设置变更事件，跨窗口同步，如主窗口 MOTD 卡片 / 登录锁定 / 窗口透明度
///
/// - `app`: 应用句柄，广播用它
/// - `config`: 变更后的客户端设置
#[gui_macros::emit]
fn emit_client_config_change(app: &AppHandle, config: crate::dtos::ClientConfigDto) {
    let _ = app.emit(listens::CLIENT_CONFIG_CHANGE, config);
}

/// 实例列表显示模式变更事件，跨窗口同步
///
/// 切换入口在设置窗口，主窗口只是读的一方：多窗口模式下两者是不同的 webview，
/// 不广播的话只能等下次启动才生效，见 `window_save_gui_config`。
///
/// - `app`: 应用句柄，广播用它
/// - `mode`: 变更后的显示模式
#[gui_macros::emit]
fn emit_gui_view_mode_change(app: &AppHandle, mode: crate::gui_config::ViewMode) {
    let _ = app.emit(listens::GUI_VIEW_MODE_CHANGE, mode);
}

/// 主题 / 强调色变更事件，跨窗口同步
///
/// 切换入口在设置窗口：多窗口模式下其它窗口是不同的 webview，要靠这条事件重新应用主题
/// （localStorage 的 storage 事件在 WebView2 的多 webview 之间不发，只能走壳层广播）。
///
/// - `app`: 应用句柄，广播用它
/// - `theme`: 变更后的主题
/// - `accent`: 变更后的强调色 id，或 `custom`
/// - `custom_accent`: 自定义强调色，`#rrggbb`
#[gui_macros::emit]
fn emit_gui_theme_change(
    app: &AppHandle,
    theme: crate::gui_config::Theme,
    accent: String,
    custom_accent: String,
) {
    let _ = app.emit(
        listens::GUI_THEME_CHANGE,
        crate::dtos::GuiThemeDto {
            theme,
            accent,
            custom_accent,
        },
    );
}
