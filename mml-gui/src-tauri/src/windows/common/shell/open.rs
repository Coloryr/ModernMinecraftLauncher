//! 窗口开关命令
//!
//! 前端按窗口 kind 打开 / 关闭 / 重开窗口；窗口身份由 `registry` 决定，
//! 真正的建窗与关窗在 `lifecycle`。命令都标了窗口分组，前端仍是 `commands.windows.*`。

use tauri::{AppHandle, Manager, WebviewWindow};
use uuid::Uuid;

use crate::dtos::{LogFocusDto, WindowSizeDto};
use crate::err_keys;

use crate::windows::lifecycle::{
    close_window, close_window_from_uuid, open_window, open_window_from_uuid,
};
use crate::windows::registry::{
    FamilyWindowId, WINDOWS_INFO, family_of_kind, family_window_id, uuid_for_kind,
};
use crate::windows::{export, lobby, server_instance};

/// 取各窗口的默认宽高，供 `mod.rs` 的 `window_get_window_sizes` 命令转调
pub(crate) fn window_sizes() -> Vec<WindowSizeDto> {
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
#[gui_macros::ipc_group("windows")]
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
    // 没给就退回按 kind 拼 URL：窗口族、导出、服务器包与生成服务器实例可带 uuid
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
    // 新窗口从 URL 取目标；导出与生成服务器实例窗口已存在时靠事件切换目标
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
#[gui_macros::ipc_group("windows")]
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
#[gui_macros::ipc_group("windows")]
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
