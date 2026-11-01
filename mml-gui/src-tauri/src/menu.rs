//! 应用菜单（macOS）
//!
//! macOS 的菜单栏是**应用级**的：`WebviewWindow::set_menu` 在 macOS 上不生效，全应用只有一份。
//! 所以这里关掉 Tauri 的默认菜单（见 lib.rs 的 `enable_macos_default_menu`），改成
//! **焦点窗口变化时按窗口类型重建一份**：主窗口给设置 / 统计这类入口，其它窗口给刷新，
//! 编辑与关闭用系统预定义项。Windows / Linux 不设原生菜单，窗口操作都在各自标题栏上，
//! [`sync`] 与 [`on_event`] 直接返回。
//!
//! 点击自建项时把动作 id 发给当前窗口，见 `windows::emit_menu_action`；前端按窗口分发。

use std::sync::Mutex;

use mml_names::Lang;
use tauri::menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Manager, Wry};

use crate::gui_config;
use crate::log_text::LogText;
use crate::windows::family_of_label;

/// 应用名，同时是第一个（应用）菜单的标题
const APP_NAME: &str = "M²L";

/// 自建菜单项的 id 前缀：只有它开头的才是本应用的动作，预定义项由系统处理
const ID_PREFIX: &str = "mml.";

/// 当前菜单对应的窗口标签：同一个窗口重复聚焦时不重建
static CURRENT: Mutex<Option<String>> = Mutex::new(None);

/// 一个窗口动作
struct Action {
    /// 动作 id，随事件发给窗口
    id: &'static str,
    /// 中文文案
    zh: &'static str,
    /// 英文文案
    en: &'static str,
}

impl Action {
    /// 取这个动作在当前语言下的文案
    ///
    /// - `lang`: 界面语言
    ///
    /// # 返回值
    ///
    /// 该语言的成品文案
    fn text(&self, lang: Lang) -> &'static str {
        match lang {
            Lang::zh_cn => self.zh,
            Lang::en_us => self.en,
        }
    }
}

/// 新开一个实例
const ADD: Action = Action {
    id: "add",
    zh: "添加实例",
    en: "New Instance",
};
/// 打开设置
const SETTINGS: Action = Action {
    id: "settings",
    zh: "设置",
    en: "Settings",
};
/// 打开游戏统计
const STATS: Action = Action {
    id: "stats",
    zh: "统计",
    en: "Statistics",
};
/// 打开收藏
const COLLECT: Action = Action {
    id: "collect",
    zh: "收藏",
    en: "Favorites",
};
/// 打开帮助
const HELP: Action = Action {
    id: "help",
    zh: "帮助",
    en: "Help",
};
/// 打开「下载资源」
const DOWNLOAD: Action = Action {
    id: "download",
    zh: "下载资源",
    en: "Browse Resources",
};
/// 刷新当前窗口的数据
const REFRESH: Action = Action {
    id: "refresh",
    zh: "刷新",
    en: "Refresh",
};

/// 该窗口类型要放进菜单的动作，空表示只有系统项
///
/// 只列前端确实接了刷新的窗口（见 `useWindowRefresh`）：菜单项点下去不能没反应。
///
/// - `kind`: 窗口类型，标签去掉 `mml-` 前缀与末尾实例 uuid 后的那一段
fn actions_of(kind: &str) -> &'static [&'static Action] {
    match kind {
        // 主窗口：放的是应用入口，与顶栏那几个按钮一致
        "main" => &[&ADD, &SETTINGS, &STATS, &COLLECT, &HELP, &REFRESH],
        // 资源管理：标题栏只有「下载资源」
        "resource" => &[&DOWNLOAD, &REFRESH],
        "account" | "stats" | "collect" | "block" | "download" | "config" | "export"
        | "server_instance" | "log" | "java_download" | "add_resource" | "add_modpack" => {
            &[&REFRESH]
        }
        _ => &[],
    }
}

/// 菜单上的固定文案
enum Text {
    /// 关于
    About,
    /// 隐藏应用
    Hide,
    /// 隐藏其他应用
    HideOthers,
    /// 显示全部
    ShowAll,
    /// 退出
    Quit,
    /// 编辑菜单标题
    Edit,
    /// 撤销
    Undo,
    /// 重做
    Redo,
    /// 剪切
    Cut,
    /// 复制
    Copy,
    /// 粘贴
    Paste,
    /// 全选
    SelectAll,
    /// 窗口菜单标题
    Window,
    /// 最小化
    Minimize,
    /// 缩放
    Zoom,
    /// 关闭窗口
    Close,
    /// 当前窗口的动作菜单标题
    Actions,
}

