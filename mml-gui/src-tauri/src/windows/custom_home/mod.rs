//! 自定义主页面：服主导入的整页 HTML 或窗口组件替换启动器主页内容区
//!
//! **压缩包是唯一产物，全程不落盘解压**：服主导入的 zip 存成 `base_dir/home`，不带扩展名，
//! 启动时打开成一个常驻句柄 [`HomeArchive`]，`mml-home` 协议的每个请求
//! **直接从包内按条目读取**，即 [`BaseArchive::read`]，内部 seek 到条目偏移解压到内存，
//! 不会把文件铺到磁盘上。页面经自定义协议 `mml-home` 供主窗口的 iframe 加载，
//! 地址形如 `http://mml-home.localhost/...`，见 [`home_base_url`]。
//!
//! | 部分 | 说明 |
//! | --- | --- |
//! | [`custom_home_import`] | 导入 zip：校验入口后原子替换 `home`，并重开句柄 |
//! | [`HomeArchive`] | 常驻的只读句柄，`BaseArchive` 内部用 `Mutex`，逐条目读取 |
//! | [`custom_home_status`] | 现算状态：zip 的 mtime 即破缓存版本号，磁盘上不存派生状态 |
//! | [`custom_home_remove`] | 删除 zip 并释放句柄 |
//! | [`custom_home_open_dir`] | 在文件管理器里定位压缩包，便于服主替换 / 备份 |
//! | [`url_custom_home`] | `mml-home` 协议 handler，路径穿越防护的唯一强制安全项 |
//!
//! 桥接脚本是内置资源，源码在 `src-tauri/resources/custom_home_bridge.js`，
//! 不在包里：返回 HTML 时注入 `<script src="/__mml_bridge.js">`，
//! 页面据此拿到 `window.mml` 的 invoke / on / ready。脚本与注入逻辑在
//! `windows/web_assets.rs`，与自定义界面的整页包共用。
//!
//! **安全边界**：自定义页面通过 `invoke` 能调任意已注册命令，这与「服主给的整合包
//! 本身就是代码」的信任级别一致，是确认过的设计；唯一的强制项是协议层的路径穿越防护。

use std::{
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use tauri::{AppHandle, Emitter};

use crate::dtos::CustomHomeInfoDto;
use crate::listens;
use crate::windows::theme::{self, ThemeInfo, ThemeKind};

mod archive;
mod protocol;

// 按原路径再导出：`lib.rs` 注册协议处理器、配置变更后重载，都用这两个
pub(crate) use self::archive::{
    CSS_FILE, INDEX_FILE, SCRIPT_FILE, import_archive, open_handle, reload,
};
pub(crate) use self::protocol::url_custom_home;

// ================= 路径 =================

/// 运行根目录下的绝对路径
///
/// - `name`: 相对运行根目录的文件名
///
/// # 返回值
///
/// 运行根目录与 `name` 拼出的绝对路径
fn base_path(name: &str) -> PathBuf {
    mml_base::get_base_dir().join(name)
}

/// 持久压缩包的文件名，放运行根目录；刻意不带扩展名
const HOME_FILE: &str = "home";

/// 持久压缩包的路径
///
/// # 返回值
///
/// 运行根目录下的 `home`
pub(crate) fn zip_path() -> PathBuf {
    base_path(HOME_FILE)
}

/// `mml-home` 自定义协议的访问前缀
///
/// 与 `image_base_url()` 同规则：Windows 与 Android 上自定义协议被映射到
/// `http://<scheme>.localhost`，其余平台是 `<scheme>://localhost`。
///
/// # 返回值
///
/// 主窗口 iframe 拼入口地址时用的前缀
pub fn home_base_url() -> &'static str {
    if cfg!(windows) || cfg!(target_os = "android") {
        "http://mml-home.localhost"
    } else {
        "mml-home://localhost"
    }
}

// ================= 常驻句柄 =================

// ================= 状态 =================

/// 内容版本号：取**压缩包**的 mtime，单位毫秒时间戳
///
/// zip 只在服主重新导入时变化，正好是「内容变没变」的判据，用来给 iframe 破缓存。
///
/// - `zip`: 压缩包路径
///
/// # 返回值
///
/// 毫秒时间戳字符串；取不到 mtime 时返回空串
fn zip_version(zip: &Path) -> String {
    std::fs::metadata(zip)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|dur| dur.as_millis().to_string())
        .unwrap_or_default()
}

