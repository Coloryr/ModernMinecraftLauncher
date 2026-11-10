//! 窗口模型与进程生命周期命令
//!
//! 单窗口模式下前端页面挂载 / 切走时 ensure / drop 窗口模型；重启与关掉其它窗口
//! 也在这里。命令都标了窗口分组，前端仍是 `commands.windows.*`。

use mml_names::names;
use tauri::AppHandle;

use crate::log_text::LogText;

use crate::windows::lifecycle::{
    close_other_windows, drop_model_for_kind, ensure_model_for_kind,
};
use crate::windows::settings;
use super::gui_config::emit_window_mode_change;
use super::shared::main_window_label;

/// 确保某个窗口 kind 的模型存在，单窗口模式下前端页面挂载时调用
///
/// 多窗口模式下模型本来就跟着真实窗口创建，这里重复调用无副作用。
///
/// - `kind`: 窗口类型
///
/// # 返回值
///
/// 模型已就绪返回 `Ok(())`；kind 未知返回错误文案
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub fn window_ensure_model(kind: String) -> Result<(), String> {
    ensure_model_for_kind(&kind)
}

/// 释放某个窗口 kind 的模型，单窗口模式下前端页面切走 / 关闭时调用
///
/// 单窗口模式没有"窗口关闭"这个事件可依赖 —— 页面切走时组件进 KeepAlive，
/// 后端收不到任何通知。不显式释放的话模型会一直挂着，常驻内存，
/// 而页面上的取消令牌 / 关闭保护 / 重名应答通道都是**就地失效**的：
/// 下次回到该页会重新 ensure 一份干净的。
///
/// - `kind`: 窗口类型
///
/// # 返回值
///
/// 模型已释放或本来就没有时返回 `Ok(())`；kind 未知返回错误文案
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub fn window_drop_model(kind: String) -> Result<(), String> {
    drop_model_for_kind(&kind)
}

/// 重启启动器：切换窗口模式这类"必须重启才干净生效"的设置改完后，由前端调用
///
/// 顺序有讲究：
/// 1. **先落盘**：`mml_core::stop()` 触发停止事件，其中 `config_save::stop` 是同步 join，
///    返回即代表队列写完。若放到拉起新进程之后，新进程可能读到旧配置、仍旧按旧模式打开；
/// 2. **再拉起一份自己**：`current_exe()` 就是当前可执行文件；
/// 3. **最后退出本进程**：放在后台线程里延迟一下再退——开发模式下 `tauri dev` 的 vite 会随本进程
///    退出而关闭，新进程得趁它活着把页面拉起来；release 没有这个依赖，稍等让新窗口先出现即可。
///
/// 本命令一定结束当前进程，是否提示由调用方决定。
///
/// - `app`: 应用句柄，退出当前进程用它
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub fn window_restart_app(app: AppHandle) {
    // 存量映射正在搬文件：重启会直接把进程杀掉
    if settings::resource_link::running() {
        return;
    }
    mml_core::stop();
    mml_game::game_count::stop();

    // 先让出端口文件：新进程带 --restart 直接锁它，晚一步就会锁不上
    crate::single_instance::release_port_file();

    std::thread::spawn(move || {
        match std::env::current_exe() {
            Ok(exe) => {
                // 带上 --restart：新进程跳过单开检查，否则会把请求交回本进程（马上要退的那个）
                if let Err(err) = std::process::Command::new(exe)
                    .arg(names::COMMAND_RESTART)
                    .arg("--run-dir")
                    .arg(crate::gui_config::dir().unwrap())
                    .spawn()
                {
                    mml_log::error(
                        LogText::RestartLauncherFailed {
                            error: err.to_string(),
                        }
                        .text(),
                    );
                }
            }
            Err(err) => {
                mml_log::error(
                    LogText::RestartLauncherError {
                        error: err.to_string(),
                    }
                    .text(),
                );
            }
        }

        // debug 即 tauri dev 时，等新进程把页面从 vite 拉起来；release 下没有 dev server 这回事
        let wait = if cfg!(debug_assertions) { 2000 } else { 150 };
        std::thread::sleep(std::time::Duration::from_millis(wait));
        app.exit(0);
    });
}

/// 切换窗口模式后收掉旧模式下的窗口：关掉主窗口以外的全部
///
/// 窗口模式只决定"新窗口怎么开"（多窗口开真实窗口，单窗口走应用内页面），所以
/// 单窗口 → 多窗口没有别的窗口要关，多窗口 → 单窗口必须关掉它们，否则它们停在旧模式里。
///
/// **不重载界面、也不重启进程**：重载会让窗口在几秒里先落回原生标题栏再被装饰插件摘掉，
/// 游戏在跑时重启更会把它一起带走。
///
/// - `app`: 应用句柄，关窗用它
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub fn window_close_other_windows(app: AppHandle) {
    close_other_windows(&app, main_window_label());
    // 模式存在每个 webview 自己那份内存里：剩下的窗口得知道它变了
    let multi = matches!(
        crate::gui_config::get().window_mode,
        crate::gui_config::WindowMode::Multi
    );
    emit_window_mode_change(&app, multi);
}
