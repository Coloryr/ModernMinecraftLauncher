//! 自定义界面：自定义 CSS 与完全自定义界面
//!
//! 三种「换皮」都打包成 zip 分发，各自独立，装一个不影响另外两个的开关：
//! 1. 自定义主页面：产物 `home`，开关 `client.custom_home`，设置放客户端设置；
//! 2. 自定义 CSS：可装多份，产物 `custom_ui/css/<id>.zip`，包根要有 `custom.css`，
//!    可选 `custom.js` 入口脚本，开关 `client.custom_css_on`；
//! 3. 完全自定义界面：只装一份，产物 `custom_ui/page.zip`，包根要有 `index.html`
//!    或 `custom.js`（窗口页面组件回调），可选 `custom.css`（回调页面样式），
//!    开关 `client.custom_page_on`。
//!
//! 2 与 3 的设置都在「设置 → 界面」，1 在「设置 → 客户端设置」。条目列表与开关都存
//! `gui_config.client`，文件本身在运行目录的 `custom_ui/` 下；两种包都走
//! 「常驻句柄 + 按条目现读」，**不落盘解压**，包里的静态资源也能用相对路径引用。
//!
//! 页面与样式经 `mml-ui` 协议伺服，地址形如 `http://mml-ui.localhost/index.html`，
//! CSS 是 `http://mml-ui.localhost/css/<id>/custom.css`，入口脚本同目录的 `custom.js`，
//! 见 [`ui_base_url`]。开关变化走
//! `window_save_gui_config` 的 `client-config-change`；导入 / 删除文件不改配置，
//! 单发 `custom-ui-change`，各窗口收到后重拉 [`custom_ui_status`] 并重新应用。
//!
//! **锁定功能预留**：后续要给服主 / 二次开发者一个「锁住这三项」的开关，届时加在
//! `ClientConfig` 里，并在这几个命令的入口统一拒绝即可 —— 文件操作只走本模块，
//! 前端不直接动 `custom_ui/`，所以锁只需要加一处。

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use tauri::{AppHandle, Emitter};

use crate::dtos::{CustomCssItemDto, CustomUiCssDto, CustomUiInfoDto};
use crate::listens;
use crate::windows::theme::{self, ThemeInfo, ThemeKind};

mod protocol;
mod store;

pub(crate) use self::protocol::url_custom_ui;
pub(crate) use self::store::reload;

/// 运行目录下的子目录名
pub(crate) const DIR: &str = "custom_ui";
/// 自定义 CSS 包所在的子目录
pub(crate) const CSS_DIR: &str = "css";
/// 整页包的文件名
pub(crate) const PAGE_FILE: &str = "page.zip";

// 入口文件名只有一份来源：主题模块的入口签名，三种包共用
pub(crate) use crate::windows::theme::{CSS_FILE, INDEX_FILE, SCRIPT_FILE};

/// `custom_ui/` 在运行目录下的绝对路径
pub(crate) fn dir() -> PathBuf {
    mml_base::get_base_dir().join(DIR)
}

/// 自定义 CSS 目录的绝对路径
pub(crate) fn css_dir() -> PathBuf {
    dir().join(CSS_DIR)
}

/// 一份自定义 CSS 压缩包的绝对路径
///
/// - `id`: 条目 id，即文件名
pub(crate) fn css_package(id: &str) -> PathBuf {
    css_dir().join(format!("{id}.zip"))
}

/// 整页包的绝对路径
pub(crate) fn page_file() -> PathBuf {
    dir().join(PAGE_FILE)
}

/// `mml-ui` 自定义协议的访问前缀
///
/// 与 `image_base_url()` / `home_base_url()` 同规则：Windows 与 Android 上自定义协议
/// 被映射到 `http://<scheme>.localhost`，其余平台是 `<scheme>://localhost`。
///
/// # 返回值
///
/// 前端拼 CSS 地址与整页入口时用的前缀
pub fn ui_base_url() -> &'static str {
    if cfg!(windows) || cfg!(target_os = "android") {
        "http://mml-ui.localhost"
    } else {
        "mml-ui://localhost"
    }
}

