//! 应用菜单（macOS）
//!
//! macOS 的菜单栏是**应用级**的：`WebviewWindow::set_menu` 在 macOS 上不生效，全应用只有一份。
//! 所以这里关掉 Tauri 的默认菜单（见 lib.rs 的 `enable_macos_default_menu`），改成
//! **焦点窗口变化时按窗口类型重建一份**。主窗口按功能分组（实例 / 账户 / 收藏 / 设置 / 统计），
//! 其余窗口只有应用菜单。Windows / Linux 不设原生菜单，窗口操作都在各自标题栏上，
//! [`sync`] 与 [`on_event`] 直接返回。
//!
//! 点击菜单项时把动作 id 发给当前窗口，见 `windows::emit_menu_action`；前端按 id 分发。

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

/// 一个菜单动作
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

/// 实例 → 添加实例
const ADD: Action = Action {
    id: "add",
    zh: "添加实例",
    en: "New Instance",
};
/// 实例 → 下载整合包
const ADD_MODPACK: Action = Action {
    id: "add_modpack",
    zh: "下载整合包",
    en: "Download Modpack",
};
/// 实例 → 实例管理（回到主窗口的实例列表）
const MANAGE: Action = Action {
    id: "manage",
    zh: "实例管理",
    en: "Manage Instances",
};
/// 账户 → 添加账户
const ADD_ACCOUNT: Action = Action {
    id: "add_account",
    zh: "添加账户",
    en: "Add Account",
};
/// 账户 → 重新登录当前账户
const RELOGIN: Action = Action {
    id: "relogin",
    zh: "重新登录",
    en: "Re-login",
};
/// 收藏 → 打开收藏窗口
const OPEN_COLLECT: Action = Action {
    id: "collect",
    zh: "打开收藏",
    en: "Open Favorites",
};
/// 设置 → 打开设置窗口
const OPEN_SETTINGS: Action = Action {
    id: "settings",
    zh: "打开设置",
    en: "Open Settings",
};
/// 统计 → 打开统计窗口
const OPEN_STATS: Action = Action {
    id: "stats",
    zh: "打开统计",
    en: "Open Statistics",
};
/// 应用菜单 → 关于（打开关于窗口）
const ABOUT: Action = Action {
    id: "about",
    zh: "关于 M²L",
    en: "About M²L",
};

/// 菜单栏上的固定文案
enum Text {
    /// 应用菜单里的「关于」
    About,
    /// 应用菜单里的「退出」
    Quit,
    /// 实例菜单标题
    Instance,
    /// 账户菜单标题
    Account,
    /// 收藏菜单标题
    Collect,
    /// 设置菜单标题
    Settings,
    /// 统计菜单标题
    Stats,
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
        (Lang::zh_cn, Text::Quit) => "退出 M²L",
        (Lang::zh_cn, Text::Instance) => "实例",
        (Lang::zh_cn, Text::Account) => "账户",
        (Lang::zh_cn, Text::Collect) => "收藏",
        (Lang::zh_cn, Text::Settings) => "设置",
        (Lang::zh_cn, Text::Stats) => "统计",
        (Lang::en_us, Text::About) => "About M²L",
        (Lang::en_us, Text::Quit) => "Quit M²L",
        (Lang::en_us, Text::Instance) => "Instance",
        (Lang::en_us, Text::Account) => "Account",
        (Lang::en_us, Text::Collect) => "Favorites",
        (Lang::en_us, Text::Settings) => "Settings",
        (Lang::en_us, Text::Stats) => "Statistics",
    }
}

/// 建一个「标题 + 一串动作」的子菜单
///
/// - `app`: 应用句柄，菜单项挂在它上面
/// - `lang`: 界面语言，决定动作文案
/// - `title`: 子菜单标题，已按语言取好
/// - `actions`: 这个子菜单里的动作
///
/// # 返回值
///
/// 建好的子菜单；失败返回 tauri 的错误
fn action_menu(
    app: &AppHandle,
    lang: Lang,
    title: &str,
    actions: &[&Action],
) -> tauri::Result<Submenu<Wry>> {
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
    Submenu::with_items(app, title, true, &refs)
}

/// 按窗口类型建一份菜单
///
/// 主窗口是唯一带功能分组的窗口；其余窗口只有应用菜单（关于 / 退出）。
///
/// - `app`: 应用句柄，菜单挂在它上面
/// - `kind`: 窗口类型
///
/// # 返回值
///
/// 建好的菜单；失败返回 tauri 的错误
fn build(app: &AppHandle, kind: &str) -> tauri::Result<Menu<Wry>> {
    let lang = gui_config::locale();

    // 应用菜单：关于打开的是应用自己的关于窗口，不是系统默认面板
    let about = MenuItem::with_id(
        app,
        format!("{ID_PREFIX}{}", ABOUT.id),
        text(lang, Text::About),
        true,
        None::<&str>,
    )?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = PredefinedMenuItem::quit(app, Some(text(lang, Text::Quit)))?;
    let app_menu = Submenu::with_items(app, APP_NAME, true, &[&about, &sep, &quit])?;

    if kind != "main" {
        return Menu::with_items(app, &[&app_menu]);
    }

    let instance_menu = action_menu(
        app,
        lang,
        text(lang, Text::Instance),
        &[&ADD, &ADD_MODPACK, &MANAGE],
    )?;
    let account_menu = action_menu(
        app,
        lang,
        text(lang, Text::Account),
        &[&ADD_ACCOUNT, &RELOGIN],
    )?;
    let collect_menu = action_menu(app, lang, text(lang, Text::Collect), &[&OPEN_COLLECT])?;
    let settings_menu = action_menu(app, lang, text(lang, Text::Settings), &[&OPEN_SETTINGS])?;
    let stats_menu = action_menu(app, lang, text(lang, Text::Stats), &[&OPEN_STATS])?;

    Menu::with_items(
        app,
        &[
            &app_menu,
            &instance_menu,
            &account_menu,
            &collect_menu,
            &settings_menu,
            &stats_menu,
        ],
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
        // 菜单项都是主窗口的（应用菜单的「关于」也从主窗口分发），统一送给主窗口；
        // 主窗口已经关掉时退回当前焦点窗口，交给它自己忽略
        let label = if let Some(main) = app.get_webview_window(crate::windows::main_window_label()) {
            main.label().to_string()
        } else if let Some(window) = app
            .webview_windows()
            .into_values()
            .find(|window| window.is_focused().unwrap_or(false))
        {
            window.label().to_string()
        } else {
            return;
        };
        crate::windows::emit_menu_action(app, &label, action);
    }
}