/// 现算当前状态，入口 URL 由这里拼好，前端不重算 scheme 前缀
///
/// # 返回值
///
/// 已导入情况、启用开关、类型与主题元数据、入口 / 脚本 / 样式 URL、
/// 内容版本号与包内文件数
fn current_info() -> CustomHomeInfoDto {
    let zip = zip_path();
    let handle = open_handle();
    // 句柄能读出来才算「已导入」：包损坏 / 没有入口页时这里就是 false，主窗口会回落
    let installed = handle
        .as_ref()
        .is_some_and(|h| h.has_entry(INDEX_FILE) || h.has_entry(SCRIPT_FILE));
    let has_index = handle.as_ref().is_some_and(|h| h.has_entry(INDEX_FILE));
    let content_version = if installed {
        zip_version(&zip)
    } else {
        String::new()
    };
    let url_of = |name: &str| {
        handle
            .as_ref()
            .filter(|h| h.has_entry(name))
            .map(|_| format!("{}/{name}?v={content_version}", home_base_url()))
            .unwrap_or_default()
    };
    let entry_url = if installed && has_index {
        url_of(INDEX_FILE)
    } else {
        String::new()
    };
    // 清单只在导入期校验，运行时读回不合法就按导入入口兜底，不能把状态查询变成错误
    let meta = handle
        .as_ref()
        .map(|archive| theme::meta_of_archive(archive, ThemeKind::Home))
        .unwrap_or_else(|| ThemeInfo::plain(ThemeKind::Home));
    CustomHomeInfoDto {
        installed,
        enabled: crate::gui_config::get().client.custom_home,
        kind: meta.kind.into(),
        name: meta.name,
        version: meta.version,
        declared: meta.declared,
        entry_url,
        script_url: url_of(SCRIPT_FILE),
        style_url: url_of(CSS_FILE),
        content_version,
        file_count: handle.as_ref().map(|h| h.file_count()).unwrap_or(0),
    }
}

/// 查询已导入情况与入口 URL
///
/// # 返回值
///
/// 与 `current_info` 相同的状态，目前不会失败
#[gui_macros::command]
pub fn custom_home_status() -> Result<CustomHomeInfoDto, String> {
    Ok(current_info())
}

// ================= 导入 =================

/// 自定义主页面内容变更事件，导入与删除成功后广播
///
/// 主窗口的 iframe 地址只在导入 / 删除时才变，而这两件事都不会改
/// `gui_config.client`，`client-config-change` 不会发，所以单独立一个事件，
/// 主窗口收到后重拉一次 `custom_home_status`。
///
/// - `app`: 应用句柄，事件发往所有窗口
#[gui_macros::emit]
pub fn emit_custom_home_change(app: &AppHandle) {
    let _ = app.emit(listens::CUSTOM_HOME_CHANGE, ());
}

/// 从 zip 导入自定义主页面，全量替换
///
/// 只校验 + 复制压缩包，**不落盘解压**：请求时直接从包内按条目读。
///
/// - `app`: 应用句柄，导入成功后广播变更
/// - `path`: 要导入的 zip 路径
///
/// # 返回值
///
/// 导入后的新状态；类型与入口不符或 zip 不合法时返回对应错误键
#[gui_macros::command]
pub async fn custom_home_import(app: AppHandle, path: String) -> Result<CustomHomeInfoDto, String> {
    let info = tauri::async_runtime::spawn_blocking(move || import_archive(Path::new(&path)))
        .await
        .map_err(|err| err.to_string())??;
    emit_custom_home_change(&app);
    Ok(info)
}

/// 删除已导入的包；配置里的启用开关不动，删掉后主窗口自动回落到内置主页
///
/// - `app`: 应用句柄，删除成功后广播变更
///
/// # 返回值
///
/// 删除文件失败返回 IO 错误信息
#[gui_macros::command]
pub fn custom_home_remove(app: AppHandle) -> Result<(), String> {
    let zip = zip_path();
    if zip.exists() {
        std::fs::remove_file(&zip).map_err(|err| err.to_string())?;
    }
    // 句柄指向已删除的文件：直接丢掉，后续请求回 404
    reload();
    emit_custom_home_change(&app);
    Ok(())
}

/// 在文件管理器里定位压缩包，方便服主替换包或做备份
///
/// 还没导入过时就选中运行根目录，让服主知道包该放哪。
///
/// # 返回值
///
/// 建运行根目录失败返回 IO 错误信息
#[gui_macros::command]
pub fn custom_home_open_dir() -> Result<(), String> {
    let zip = zip_path();
    let target = if zip.is_file() {
        zip
    } else {
        let dir = mml_base::get_base_dir();
        std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
        dir
    };
    mml_sys::open_helper::open_file(&target);
    Ok(())
}

// ================= mml-home 协议 =================
