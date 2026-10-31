//! 窗口模块
//!
//! 每个窗口一个模块，放该窗口的规格、数据模型与 IPC 命令：
//! - 窗口注册表（uuid / 标签 / 最小尺寸 / 窗口族）在 `registry`，是唯一来源
//! - 建窗、关窗与窗口事件在 `lifecycle`，几何口径与 `window_save.json` 在 `geometry`
//! - 本文件只放 `window_*` 命令与几个跨窗口共用的解析函数：命令的分组键取来源模块，
//!   搬进子模块会让前端 `bindings.ts` 的组名跟着变
//!
//! 主窗口在 `setup` 阶段通过 [`show_main_window`] 创建并恢复上次几何；功能窗口走
//! `window_open_window` / `window_close_window` 两个命令，参数是窗口 kind，与前端
//! `windowManager` 的 kind 对应。GUI 配置的读写命令也留在本文件。

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
pub(crate) mod gamepad;
pub mod java_download;
pub mod lobby;
pub mod log;
pub mod main;
pub mod minecraft;
pub mod resource;
pub(crate) mod server_instance;
pub mod settings;
pub mod stats;

// 内部子模块：**只放实现细节**，IPC 命令一律留在本文件。
// `bindings.ts` 的命令分组键取"来源 .rs 模块最后一段"，命令一搬组键就变，前端会断。
//
// 注意：仓库根的 `ipc-gen` 是**裸文本搜索**命令属性，见 ipc-gen/src/scan.rs 的 commands_in；
// 注释里写出那个属性字面量会让它把**紧随其后的函数**当成命令 —— 别在注释里写它。
mod effects;
mod export_file;
mod geometry;
// 实例上的长任务：导出 / 生成实例信息 / 生成服务器实例，删实例前要查
pub(crate) mod instance_task;
mod lifecycle;
mod modpack_task;
mod registry;
// 自定义整页站点的公共层：整包句柄、越界防护与协议响应，custom_home / custom_ui 共用
mod web_assets;
// 三种换皮包共用的类型判定与坏包校验，见 theme/mod.rs
mod theme;

/// 集成测试用的内部实现入口，只给 `tests/` 下的用例取用
///
/// 主题包判定与整包句柄都是窗口模块的内部实现，用例却编在另一个 crate 里
/// （见 tests/theme_test.rs），只能经这里再导出。**不属于对外接口**，业务代码不要用。
#[doc(hidden)]
pub mod test_api {
    pub use super::theme::{ThemeInfo, ThemeKind, detect_theme, meta_of_archive};
    pub use super::web_assets::{
        MAX_ARCHIVE_BYTES, MAX_ENTRIES, MAX_ENTRY_BYTES, MAX_TOTAL_BYTES, WebArchive,
        replace_atomic,
    };
}

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

use std::sync::atomic::{AtomicBool, Ordering};

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

/// 是否有会自己发系统通知的长任务在跑
///
/// 资源下载与整合包安装在结束时各发一条通知，下载队列清空时就不必再补一条泛化的
/// 下载完成。资源任务要等下载器移除任务之后才写终态，所以此刻仍能读到它未结束。
pub(crate) fn has_self_notifying_task() -> bool {
    add_resource::has_active() || modpack_task::has_active()
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
/// 传空 / 不传时退回下面按 kind 拼的老路子，只有窗口族与导出 / 服务器包 / 服务器实例带 uuid。
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
            ensure_query_ok(query)?;
            format!("index.html?{query}")
        }
        // 只有窗口族与导出 / 服务器包 / 服务器实例能带 instance，上面校验过了；这里把实例写进 URL，
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
        let id = family_window_id(family, &required_instance(instance.as_deref())?);
        // 目标实例已经写进窗口身份，已开的窗口不必、也不能再换实例
        return open_window(&app, &id.uuid, &id.label, &url_path, Some(&window));
    }

    let Some(uuid) = uuid_for_kind(&kind) else {
        return Err(format!("unknown window kind: {kind}"));
    };
    if kind == "lobby" {
        lobby::lobby_prepare(app.clone()).await?;
    }
    if let Err(error) = open_window_from_uuid(&app, &uuid, &url_path, Some(&window)) {
        if kind == "lobby" {
            lobby::lobby_close();
        }
        return Err(error);
    }
    // 新创建的窗口靠 URL query 拿目标；已存在的窗口 URL 不变，靠事件切换
    if let Some(instance) = &instance {
        let event = LogFocusDto {
            uuid: instance.clone(),
        };
        match kind.as_str() {
            "export" => export::emit_export_focus(&app, event),
            "server_instance" => server_instance::emit_server_instance_focus(&app, event),
            _ => {}
        }
    }
    Ok(())
}

