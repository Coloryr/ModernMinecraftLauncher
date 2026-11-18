//! 自定义界面的文件与常驻句柄
//!
//! 自定义 CSS 是一个 zip 包，放在 `custom_ui/css/<id>.zip`，包根目录要有 `custom.css`，
//! 图片等静态资源放子目录、CSS 里用相对路径引用；每份包打开成常驻句柄，由 `mml-ui` 协议
//! 按条目现读。主题包放在 `custom_ui/pages/<id>.zip`，同样打开成常驻句柄，协议只伺服
//! 当前选中的那一份。
//!
//! 所有写入都先判定类型、校验整包，再原子替换，失败不碰已有文件；校验本身在
//! `windows/theme`，与另外两种换皮包共用。

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use crate::dtos::{CustomCssItemDto, CustomUiPageDto};
use crate::gui_config::{self, CustomPageItemObj};
use crate::log_text::LogText;
use crate::windows::theme::{self, ThemeInfo, ThemeKind};
use crate::windows::web_assets::{WebArchive, replace_atomic};

use super::{
    CSS_FILE, INDEX_FILE, SCRIPT_FILE, css_dir, css_package, page_file, page_package, pages_dir,
};

/// 进程内常驻的主题包句柄：条目 id → 整包
static PAGES: OnceLock<Mutex<HashMap<String, Arc<WebArchive>>>> = OnceLock::new();
/// 进程内常驻的自定义 CSS 包句柄：条目 id → 整包
static CSS: OnceLock<Mutex<HashMap<String, Arc<WebArchive>>>> = OnceLock::new();

/// 取主题包句柄表所在的格子，首次调用时初始化为空表
fn page_cell() -> &'static Mutex<HashMap<String, Arc<WebArchive>>> {
    PAGES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 取 CSS 包句柄表所在的格子，首次调用时初始化为空表
fn css_cell() -> &'static Mutex<HashMap<String, Arc<WebArchive>>> {
    CSS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 取指定主题包句柄
///
/// - `id`: 条目 id，即 `pages/<id>.zip` 的文件名
///
/// # 返回值
///
/// 已导入且能打开时的句柄；没导入、打不开或没有入口文件时返回 `None`
pub(crate) fn page_handle_of(id: &str) -> Option<Arc<WebArchive>> {
    page_cell().lock().ok()?.get(id).cloned()
}

/// 取当前选中的主题包句柄
///
/// 选中项文件缺失时退回旧的单包 `page.zip`，让升级前的配置还能用。
///
/// # 返回值
///
/// 有可用整页包时返回句柄；没有时返回 `None`
pub(crate) fn page_handle() -> Option<Arc<WebArchive>> {
    let id = gui_config::get().client.custom_page_id;
    if let Some(archive) = page_handle_of(&id) {
        return Some(archive);
    }
    let zip = page_file();
    if !zip.is_file() {
        return None;
    }
    let archive = WebArchive::open(&zip).ok()?;
    (archive.has_entry(INDEX_FILE) || archive.has_entry(SCRIPT_FILE)).then(|| Arc::new(archive))
}

/// 按条目 id 取 CSS 包句柄
///
/// - `id`: 条目 id，即 `css/<id>.zip` 的文件名
///
/// # 返回值
///
/// 已导入且能打开时的句柄；没导入、打不开或没有 `custom.css` 时返回 `None`
pub(crate) fn css_handle(id: &str) -> Option<Arc<WebArchive>> {
    css_cell().lock().ok()?.get(id).cloned()
}

/// 重开全部常驻句柄：主题包与每份 CSS 包，文件不在就清空
///
/// 启动 / 导入 / 删除时各调一次。包损坏、超限或没有入口文件时对应条目直接不放进句柄表，
/// 协议层自然回 404，界面回落到内置样式。
pub(crate) fn reload() {
    migrate_legacy_page();
    reload_pages();
    reload_css();
}

/// 把旧的单包 `page.zip` 搬成 `pages/<id>.zip`，并写进配置
///
/// 升级前只有一份整页包；不迁移的话列表里一个新条目都没有，用户会以为包丢了。
fn migrate_legacy_page() {
    let legacy = page_file();
    if !legacy.is_file() {
        return;
    }
    let mut config = gui_config::get();
    if !config.client.custom_pages.is_empty() {
        return;
    }
    let Ok(archive) = WebArchive::open(&legacy) else {
        return;
    };
    if !archive.has_entry(INDEX_FILE) && !archive.has_entry(SCRIPT_FILE) {
        return;
    }
    let name = theme::meta_of_archive(&archive, ThemeKind::Page)
        .name
        .trim()
        .to_string();
    let name = if name.is_empty() {
        String::from("page.zip")
    } else {
        name
    };
    if std::fs::create_dir_all(pages_dir()).is_err() {
        return;
    }
    let id = uuid::Uuid::new_v4().to_string();
    if std::fs::rename(&legacy, page_package(&id)).is_err() {
        return;
    }
    config.client.custom_pages.push(CustomPageItemObj {
        id: id.clone(),
        name,
    });
    config.client.custom_page_id = id;
    gui_config::set(config);
}

