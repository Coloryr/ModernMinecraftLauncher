//! 自定义界面的文件与常驻句柄
//!
//! 自定义 CSS 是一个 zip 包，放在 `custom_ui/css/<id>.zip`，包根目录要有 `custom.css`，
//! 图片等静态资源放子目录、CSS 里用相对路径引用；每份包打开成常驻句柄，由 `mml-ui` 协议
//! 按条目现读。整页包同样打开成常驻句柄，与自定义主页面共用 [`WebArchive`]。
//!
//! 所有写入都先判定类型、校验整包，再原子替换，失败不碰已有文件；校验本身在
//! `windows/theme`，与另外两种换皮包共用。

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use crate::dtos::CustomCssItemDto;
use crate::log_text::LogText;
use crate::windows::theme::{self, ThemeKind};
use crate::windows::web_assets::{WebArchive, replace_atomic};

use super::{CSS_FILE, INDEX_FILE, SCRIPT_FILE, css_dir, css_package, page_file};

/// 进程内常驻的整页包句柄；`None` 表示没导入过
static PAGE: OnceLock<Mutex<Option<Arc<WebArchive>>>> = OnceLock::new();
/// 进程内常驻的自定义 CSS 包句柄：条目 id → 整包
static CSS: OnceLock<Mutex<HashMap<String, Arc<WebArchive>>>> = OnceLock::new();

/// 取整页包句柄所在的格子，首次调用时初始化为 `None`
fn page_cell() -> &'static Mutex<Option<Arc<WebArchive>>> {
    PAGE.get_or_init(|| Mutex::new(None))
}

/// 取 CSS 包句柄表所在的格子，首次调用时初始化为空表
fn css_cell() -> &'static Mutex<HashMap<String, Arc<WebArchive>>> {
    CSS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 取当前整页包句柄
///
/// # 返回值
///
/// 已导入且能打开时的句柄；没导入或打不开时返回 `None`
pub(crate) fn page_handle() -> Option<Arc<WebArchive>> {
    page_cell().lock().ok()?.clone()
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

/// 重开全部常驻句柄：页包与每份 CSS 包，文件不在就清空
///
/// 启动 / 导入 / 删除时各调一次。包损坏、超限或没有入口文件时对应条目直接不放进句柄表，
/// 协议层自然回 404，界面回落到内置样式。
pub(crate) fn reload() {
    reload_page();
    reload_css();
}

/// 打开或重开整页包句柄
fn reload_page() {
    let zip = page_file();
    let loaded = if zip.is_file() {
        match WebArchive::open(&zip) {
            Ok(archive) if archive.has_entry(INDEX_FILE) || archive.has_entry(SCRIPT_FILE) => {
                Some(Arc::new(archive))
            }
            Ok(_) => {
                mml_log::warn(
                    LogText::ArchiveIndexMissing {
                        name: INDEX_FILE.to_string(),
                    }
                    .text(),
                );
                None
            }
            Err(err) => {
                mml_log::warn(LogText::ArchiveOpenFailed { error: err }.text());
                None
            }
        }
    } else {
        None
    };
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

/// 导入整页包：判定类型并做整包校验后覆盖 `page.zip`，并重开常驻句柄
///
/// 校验必须发生在替换之前，否则一个没有入口页、带加密条目或路径越界的包会把用户
/// 正在用的页面顶掉。
///
/// - `path`: 用户选的 zip 路径
///
/// # 返回值
///
/// 导入成功返回 `()`；类型与入口不符或 zip 不合法时返回对应错误键
pub(crate) fn import_page(path: &Path) -> Result<(), String> {
    theme::detect_theme(path, Some(ThemeKind::Page))?;
    replace_atomic(path, &page_file())?;
    reload_page();
    Ok(())
}

/// 删除整页包并释放常驻句柄
///
/// # 返回值
///
/// 删除文件失败返回 IO 错误信息
pub(crate) fn remove_page() -> Result<(), String> {
    let file = page_file();
    if file.exists() {
        std::fs::remove_file(&file).map_err(|err| err.to_string())?;
    }
    reload_page();
    Ok(())
}