/// 打开 / 关闭窗口时的实例参数校验：kind 与 instance 要匹配
///
/// - `kind`: 窗口类型，只有窗口族与导出 / 服务器包 / 服务器实例接受 instance
/// - `instance`: 目标实例 UUID；窗口族、导出、服务器包与服务器实例之外传了即报错
///
/// # 返回值
///
/// 校验不通过返回 `err_keys` 里的键：kind 不匹配为 `err.windowQuery`，非法 uuid 为 `err.uuid`
fn kind_parse_check(kind: &str, instance: &Option<String>) -> Result<(), String> {
    let accepts_instance = matches!(kind, "export" | "server_pack" | "server_instance")
        || family_of_kind(kind).is_some();
    if !accepts_instance && instance.is_some() {
        return Err(err_keys::WINDOW_QUERY.to_string());
    }
    if accepts_instance && let Some(instance) = instance {
        Uuid::parse_str(instance).map_err(|_| err_keys::UUID.to_string())?;
    }
    Ok(())
}

/// 取命令参数里的实例 uuid，窗口族与导出类窗口都靠它定位窗口
///
/// - `instance`: 前端传来的实例 uuid 字符串
///
/// # 返回值
///
/// 缺失返回 `err.windowInstanceMissing`，非法返回 `err.uuid`
fn required_instance(instance: Option<&str>) -> Result<Uuid, String> {
    let instance = instance.ok_or_else(|| err_keys::WINDOW_INSTANCE_MISSING.to_string())?;
    Uuid::parse_str(instance).map_err(|_| err_keys::UUID.to_string())
}

