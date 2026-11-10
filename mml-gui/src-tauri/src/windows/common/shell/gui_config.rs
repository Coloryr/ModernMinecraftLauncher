//! GUI 配置命令与跨窗口配置事件
//!
//! 读写 `gui_config.json`、客户端设置锁，以及皮肤 / 主题 / 视图模式 / 窗口模式的变更广播。
//! 命令带 `ipc_group("windows")`，前端仍是 `commands.windows.*`。

use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
use uuid::Uuid;

use crate::{dtos::GuiConfigDto, listens};

/// 窗口模式变了：各窗口照着改，免得还按旧模式开窗
///
/// - `app`: 应用句柄，广播用它
/// - `multi`: 切换后的模式，true 为多窗口
#[gui_macros::emit]
pub fn emit_window_mode_change(app: &AppHandle, multi: bool) {
    let _ = app.emit(listens::WINDOW_MODE_CHANGE, multi);
}
/// 获取 GUI 状态；无文件时返回默认值。前端 wire 为 DTO，TS 命名 camelCase
///
/// 首次启动、无配置文件时默认主题跟随系统深浅色——WebView2 的窗口主题
/// 即系统的应用模式。此时不落盘，用户第一次改设置才会把当时的主题存进文件
///
/// - `window`: 调用方窗口，读它的主题判断系统深浅色
///
/// # 返回值
///
/// 当前 GUI 状态；首次启动时 `theme` 已按系统深浅色填好
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub fn window_get_gui_config(window: WebviewWindow) -> GuiConfigDto {
    let fresh = crate::gui_config::is_fresh();
    let mut config = crate::gui_config::get();
    if fresh {
        config.theme = match window.theme() {
            Ok(tauri::Theme::Light) => crate::gui_config::Theme::Light,
            _ => crate::gui_config::Theme::Dark,
        };
    }
    let mut dto: GuiConfigDto = config.into();
    // 首启向导（语言 → 旧数据 → 主页面设置）只在没有配置文件时走一遍
    dto.fresh = fresh;
    dto
}

/// 设置密码并锁定客户端设置
///
/// 盐与哈希写运行目录的隐藏文件，见 client_lock 模块。
///
/// - `app`: 应用句柄，变更后广播 `client-config-change`
/// - `password`: 新密码，不能为空
///
/// # 返回值
///
/// 成功返回 `Ok(())`；密码为空或两份锁文件都写不进去时返回错误
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub fn window_client_lock_set(app: AppHandle, password: String) -> Result<(), String> {
    crate::client_lock::set(&password)?;
    emit_client_config_change(&app, crate::gui_config::get().client.into());
    Ok(())
}

/// 解除客户端设置锁定，删除隐藏的锁文件
///
/// - `app`: 应用句柄，变更后广播 `client-config-change`
///
/// # 返回值
///
/// 成功返回 `Ok(())`；删不掉锁文件时返回错误
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub fn window_client_lock_clear(app: AppHandle) -> Result<(), String> {
    crate::client_lock::clear()?;
    emit_client_config_change(&app, crate::gui_config::get().client.into());
    Ok(())
}

/// 校验解锁密码
///
/// - `password`: 用户输入的密码
///
/// # 返回值
///
/// 密码正确或根本没有锁定时返回 `true`
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub fn window_client_lock_verify(password: String) -> bool {
    crate::client_lock::verify(&password)
}