/// 取固定文案
///
/// 菜单是原生控件，走不了前端 i18n 表；与 tray.rs 一样中英各一套，语言取 gui_config。
///
/// - `lang`: 界面语言
/// - `which`: 要哪一条
///
/// # 返回值
///
/// 该语言的成品文案
fn text(lang: Lang, which: Text) -> &'static str {
    match (lang, which) {
        (Lang::zh_cn, Text::About) => "关于 M²L",
        (Lang::zh_cn, Text::Hide) => "隐藏 M²L",
        (Lang::zh_cn, Text::HideOthers) => "隐藏其他",
        (Lang::zh_cn, Text::ShowAll) => "显示全部",
        (Lang::zh_cn, Text::Quit) => "退出 M²L",
        (Lang::zh_cn, Text::Edit) => "编辑",
        (Lang::zh_cn, Text::Undo) => "撤销",
        (Lang::zh_cn, Text::Redo) => "重做",
        (Lang::zh_cn, Text::Cut) => "剪切",
        (Lang::zh_cn, Text::Copy) => "复制",
        (Lang::zh_cn, Text::Paste) => "粘贴",
        (Lang::zh_cn, Text::SelectAll) => "全选",
        (Lang::zh_cn, Text::Window) => "窗口",
        (Lang::zh_cn, Text::Minimize) => "最小化",
        (Lang::zh_cn, Text::Zoom) => "缩放",
        (Lang::zh_cn, Text::Close) => "关闭窗口",
        (Lang::zh_cn, Text::Actions) => "操作",
        (Lang::en_us, Text::About) => "About M²L",
        (Lang::en_us, Text::Hide) => "Hide M²L",
        (Lang::en_us, Text::HideOthers) => "Hide Others",
        (Lang::en_us, Text::ShowAll) => "Show All",
        (Lang::en_us, Text::Quit) => "Quit M²L",
        (Lang::en_us, Text::Edit) => "Edit",
        (Lang::en_us, Text::Undo) => "Undo",
        (Lang::en_us, Text::Redo) => "Redo",
        (Lang::en_us, Text::Cut) => "Cut",
        (Lang::en_us, Text::Copy) => "Copy",
        (Lang::en_us, Text::Paste) => "Paste",
        (Lang::en_us, Text::SelectAll) => "Select All",
        (Lang::en_us, Text::Window) => "Window",
        (Lang::en_us, Text::Minimize) => "Minimize",
        (Lang::en_us, Text::Zoom) => "Zoom",
        (Lang::en_us, Text::Close) => "Close Window",
        (Lang::en_us, Text::Actions) => "Actions",
    }
}

