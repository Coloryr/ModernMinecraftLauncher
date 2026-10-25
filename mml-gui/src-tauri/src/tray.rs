//! 系统托盘：**只在有实例运行时显示** —— 这时关掉主窗口进程仍留在后台
//! （给运行中的游戏计时、随时从托盘唤回界面），托盘是后台唯一的退出入口。
//!
//! 没有实例在跑时既没有托盘，关窗就是退出启动器：后台没有要干的事，也不该留下
//! 一个唤不回来的幽灵进程，见 windows::lifecycle::events 的关闭处理与
//! lib.rs 的 ExitRequested。
//!
//! 托盘跟着运行态走：实例真正启动时 sync 把它建出来，最后一个实例退出时收掉；
//! 若收掉的那一刻主界面也已经关掉，进程没有留下的理由，quit_idle 收工。

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use mml_names::Lang;
use tauri::AppHandle;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

use crate::gui_config;
use crate::log_text::LogText;

/// 托盘图标 id，刷新文案 / 显隐时按它取回句柄
const TRAY_ID: &str = "mml-tray";

/// 托盘菜单项 id：显示主界面
const MENU_SHOW: &str = "show";
/// 托盘菜单项 id：退出启动器
const MENU_QUIT: &str = "quit";

/// 托盘是否正在显示
static SHOWN: AtomicBool = AtomicBool::new(false);

/// 是否已经在退出流程里：最后一个实例退出与主窗口销毁两处都会触发，只执行一次
static QUITTING: AtomicBool = AtomicBool::new(false);

/// 显隐状态锁：实例启动线程与退出轮询线程都会调 sync，建图标不能并发
static SYNC: Mutex<()> = Mutex::new(());

/// 是否还有实例在运行
///
/// 托盘显隐与"关窗后是否留在后台"都以它为准，运行态的唯一来源是 mml_game，
/// 这里不另记一份。
///
/// # 返回值
///
/// 至少有一个游戏实例在运行时为 true
pub(crate) fn has_running_instance() -> bool {
    !mml_game::get_running_instances().is_empty()
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
/// 与 log_text 一样中英各一套，切换语言时由 refresh_text 重建菜单。
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

/// 按运行态同步托盘：有实例在跑就显示，没有就收起来
///
/// 实例真正启动（LaunchState::End）与实例退出时各调一次，见 windows::main::launch。
///
/// - `app`: 应用句柄，建图标 / 取回图标都用它
pub(crate) fn sync(app: &AppHandle) {
    // 加锁时忽略中毒：这里只是串行化建图标，状态都在这几个原子量里
    let _lock = SYNC.lock().unwrap_or_else(|err| err.into_inner());
    if has_running_instance() {
        show(app);
    } else {
        hide(app);
    }
}

/// 显示托盘：还没建过就现在建
///
/// - `app`: 应用句柄
fn show(app: &AppHandle) {
    if SHOWN.load(Ordering::Relaxed) {
        return;
    }
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        match tray.set_visible(true) {
            Ok(()) => SHOWN.store(true, Ordering::Relaxed),
            Err(err) => mml_log::error(
                LogText::TrayVisibleFailed {
                    visible: true,
                    error: err.to_string(),
                }
                .text(),
            ),
        }
        return;
    }
    match create(app) {
        Ok(()) => SHOWN.store(true, Ordering::Relaxed),
        Err(err) => mml_log::error(LogText::TrayInitFailed { error: err }.text()),
    }
}

/// 收起托盘：图标留着，下次有实例启动时直接显示
///
/// - `app`: 应用句柄
fn hide(app: &AppHandle) {
    if !SHOWN.swap(false, Ordering::Relaxed) {
        return;
    }
    if let Some(tray) = app.tray_by_id(TRAY_ID)
        && let Err(err) = tray.set_visible(false)
    {
        mml_log::error(
            LogText::TrayVisibleFailed {
                visible: false,
                error: err.to_string(),
            }
            .text(),
        );
    }
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
/// 没有实例在跑时托盘还不存在，直接返回即可 —— 下次建出来时用的就是新语言。
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

/// 没有实例在跑、主界面也已经关掉：后台没有留下的理由，退出启动器
///
/// 主窗口关掉与最后一个实例退出可能先后颠倒，两处都调这里兜底，
/// 由 quit 保证只退一次。
///
/// - `app`: 应用句柄
pub(crate) fn quit_idle(app: &AppHandle) {
    // 刚有实例启动就别退
    if has_running_instance() || QUITTING.load(Ordering::SeqCst) {
        return;
    }
    mml_log::info(LogText::QuitAfterLastGame.text());
    quit(app);
}

/// 退出启动器：先让内核与统计落盘，再结束进程
///
/// 托盘菜单与 quit_idle 都走这里，重复调用只执行一次。
///
/// - `app`: 应用句柄
fn quit(app: &AppHandle) {
    if QUITTING.swap(true, Ordering::SeqCst) {
        return;
    }
    mml_core::stop();
    mml_game::game_count::stop();
    app.exit(0);
}