/// 文件 mtime 的毫秒时间戳
///
/// 导入 / 删除都会换掉文件，mtime 正好是「内容变没变」的判据，用来给链接与 iframe 破缓存。
///
/// - `path`: 目标文件
///
/// # 返回值
///
/// 毫秒时间戳字符串；取不到 mtime 时返回空串
fn version_of(path: &Path) -> String {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|dur| dur.as_millis().to_string())
        .unwrap_or_default()
}

/// 现算当前状态，地址由这里拼好，前端不重算 scheme 前缀
///
/// CSS 列表直接取配置里的条目，文件缺失时 `url` 为空串，包里没有 `custom.js`
/// 时 `script_url` 为空串；整页入口只在包能打开且入口页在时才有，未导入时为空串。
/// 整页包里的 `custom.js` / `custom.css` 同样按包内条目给地址，没有就是空串。
/// 整页包的类型与主题元数据从常驻句柄里的 theme.json 读回，没有清单时按 page 兜底。
///
/// # 返回值
///
/// 两个开关、CSS 条目、整页类型与元数据、整页入口 / 版本号 / 包内文件数
pub(crate) fn current_info() -> CustomUiInfoDto {
    let client = crate::gui_config::get().client;
    let css = client
        .custom_css
        .iter()
        .map(|item| {
            let archive = store::css_handle(&item.id);
            let version = version_of(&css_package(&item.id));
            // 样式表与入口脚本同目录，只有文件名不同
            let url_of = |name: &str| {
                format!(
                    "{}/{}/{}/{}?v={}",
                    ui_base_url(),
                    CSS_DIR,
                    item.id,
                    name,
                    version
                )
            };
            let url = archive
                .as_ref()
                .map(|_| url_of(CSS_FILE))
                .unwrap_or_default();
            let script_url = archive
                .as_ref()
                .filter(|archive| archive.has_entry(SCRIPT_FILE))
                .map(|_| url_of(SCRIPT_FILE))
                .unwrap_or_default();
            CustomUiCssDto {
                id: item.id.clone(),
                name: item.name.clone(),
                enabled: item.enabled,
                url,
                script_url,
            }
        })
        .collect();

    let page = store::page_handle();
    let page_installed = page.is_some();
    let page_has_index = page
        .as_ref()
        .is_some_and(|archive| archive.has_entry(INDEX_FILE));
    let page_version = if page_installed {
        version_of(&page_file())
    } else {
        String::new()
    };
    // 清单只在导入期校验，运行时读回不合法就按导入入口兜底，不能把状态查询变成错误
    let meta = page
        .as_ref()
        .map(|archive| theme::meta_of_archive(archive, ThemeKind::Page))
        .unwrap_or_else(|| ThemeInfo::plain(ThemeKind::Page));
    CustomUiInfoDto {
        css_on: client.custom_css_on,
        css,
        page_on: client.custom_page_on,
        page_installed,
        kind: meta.kind.into(),
        name: meta.name,
        version: meta.version,
        declared: meta.declared,
        page_entry_url: if page_installed && page_has_index {
            format!("{}/{}?v={}", ui_base_url(), INDEX_FILE, page_version)
        } else {
            String::new()
        },
        page_script_url: page
            .as_ref()
            .filter(|archive| archive.has_entry(SCRIPT_FILE))
            .map(|_| format!("{}/{}?v={}", ui_base_url(), SCRIPT_FILE, page_version))
            .unwrap_or_default(),
        page_style_url: page
            .as_ref()
            .filter(|archive| archive.has_entry(CSS_FILE))
            .map(|_| format!("{}/{}?v={}", ui_base_url(), CSS_FILE, page_version))
            .unwrap_or_default(),
        page_version,
        page_file_count: page.as_ref().map(|p| p.file_count()).unwrap_or(0),
    }
}