/// 按窗口类型建一份菜单
///
/// - `app`: 应用句柄，菜单挂在它上面
/// - `kind`: 窗口类型
///
/// # 返回值
///
/// 建好的菜单；失败返回 tauri 的错误
fn build(app: &AppHandle, kind: &str) -> tauri::Result<Menu<Wry>> {
    let lang = gui_config::locale();

    // 应用菜单：关于 / 隐藏 / 退出
    let about = PredefinedMenuItem::about(app, Some(text(lang, Text::About)), None)?;
    let sep_about = PredefinedMenuItem::separator(app)?;
    let hide = PredefinedMenuItem::hide(app, Some(text(lang, Text::Hide)))?;
    let hide_others = PredefinedMenuItem::hide_others(app, Some(text(lang, Text::HideOthers)))?;
    let show_all = PredefinedMenuItem::show_all(app, Some(text(lang, Text::ShowAll)))?;
    let sep_quit = PredefinedMenuItem::separator(app)?;
    let quit = PredefinedMenuItem::quit(app, Some(text(lang, Text::Quit)))?;
    let app_menu = Submenu::with_items(
        app,
        APP_NAME,
        true,
        &[
            &about,
            &sep_about,
            &hide,
            &hide_others,
            &show_all,
            &sep_quit,
            &quit,
        ],
    )?;

    // 编辑：macOS 的复制 / 粘贴快捷键由菜单项转发，没有这一栏文本框里 Cmd+C 不响应
    let undo = PredefinedMenuItem::undo(app, Some(text(lang, Text::Undo)))?;
    let redo = PredefinedMenuItem::redo(app, Some(text(lang, Text::Redo)))?;
    let sep_edit = PredefinedMenuItem::separator(app)?;
    let cut = PredefinedMenuItem::cut(app, Some(text(lang, Text::Cut)))?;
    let copy = PredefinedMenuItem::copy(app, Some(text(lang, Text::Copy)))?;
    let paste = PredefinedMenuItem::paste(app, Some(text(lang, Text::Paste)))?;
    let select_all = PredefinedMenuItem::select_all(app, Some(text(lang, Text::SelectAll)))?;
    let edit_menu = Submenu::with_items(
        app,
        text(lang, Text::Edit),
        true,
        &[
            &undo,
            &redo,
            &sep_edit,
            &cut,
            &copy,
            &paste,
            &select_all,
        ],
    )?;

    // 窗口：最小化 / 缩放 / 关闭
    let minimize = PredefinedMenuItem::minimize(app, Some(text(lang, Text::Minimize)))?;
    let zoom = PredefinedMenuItem::maximize(app, Some(text(lang, Text::Zoom)))?;
    let close = PredefinedMenuItem::close_window(app, Some(text(lang, Text::Close)))?;
    let window_menu = Submenu::with_items(
        app,
        text(lang, Text::Window),
        true,
        &[&minimize, &zoom, &close],
    )?;

    let actions = actions_of(kind);
    if actions.is_empty() {
        return Menu::with_items(app, &[&app_menu, &edit_menu, &window_menu]);
    }

    let mut items = Vec::with_capacity(actions.len());
    for action in actions {
        items.push(MenuItem::with_id(
            app,
            format!("{ID_PREFIX}{}", action.id),
            action.text(lang),
            true,
            None::<&str>,
        )?);
    }
    let refs: Vec<&dyn IsMenuItem<Wry>> = items
        .iter()
        .map(|item| item as &dyn IsMenuItem<Wry>)
        .collect();
    let actions_menu = Submenu::with_items(app, text(lang, Text::Actions), true, &refs)?;
    Menu::with_items(
        app,
        &[&app_menu, &edit_menu, &actions_menu, &window_menu],
    )
}

/// 按窗口标签取窗口类型
///
/// 窗口族的标签是 `mml-<kind>-<实例uuid>`，静态窗口是 `mml-<kind>`。
///
/// - `label`: 窗口标签
///
/// # 返回值
///
/// 窗口类型，如 `main` / `resource` / `log`
fn kind_of(label: &str) -> &str {
    if let Some(hit) = family_of_label(label) {
        return hit.family.kind;
    }
    label.strip_prefix("mml-").unwrap_or(label)
}

/// 焦点窗口变了：按它的类型换一份菜单
///
/// 只在 macOS 上真的设置菜单；Windows / Linux 没有应用菜单，直接返回。
///
/// - `app`: 应用句柄
/// - `label`: 获得焦点的窗口标签
pub(crate) fn sync(app: &AppHandle, label: &str) {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, label);
    }

    #[cfg(target_os = "macos")]
    {
        let mut current = CURRENT.lock().unwrap_or_else(|err| err.into_inner());
        if current.as_deref() == Some(label) {
            return;
        }
        let failed = |error: tauri::Error| {
            mml_log::error(LogText::MenuBuildFailed { error: error.to_string() }.text());
        };
        match build(app, kind_of(label)) {
            Ok(menu) => match app.set_menu(menu) {
                Ok(_) => *current = Some(label.to_string()),
                Err(err) => failed(err),
            },
            Err(err) => failed(err),
        }
    }
}

/// 菜单项被点中：把动作交给当前窗口
///
/// - `app`: 应用句柄
/// - `id`: 菜单项 id
pub(crate) fn on_event(app: &AppHandle, id: &str) {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, id);
    }

    #[cfg(target_os = "macos")]
    {
        let Some(action) = id.strip_prefix(ID_PREFIX) else {
            return;
        };
        let Some(window) = app
            .webview_windows()
            .into_values()
            .find(|window| window.is_focused().unwrap_or(false))
        else {
            return;
        };
        crate::windows::emit_menu_action(app, window.label(), action);
    }
}
