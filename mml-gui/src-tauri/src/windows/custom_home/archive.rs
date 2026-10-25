//! 自定义主页的整包句柄：打开 / 常驻 / 重载 / 导入
//!
//! 从 `custom_home/mod.rs` 拆出来的。整包是 zip，里面有 `index.html` 或 `custom.js` 与资源，句柄常驻在
//! `HANDLE` 里以免每次请求重开；`reload` 在配置变更后被 `lib.rs` 调用。
//!
//! 条目索引与读取在 `windows/web_assets.rs`，与自定义界面的整页包共用一份；
//! 导入期的类型判定与坏包校验在 `windows/theme`，与另外两种换皮包共用。

use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use crate::dtos::CustomHomeInfoDto;
use crate::log_text::LogText;
use crate::windows::theme::{self, ThemeKind};
use crate::windows::web_assets::replace_atomic;

use super::{current_info, zip_path};

pub(crate) use crate::windows::theme::{CSS_FILE, INDEX_FILE, SCRIPT_FILE};

/// 本窗口的整包句柄类型：读取与索引都在 `WebArchive`，这里只按旧名引用
pub(crate) use crate::windows::web_assets::WebArchive as HomeArchive;

/// 进程内常驻的包句柄；`None` 表示没导入过
pub(super) static HANDLE: OnceLock<Mutex<Option<Arc<HomeArchive>>>> = OnceLock::new();

/// 取常驻句柄所在的格子，首次调用时初始化为 `None`
///
/// # 返回值
///
/// 进程内唯一的句柄格子，读写方自行加锁
pub(super) fn handle_cell() -> &'static Mutex<Option<Arc<HomeArchive>>> {
    HANDLE.get_or_init(|| Mutex::new(None))
}

/// 取当前句柄
///
/// # 返回值
///
/// 已导入且能打开时的句柄；没导入或打不开时返回 `None`
pub(crate) fn open_handle() -> Option<Arc<HomeArchive>> {
    handle_cell().lock().ok()?.clone()
}

/// 旧版本把整包存成 `custom_home.zip`，新版换成不带扩展名的 `home`
const LEGACY_FILE: &str = "custom_home.zip";

/// 把旧版本留下的 `custom_home.zip` 搬到 `home`
///
/// 不搬的话升级后旧用户的包会被当成没导入；新文件已在时不动，改名失败就按没导入处理。
///
/// - `zip`: 新版整包路径
fn migrate_legacy(zip: &Path) {
    if zip.is_file() {
        return;
    }
    let legacy = zip.with_file_name(LEGACY_FILE);
    if legacy.is_file() {
        let _ = std::fs::rename(&legacy, zip);
    }
}

/// 打开或重开常驻句柄：zip 在就打开，不在就清空
///
/// 导入 / 删除 / 启动时各调一次。包损坏、超出限额或没有入口页时句柄保持为 None，
/// 协议层自然回 404，主窗口回落到内置主页。
pub(crate) fn reload() {
    let zip = zip_path();
    migrate_legacy(&zip);
    let loaded = if zip.is_file() {
        match HomeArchive::open(&zip) {
            Ok(archive) => {
                // 打开时就把入口定位算出来：包不合法也当作没导入，
                // 免得主窗口拿到一个必然 404 的 entry_url
                if archive.has_entry(INDEX_FILE) || archive.has_entry(SCRIPT_FILE) {
                    Some(Arc::new(archive))
                } else {
                    mml_log::warn(
                        LogText::ArchiveIndexMissing {
                            name: INDEX_FILE.to_string(),
                        }
                        .text(),
                    );
                    None
                }
            }
            Err(err) => {
                mml_log::warn(
                    LogText::ArchiveOpenFailed {
                        error: err.to_string(),
                    }
                    .text(),
                );
                None
            }
        }
    } else {
        None
    };
    if let Ok(mut cell) = handle_cell().lock() {
        *cell = loaded;
    }
}

/// 导入 zip：**不解压、不落任何目录**，先判定类型并校验整包，再存成 `home`
///
/// 导入语义是全量替换，且中途失败不能破坏服主已有的包：校验全部通过后才经
/// `home.tmp` 中转，最后 rename 覆盖上架并重开常驻句柄。
///
/// 之所以在替换前就把整包读一遍：类型判定与坏包校验必须发生在替换之前，否则一个
/// 没有入口、带加密条目或路径越界的包会把服主原来能用的主页顶掉。
///
/// - `path`: 待导入的压缩包路径
///
/// # 返回值
///
/// 导入后的新状态；类型不符、入口缺失或 zip 非法时返回对应错误键
pub(crate) fn import_archive(path: &Path) -> Result<CustomHomeInfoDto, String> {
    theme::detect_theme(path, Some(ThemeKind::Home))?;
    replace_atomic(path, &zip_path())?;
    // 句柄换成新包；旧句柄的 Arc 还活着也不影响，它指向的是已被替换的旧文件
    reload();
    Ok(current_info())
}