/// 保存 GUI 状态到 gui_config.json，前端 DTO 转内部 GuiConfig
///
/// 皮肤显示模式 / 头像配置有变化时广播 `skin-config-change`：渲染缓存键里含这两项，
/// 各窗口收到后按新模式重取头像 / 皮肤图；实例列表显示模式有变化时广播
/// `gui-view-mode-change`，主窗口据此立即换视图；背景图的不透明度 / 模糊有变化时
/// 广播 `bg-change`，各窗口重取背景状态；主题 / 强调色有变化时广播
/// `gui-theme-change`，各窗口重新应用主题；窗口材质变化时直接在这里重铺所有窗口，
/// 透明度是前端 CSS 变量，随 `client-config-change` 由各窗口自己跟上
///
/// - `app`: 应用句柄，广播变更事件用它
/// - `config`: 前端提交的完整 GUI 状态，内部转成落盘用的 `GuiConfig`
///
/// # 返回值
///
/// 保存成功返回 `Ok(())`；失败返回错误文案
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub async fn window_save_gui_config(app: AppHandle, config: GuiConfigDto) -> Result<(), String> {
    let mut new_config: crate::gui_config::GuiConfig = config.into();
    // 窗口浏览偏好不属于设置页：从旧配置整份带过来，免得存一次设置就把它们清掉
    new_config.resource_window = crate::gui_config::get().resource_window;
    new_config.config_window = crate::gui_config::get().config_window;
    let skin_changed = {
        let old = crate::gui_config::get();
        old.skin != new_config.skin || old.head != new_config.head || old.cape != new_config.cape
    };
    let client_changed = {
        let old = crate::gui_config::get();
        old.client != new_config.client
    };
    let view_mode_changed = {
        let old = crate::gui_config::get();
        old.main_window.view_mode != new_config.main_window.view_mode
    };
    // 背景图的不透明度 / 模糊没有专门的命令，只经这里落盘：变了就广播背景变更，
    // 否则别的窗口要等下次启动才跟着变
    let bg_view_changed = {
        let old = crate::gui_config::get();
        old.bg_opacity != new_config.bg_opacity || old.bg_blur != new_config.bg_blur
    };
    // 主题 / 强调色同理，而且它连 localStorage 都靠不住：多窗口模式下各窗口是不同 webview，
    // storage 事件在它们之间不发，只有设置窗口自己会变
    let theme_changed = {
        let old = crate::gui_config::get();
        old.theme != new_config.theme
            || old.accent != new_config.accent
            || old.custom_accent != new_config.custom_accent
    };
    let new_theme = new_config.theme;
    let new_accent = new_config.accent.clone();
    let new_custom_accent = new_config.custom_accent.clone();
    let new_client = new_config.client.clone();
    if client_changed {
        let uuid = new_client.lock_instance.clone();
        if !uuid.is_empty()
            && let Ok(uuid) = Uuid::parse_str(&uuid)
            && mml_game::have_instance_uuid(&uuid)
        {
            new_config.main_window.selected_instance = new_client.lock_instance.clone();
        }
    }
    let new_view_mode = new_config.main_window.view_mode;
    let locale_changed = crate::gui_config::get().locale != new_config.locale;
    let effect_before = crate::gui_config::get().client.window_effect;
    crate::gui_config::set(new_config);
    // 材质是建窗那一刻铺上去的：设置里换了就地重铺一遍，否则得等下次开窗才看得到
    let effect_now = crate::gui_config::get().client.window_effect;
    if effect_before != effect_now {
        for (_, win) in app.webview_windows() {
            crate::windows::effects::apply(&win, &effect_now);
        }
    }
    if skin_changed {
        emit_skin_config_change(&app);
    }
    if client_changed {
        emit_client_config_change(&app, new_client.into());
    }
    if view_mode_changed {
        emit_gui_view_mode_change(&app, new_view_mode);
    }
    if bg_view_changed {
        crate::windows::settings::bg::emit_bg_change(&app);
    }
    if theme_changed {
        emit_gui_theme_change(&app, new_theme, new_accent, new_custom_accent);
    }
    if locale_changed {
        // 托盘是原生菜单，前端切语言带不动它，只能在这里重建一遍
        crate::tray::refresh_text(&app);
    }
    Ok(())
}

/// 皮肤 / 头像显示配置变更事件，跨窗口同步重取渲染图
///
/// - `app`: 应用句柄，广播用它
#[gui_macros::emit]
fn emit_skin_config_change(app: &AppHandle) {
    let _ = app.emit(listens::SKIN_CONFIG_CHANGE, ());
}

/// 客户端设置变更事件，跨窗口同步，如主窗口 MOTD 卡片 / 登录锁定 / 窗口透明度
///
/// - `app`: 应用句柄，广播用它
/// - `config`: 变更后的客户端设置
#[gui_macros::emit]
fn emit_client_config_change(app: &AppHandle, config: crate::dtos::ClientConfigDto) {
    let _ = app.emit(listens::CLIENT_CONFIG_CHANGE, config);
}

/// 实例列表显示模式变更事件，跨窗口同步
///
/// 切换入口在设置窗口，主窗口只是读的一方：多窗口模式下两者是不同的 webview，
/// 不广播的话只能等下次启动才生效，见 `window_save_gui_config`。
///
/// - `app`: 应用句柄，广播用它
/// - `mode`: 变更后的显示模式
#[gui_macros::emit]
fn emit_gui_view_mode_change(app: &AppHandle, mode: crate::gui_config::ViewMode) {
    let _ = app.emit(listens::GUI_VIEW_MODE_CHANGE, mode);
}

/// 主题 / 强调色变更事件，跨窗口同步
///
/// 切换入口在设置窗口：多窗口模式下其它窗口是不同的 webview，要靠这条事件重新应用主题
/// （localStorage 的 storage 事件在 WebView2 的多 webview 之间不发，只能走壳层广播）。
///
/// - `app`: 应用句柄，广播用它
/// - `theme`: 变更后的主题
/// - `accent`: 变更后的强调色 id，或 `custom`
/// - `custom_accent`: 自定义强调色，`#rrggbb`
#[gui_macros::emit]
fn emit_gui_theme_change(
    app: &AppHandle,
    theme: crate::gui_config::Theme,
    accent: String,
    custom_accent: String,
) {
    let _ = app.emit(
        listens::GUI_THEME_CHANGE,
        crate::dtos::GuiThemeDto {
            theme,
            accent,
            custom_accent,
        },
    );
}