/// 查询两个开关、已安装的 CSS 与整页包状态
///
/// # 返回值
///
/// 见 [`current_info`]；目前不会失败
#[gui_macros::command]
pub fn custom_ui_status() -> Result<CustomUiInfoDto, String> {
    Ok(current_info())
}

/// 自定义界面内容变更事件，导入与删除文件成功后广播
///
/// 这些动作都不改 `gui_config.client`，`client-config-change` 不会发，所以单独立一条：
/// 各窗口据此重拉状态、重挂 CSS 链接或换整页入口；设置窗口重拉页面状态。
///
/// - `app`: 应用句柄，事件发往所有窗口
#[gui_macros::emit]
pub fn emit_custom_ui_change(app: &AppHandle) {
    let _ = app.emit(listens::CUSTOM_UI_CHANGE, ());
}

/// 安装一份自定义 CSS 包，文件存成 `custom_ui/css/<id>.zip`
///
/// 包里必须有 `custom.css`（包根，或整包只套一层目录），其余静态资源用相对路径引用。
/// 条目本身由前端追加进 `client.custom_css` 再保存配置：列表属于设置，只有设置窗口写它，
/// 后端只负责拿不到的磁盘操作。
///
/// - `app`: 应用句柄，安装成功后广播变更
/// - `path`: 用户选的 zip 路径
///
/// # 返回值
///
/// 新条目的 id / 显示名 / 默认启用；没有 `custom.css` 返回 `err.themeMissingEntry`，
/// 类型或 zip 不合法时返回对应错误键
#[gui_macros::command]
pub async fn custom_ui_import_css(
    app: AppHandle,
    path: String,
) -> Result<CustomCssItemDto, String> {
    let item = tauri::async_runtime::spawn_blocking(move || store::import_css(Path::new(&path)))
        .await
        .map_err(|err| err.to_string())??;
    emit_custom_ui_change(&app);
    Ok(item)
}

/// 删除一份自定义 CSS 文件；条目由前端从配置里去掉
///
/// - `app`: 应用句柄，删除成功后广播变更
/// - `id`: 条目 id
///
/// # 返回值
///
/// id 不合法返回 `err.customUiId`，删除失败返回 IO 错误信息
#[gui_macros::command]
pub fn custom_ui_remove_css(app: AppHandle, id: String) -> Result<(), String> {
    store::remove_css(&id)?;
    emit_custom_ui_change(&app);
    Ok(())
}

/// 导入整页包，全量替换 `page.zip`；只能有一份，再导入就是换掉旧的
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
pub async fn custom_ui_import_page(
    app: AppHandle,
    path: String,
) -> Result<CustomUiInfoDto, String> {
    tauri::async_runtime::spawn_blocking(move || store::import_page(Path::new(&path)))
        .await
        .map_err(|err| err.to_string())??;
    emit_custom_ui_change(&app);
    Ok(current_info())
}

/// 删除整页包；开关不动，删掉后主窗口回内置界面
///
/// - `app`: 应用句柄，删除成功后广播变更
///
/// # 返回值
///
/// 删除后的新状态；删除文件失败返回 IO 错误信息
#[gui_macros::command]
pub fn custom_ui_remove_page(app: AppHandle) -> Result<CustomUiInfoDto, String> {
    store::remove_page()?;
    emit_custom_ui_change(&app);
    Ok(current_info())
}

/// 在文件管理器里打开 `custom_ui/`，方便用户直接放文件或做备份
///
/// 目录不存在时先建出来，让用户知道文件该放哪。
///
/// # 返回值
///
/// 建目录失败返回 IO 错误信息
#[gui_macros::command]
pub fn custom_ui_open_dir() -> Result<(), String> {
    let dir = dir();
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    mml_sys::open_helper::open_file(&dir);
    Ok(())
}