/// 校验前端拼好的 URL query：不能带裸 `#` 与控制字符
///
/// 前端那份 query 由 `URLSearchParams` 编出来，本来就不会有这些字符；
/// 这里只做一次兜底，免得畸形串被拼进 webview 地址。
///
/// - `query`: 不带 `?` 的 query，空串也接受
///
/// # 返回值
///
/// 合法返回 `Ok(())`，否则返回 `err.windowQuery`
fn ensure_query_ok(query: &str) -> Result<(), String> {
    if query.contains('#') || query.chars().any(char::is_control) {
        return Err(String::from(err_keys::WINDOW_QUERY));
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
        let id = family_window_id(family, &required_instance(instance.as_deref())?);
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
    ensure_query_ok(&query)?;
    let url_path = if query.is_empty() {
        String::from("index.html")
    } else {
        format!("index.html?{query}")
    };

    // 窗口族按实例定位到族里那一个；静态窗口按 kind 查表
    let id = match family_of_kind(&kind) {
        Some(family) => family_window_id(family, &required_instance(instance.as_deref())?),
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
#[cfg(windows)]
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
/// `kind` 取 `folder` / `archive` / `serverPack` / `shortcut` / `other`；
/// `shortcut` 时 `url` 为解析出的地址，其余情况为空串
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
        Some("zip") if export::server_pack::is_server_pack(std::path::Path::new(&path)) => {
            other("serverPack")
        }
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

/// 整份读成文本；只给拖入探测用，调用方已按后缀与大小筛过
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
    use super::{
        clean_url, find_http, main_window_label, read_shortcut_url, take_startup_quiet,
        window_probe_drop, work_area_is_full_monitor,
    };
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

    /// 只有主窗口的第一次显示不抢前台，其余照常
    #[test]
    fn startup_quiet_only_for_first_main_show() {
        crate::windows::quiet_main_window_startup();
        assert!(!take_startup_quiet("mml-log-x"), "别的窗口不该走不抢前台");
        assert!(take_startup_quiet(main_window_label()), "主窗口第一次要走");
        assert!(
            !take_startup_quiet(main_window_label()),
            "第二次起照常拿焦点"
        );
    }

    /// 工作区与显示器完全重合时判定为「平台没给任务栏信息」
    #[test]
    fn full_work_area_matches_monitor() {
        let position = tauri::PhysicalPosition::new(0, 0);
        let size = tauri::PhysicalSize::new(1920, 1080);

        let full = tauri::PhysicalRect { position, size };
        assert!(work_area_is_full_monitor(&full, &position, &size));

        let bottom_bar = tauri::PhysicalRect {
            position,
            size: tauri::PhysicalSize::new(1920, 1032),
        };
        assert!(!work_area_is_full_monitor(&bottom_bar, &position, &size));

        let left_bar = tauri::PhysicalRect {
            position: tauri::PhysicalPosition::new(72, 0),
            size: tauri::PhysicalSize::new(1848, 1080),
        };
        assert!(!work_area_is_full_monitor(&left_bar, &position, &size));
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

// ---------------- 启动时的显示 ----------------

/// 下一次显示主窗口是否只显示、不抢前台；被 `take_startup_quiet` 消费一次即复位
static QUIET_STARTUP: AtomicBool = AtomicBool::new(false);

/// 标记下一次显示主窗口不要抢前台，setup 阶段建主窗口之前调一次
///
/// 应用由系统、脚本或开发期的 watcher 拉起来时，把前台从用户正在用的窗口抢走会打断操作，
/// 所以**进程启动后的第一次**主窗口显示只显示、不激活，之后照常拿焦点。
pub(crate) fn quiet_main_window_startup() {
    QUIET_STARTUP.store(true, Ordering::SeqCst);
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
fn take_startup_quiet(label: &str) -> bool {
    label == main_window_label() && QUIET_STARTUP.swap(false, Ordering::SeqCst)
}

/// 显示窗口但不激活：Windows 走 `SW_SHOWNOACTIVATE`，前台窗口保持不动
///
/// - `window`: 要显示的窗口
///
/// # 返回值
///
/// 显示成功返回 `Ok(())`，取窗口句柄失败返回错误文案
#[cfg(windows)]
fn show_without_activate(window: &WebviewWindow) -> Result<(), String> {
    use windows::Win32::UI::WindowsAndMessaging::{SW_SHOWNOACTIVATE, ShowWindow};

    let hwnd = window.hwnd().map_err(|err| err.to_string())?;
    // SAFETY: 句柄取自本窗口，调用期间窗口仍然存活；只改显示状态，不解引用其它指针
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
    Ok(())
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
/// 窗口建出来时是 `decorations: true` + 隐藏的；
/// 这里在原生 frame 上激活装饰、再显示：
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
        self::effects::apply(&window, &crate::gui_config::get().client.window_effect);
        return Err(format!("decoration activate failed: {error}"));
    }

    // macOS 的红黄绿是 AppKit 原生按钮，位置必须在激活之后、显示之前定好，
    // 见插件 docs/platforms.md。第一个参数是关闭钮距左缘的点数，第二个加在按钮高度上
    // 形成容器高度（AppKit 在容器里垂直居中）：按钮 14 + 38 = 52，与 --titlebar-h 对齐
    #[cfg(target_os = "macos")]
    {
        if is_april_fools() {
            // 标题栏按另一套样式展示时窗口按钮也换边：原生红黄绿要藏掉，
            // 否则会和前端画的那套并存
            if let Err(error) = hide_native_traffic_lights(&window) {
                mml_log::info(crate::log_text::LogText::TrafficLightsFailed { error }.text());
            }
        } else if let Err(error) = window.set_traffic_lights_inset(16.0, 38.0).await {
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
    // 材质要在窗口显示之后铺：隐藏状态下铺的会被随后 activate_decoration 改窗口样式一起丢掉
    self::effects::apply(&window, &crate::gui_config::get().client.window_effect);
    Ok("custom")
}

/// 按本地日期判断今天是否要换用另一套标题栏样式
///
/// # 返回值
///
/// 需要换用时返回 `true`
#[cfg(target_os = "macos")]
fn is_april_fools() -> bool {
    use chrono::{Datelike, Local};

    let now = Local::now();
    now.month() == 4 && now.day() == 1
}

/// 藏掉 macOS 的原生红黄绿，改由前端按对调后的样式自己画
///
/// - `window`: 目标窗口
///
/// # 返回值
///
/// 取不到原生窗口或主线程调度失败时返回错误文案
#[cfg(target_os = "macos")]
fn hide_native_traffic_lights(window: &WebviewWindow) -> Result<(), String> {
    use objc2_app_kit::{NSWindow, NSWindowButton};
    use tauri::Manager;

    let handle = window.app_handle().clone();
    let window = window.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    handle
        .run_on_main_thread(move || {
            let result = (|| -> Result<(), String> {
                let pointer = window.ns_window().map_err(|err| err.to_string())?;
                if pointer.is_null() {
                    return Ok(());
                }
                // SAFETY: 主线程上执行；句柄取自本窗口且期间存活，只改按钮可见性
                unsafe {
                    let ns_window = &*(pointer as *const NSWindow);
                    for kind in [
                        NSWindowButton::CloseButton,
                        NSWindowButton::MiniaturizeButton,
                        NSWindowButton::ZoomButton,
                    ] {
                        if let Some(button) = ns_window.standardWindowButton(kind) {
                            button.setHidden(true);
                        }
                    }
                }
                Ok(())
            })();
            let _ = tx.send(result);
        })
        .map_err(|err| err.to_string())?;
    rx.recv().map_err(|err| err.to_string())?
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
#[gui_macros::command]
pub fn window_is_fullscreen(window: WebviewWindow) -> bool {
    window.is_fullscreen().unwrap_or(false)
}

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
fn work_area_is_full_monitor(
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
    // 存量映射正在搬文件：重启会直接把进程杀掉
    if settings::resource_link::running() {
        return;
    }
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
                mml_log::error(
                    LogText::RestartLauncherError {
                        error: err.to_string(),
                    }
                    .text(),
                );
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

/// 设置密码并锁定客户端设置
///
/// 盐与哈希写运行目录的隐藏文件，见 client_lock 模块。
///
/// - `app`: 应用句柄，变更后广播 `client-config-change`
/// - `password`: 新密码，不能为空
///
/// # 返回值
///
/// 成功返回 `Ok(())`；密码为空或两份锁文件都写不进去时返回错误
#[gui_macros::command]
pub fn window_client_lock_set(app: AppHandle, password: String) -> Result<(), String> {
    crate::client_lock::set(&password)?;
    emit_client_config_change(&app, crate::gui_config::get().client.into());
    Ok(())
}

/// 解除客户端设置锁定，删除隐藏的锁文件
///
/// - `app`: 应用句柄，变更后广播 `client-config-change`
///
/// # 返回值
///
/// 成功返回 `Ok(())`；删不掉锁文件时返回错误
#[gui_macros::command]
pub fn window_client_lock_clear(app: AppHandle) -> Result<(), String> {
    crate::client_lock::clear()?;
    emit_client_config_change(&app, crate::gui_config::get().client.into());
    Ok(())
}

/// 校验解锁密码
///
/// - `password`: 用户输入的密码
///
/// # 返回值
///
/// 密码正确或根本没有锁定时返回 `true`
#[gui_macros::command]
pub fn window_client_lock_verify(password: String) -> bool {
    crate::client_lock::verify(&password)
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