/// 重新扫描 `pages/`，把每份能打开的包放进句柄表
fn reload_pages() {
    let mut loaded = HashMap::new();
    if let Ok(entries) = std::fs::read_dir(pages_dir()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("zip") {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|name| name.to_str()) else {
                continue;
            };
            if uuid::Uuid::parse_str(id).is_err() {
                continue;
            }
            match WebArchive::open(&path) {
                Ok(archive) if archive.has_entry(INDEX_FILE) || archive.has_entry(SCRIPT_FILE) => {
                    loaded.insert(id.to_string(), Arc::new(archive));
                }
                Ok(_) => {
                    mml_log::warn(
                        LogText::ArchiveIndexMissing {
                            name: INDEX_FILE.to_string(),
                        }
                        .text(),
                    );
                }
                Err(err) => {
                    mml_log::warn(LogText::ArchiveOpenFailed { error: err }.text());
                }
            }
        }
    }
    if let Ok(mut cell) = page_cell().lock() {
        *cell = loaded;
    }
}

/// 重新扫描 `css/`，把每份能打开的包放进句柄表
fn reload_css() {
    let mut loaded = HashMap::new();
    if let Ok(entries) = std::fs::read_dir(css_dir()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("zip") {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|name| name.to_str()) else {
                continue;
            };
            if uuid::Uuid::parse_str(id).is_err() {
                continue;
            }
            match WebArchive::open(&path) {
                Ok(archive) if archive.has_entry(CSS_FILE) => {
                    loaded.insert(id.to_string(), Arc::new(archive));
                }
                Ok(_) => {
                    mml_log::warn(
                        LogText::CustomUiArchiveMissing {
                            name: CSS_FILE.to_string(),
                        }
                        .text(),
                    );
                }
                Err(err) => {
                    mml_log::warn(LogText::CustomUiArchiveOpenFailed { error: err }.text());
                }
            }
        }
    }
    if let Ok(mut cell) = css_cell().lock() {
        *cell = loaded;
    }
}

/// 安装一份自定义 CSS 包：判定类型并校验后按 uuid 落成 `css/<id>.zip`
///
/// 显示名优先取包根 theme.json 的 name，没有清单时用导入文件名，方便用户区分多份包。
///
/// - `path`: 用户选的 zip 路径
///
/// # 返回值
///
/// 新条目，默认启用；没有 `custom.css` 返回 `err.themeMissingEntry`，
/// 类型或 zip 不合法时返回对应错误键
pub(crate) fn import_css(path: &Path) -> Result<CustomCssItemDto, String> {
    let info = theme::detect_theme(path, Some(ThemeKind::Css))?;
    let name = if info.name.is_empty() {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| String::from("custom.css"))
    } else {
        info.name
    };
    let id = uuid::Uuid::new_v4().to_string();
    replace_atomic(path, &css_package(&id))?;
    reload_css();
    Ok(CustomCssItemDto {
        id,
        name,
        enabled: true,
    })
}

/// 删除一份自定义 CSS 包，条目由前端从配置里去掉
///
/// - `id`: 条目 id，只接受 uuid，挡掉拼路径
///
/// # 返回值
///
/// id 不合法返回 `err.customUiId`；文件本来就不在也算成功
pub(crate) fn remove_css(id: &str) -> Result<(), String> {
    if uuid::Uuid::parse_str(id).is_err() {
        return Err(String::from(crate::err_keys::CUSTOM_UI_ID));
    }
    let file = css_package(id);
    if file.exists() {
        std::fs::remove_file(&file).map_err(|err| err.to_string())?;
    }
    reload_css();
    Ok(())
}

/// 导入一个主题包：判定类型并做整包校验后落成 `pages/<id>.zip`
///
/// 校验必须发生在替换之前，否则一个没有入口页、带加密条目或路径越界的包会把用户
/// 正在用的页面顶掉。导入只负责文件与状态，选中哪一条由前端写配置决定。
///
/// - `path`: 用户选的 zip 路径
///
/// # 返回值
///
/// 新条目的 id / 显示名 / 文件数 / 主题元数据；类型与入口不符或 zip 不合法时返回对应错误键
pub(crate) fn import_page(path: &Path) -> Result<CustomUiPageDto, String> {
    let info = theme::detect_theme(path, Some(ThemeKind::Page))?;
    let name = if info.name.is_empty() {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| String::from("page.zip"))
    } else {
        info.name
    };
    let id = uuid::Uuid::new_v4().to_string();
    std::fs::create_dir_all(pages_dir()).map_err(|err| err.to_string())?;
    replace_atomic(path, &page_package(&id))?;
    reload_pages();
    Ok(page_dto(&id, name))
}

/// 删除一个主题包并释放常驻句柄
///
/// - `id`: 条目 id，只接受 uuid，挡掉拼路径
///
/// # 返回值
///
/// id 不合法返回 `err.customUiId`；文件本来就不在也算成功
pub(crate) fn remove_page(id: &str) -> Result<(), String> {
    if uuid::Uuid::parse_str(id).is_err() {
        return Err(String::from(crate::err_keys::CUSTOM_UI_ID));
    }
    let file = page_package(id);
    if file.exists() {
        std::fs::remove_file(&file).map_err(|err| err.to_string())?;
    }
    reload_pages();
    Ok(())
}

/// 按 id 与显示名拼一条状态里的主题包
fn page_dto(id: &str, name: String) -> CustomUiPageDto {
    let archive = page_handle_of(id);
    let meta = archive
        .as_ref()
        .map(|archive| theme::meta_of_archive(archive, ThemeKind::Page))
        .unwrap_or_else(|| ThemeInfo::plain(ThemeKind::Page));
    CustomUiPageDto {
        id: id.to_string(),
        name,
        file_count: archive.as_ref().map(|a| a.file_count()).unwrap_or(0),
        kind: meta.kind.into(),
        version: meta.version,
        declared: meta.declared,
    }
}
