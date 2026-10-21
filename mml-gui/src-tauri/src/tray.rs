//! 系统托盘：主窗口关掉后进程留在后台，托盘是唤回界面的入口之一（另一个是再启动一次 exe，
//! 见 [`crate::single_instance`]），也是唯一的退出入口。
//!
//! 为什么要有它：关窗只是"收起界面"，后台还要继续给运行中的游戏计时
//! （`mml_game::game_count` 的计时线程与界面无关）。没有托盘的话进程就成了
//! 没有任何退出入口的幽灵进程，所以**托盘没建起来时关窗仍然按老样子退出应用**，
//! 见 `windows::lifecycle::events` 的关闭处理与 `lib.rs` 的 ExitRequested。

use std::sync::atomic::{AtomicBool, Ordering};

use mml_names::Lang;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::AppHandle;

use crate::gui_config;
use crate::log_text::LogText;

/// 托盘图标 id，刷新文案时按它取回句柄
const TRAY_ID: &str = "mml-tray";

/// 托盘菜单项 id：显示主界面
const MENU_SHOW: &str = "show";
/// 托盘菜单项 id：退出启动器
const MENU_QUIT: &str = "quit";

/// 托盘是否已经建起来
static READY: AtomicBool = AtomicBool::new(false);

/// 托盘是否可用
///
/// # 返回值
///
/// 图标建成功过为 true；为 false 时关主窗口必须退出应用，否则没有唤回入口
pub fn is_ready() -> bool {
    READY.load(Ordering::Relaxed)
}

/// 托盘上要显示的几条文案
enum Text {
    /// 悬停提示
    Tooltip,
    /// 菜单：显示主界面
    Show,
    /// 菜单：退出
    Quit,
}

/// 取当前语言的托盘文案
///
/// 托盘是原生菜单，走不了前端的 i18n 表；条目只有三条，就近放这里，
/// 与 `log_text` 一样中英各一套，切换语言时由 [`refresh_text`] 重建菜单。
///
/// - `lang`: 界面语言
/// - `which`: 要哪一条
///
/// # 返回值
///
/// 该语言的成品文案
fn text(lang: Lang, which: Text) -> &'static str {
    match (lang, which) {
        (Lang::zh_cn, Text::Tooltip) => "M²L 启动器",
        (Lang::zh_cn, Text::Show) => "显示主界面",
        (Lang::zh_cn, Text::Quit) => "退出",
        (Lang::en_us, Text::Tooltip) => "M²L Launcher",
        (Lang::en_us, Text::Show) => "Show window",
        (Lang::en_us, Text::Quit) => "Quit",
    }
}

/// 按当前语言建一份菜单
///
/// - `app`: 应用句柄，菜单挂在它上面
///
/// # 返回值
///
/// 建好的菜单；失败返回 tauri 的错误
fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let lang = gui_config::locale();
    let show = MenuItem::with_id(app, MENU_SHOW, text(lang, Text::Show), true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, text(lang, Text::Quit), true, None::<&str>)?;
    Menu::with_items(app, &[&show, &quit])
}

/// 建托盘图标与右键菜单，`setup` 阶段调用
///
/// 失败只记日志、不拦启动：`is_ready` 保持 false，关窗仍按老样子退出应用。
///
/// - `app`: 应用句柄，托盘挂在它上面
pub fn init(app: &AppHandle) {
    if let Err(err) = create(app) {
        mml_log::error(LogText::TrayInitFailed { error: err }.text());
        return;
    }
    READY.store(true, Ordering::Relaxed);
}

/// 实际建托盘，失败把原因交回调用方记日志
fn create(app: &AppHandle) -> Result<(), String> {
    let lang = gui_config::locale();
    let menu = build_menu(app).map_err(|err| err.to_string())?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip(text(lang, Text::Tooltip))
        .menu(&menu)
        // 左键用来唤出界面，菜单走右键，与 Windows 上的习惯一致
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            MENU_SHOW => crate::windows::summon_main_window(app),
            MENU_QUIT => quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                crate::windows::summon_main_window(tray.app_handle());
            }
        });
    // 图标直接用窗口图标：打包时由 tauri.conf.json 的 bundle.icon 内嵌进来
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder
        .build(app)
        .map(|_| ())
        .map_err(|err| err.to_string())
}

/// 切换语言后重建托盘菜单与提示
///
/// 托盘是原生菜单，前端切语言不会自动跟；设置页保存 GUI 配置时若语言变了就调这里。
///
/// - `app`: 应用句柄，按 id 取回托盘
pub fn refresh_text(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    let lang = gui_config::locale();
    if let Ok(menu) = build_menu(app) {
        let _ = tray.set_menu(Some(menu));
    }
    let _ = tray.set_tooltip(Some(text(lang, Text::Tooltip)));
}

/// 退出启动器：先让内核与统计落盘，再结束进程
///
/// - `app`: 应用句柄
fn quit(app: &AppHandle) {
    mml_core::stop();
    mml_game::game_count::stop();
    app.exit(0);
}
