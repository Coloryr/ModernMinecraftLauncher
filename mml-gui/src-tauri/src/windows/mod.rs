//! 窗口模块
//!
//! 目录只分两类：
//! - `<窗口>/`：各窗口自己的命令、模型与实现，见各模块头部
//! - `common/`：其余全部共享代码：窗口注册表、几何、生命周期、壳层命令、NBT 会话、材质、拖放桥等
//!
//! 本文件是窗口模块根：声明各窗口，把 common 的实现按旧路径再导出。
//! 生命周期在 `common/lifecycle`，旧路径 `crate::windows::lifecycle::*` 仍可用。

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

pub(crate) mod common;

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

// 旧路径兼容：这些实现已收进 common，窗口模块仍按 crate::windows::xxx 引用。
pub(crate) use self::common::{
    effects, export_file, geometry, instance_task, lifecycle, modpack_task, nbt_session, registry,
    shell, theme, web_assets,
};
#[cfg(windows)]
pub(crate) use self::common::{file_drop, webview_keys};

pub use self::geometry::init;
pub use self::lifecycle::{on_window_event, show_main_window};
pub(crate) use self::lifecycle::{model_for_kind, window_model};
pub(crate) use self::registry::family_of_label;

// 壳层命令按原路径再导出，前端与集成测试都还用 `windows::window_*` 这一层
pub(crate) use self::shell::chrome::quiet_main_window_startup;
#[cfg(windows)]
pub(crate) use self::shell::events::emit_file_drop;
#[cfg_attr(not(target_os = "macos"), allow(unused_imports))]
pub(crate) use self::shell::events::emit_menu_action;
pub use self::shell::chrome::{
    window_activate_decoration, window_fill_screen, window_is_fullscreen, window_is_maximized,
    window_minimize, window_open_devtools, window_set_fullscreen, window_set_title,
    window_start_dragging, window_toggle_maximize,
};
pub use self::shell::drop::{window_probe_drop, window_url_of_text};
pub use self::shell::gui_config::{
    window_client_lock_clear, window_client_lock_set, window_client_lock_verify,
    window_get_gui_config, window_save_gui_config,
};
pub use self::shell::lifecycle::{
    window_close_other_windows, window_drop_model, window_ensure_model, window_restart_app,
};
pub use self::shell::open::{
    window_close_window, window_open_window, window_reopen_window,
};
pub(crate) use self::shell::shared::{
    has_self_notifying_task, instance_handle, instance_snapshot, main_window_label, parse_group_id,
    summon_main_window,
};
pub(crate) use self::shell::upgrade;

use crate::dtos::WindowSizeDto;

/// 取各窗口的默认宽高，无历史几何时创建窗口就用它，与 WINDOWS_INFO 同源
///
/// 命令留在 `mod.rs`：`bindings.ts` 的 `windows` 分组按组键首次出现排序，
/// 这里保留一条命令把分组锚在最前，拆文件不会让前端契约产生顺序 diff。
///
/// # 返回值
///
/// 每个已注册窗口一条，含 `kind`、默认宽与默认高；宽高单位是逻辑像素的客户区尺寸
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub fn window_get_window_sizes() -> Vec<WindowSizeDto> {
    shell::open::window_sizes()
}

#[cfg(test)]
#[path = "../../tests/unit/windows_mod_tests.rs"]
mod tests;
