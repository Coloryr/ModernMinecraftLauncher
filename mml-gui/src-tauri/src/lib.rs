//! M²L 启动器 Tauri 壳
//!
//! | 模块 | 说明 |
//! | --- | --- |
//! | [`build_info`] | 版本号与构建通道，主页面据此提示测试版 / 预发布版 |
//! | [`collect_utils`] | 收藏数据读写 |
//! | [`dtos`] | 跨 IPC DTO，Rust 结构转 camelCase |
//! | [`err_box`] | 致命错误弹窗 |
//! | [`err_keys`] | 命令返回给前端的错误键，与语言表对账 |
//! | [`gui_config`] | GUI 配置读写 |
//! | [`gui_setting`] | 实例 GUI 设置读写，文件 gui_setting.json，归属 GUI 而非内核 |
//! | [`image_manager`] | 图片资源加载，走 `mml-image://` 协议 |
//! | [`log_text`] | 界面侧日志文案，中英两套，不进内核 i18n |
//! | [`start_arg`] | 启动参数（命令行 / `mml:` 协议）解析，等前端初始化后取走执行 |
//! | [`windows`] | 各窗口的规格 / IPC 命令 / 事件，窗口创建 / 聚焦 / 关闭统一处理 |
//!
//! 自定义主页面走 `mml-home://` 协议，其压缩包在运行根目录常驻，
//! 启动时打开成常驻句柄、请求时按条目现读，**全程不落盘解压**，见 [`windows::custom_home`]。
//! 自定义界面的 CSS 与整页包走 `mml-ui://`，整页包同一套现读口径，见 [`windows::custom_ui`]。

use mml_names::i18;
use tauri::Manager;

use crate::windows::{block, download, main};

pub mod block_groups;
pub mod build_info;
pub(crate) mod client_lock;
pub mod collect_utils;
pub mod dtos;
pub mod err_box;
pub mod err_keys;
pub mod gui_config;
pub mod gui_setting;
pub mod image_manager;
pub mod integrations;
pub mod log_text;
pub mod music;
pub mod single_instance;
pub mod start_arg;
pub mod tray;
pub mod windows;

/// 单元测试的共用启动，内核单例每进程只 boot 一次，见 AGENTS.md §6
#[cfg(test)]
mod test_support;

include!(concat!(env!("OUT_DIR"), "/invokes_gen.rs"));

/// 给进程钉一个固定的任务栏身份
///
/// 不设的话 Windows 会按"从哪个快捷方式启动"来猜任务栏图标：桌面快捷方式带的是实例图标
/// （见 `windows::main::instance::shortcut`），从它启动时任务栏就跟着显示实例图标。
/// 钉成与 `tauri.conf.json` 相同的 identifier，任务栏始终认这个启动器。
#[cfg(windows)]
fn set_app_user_model_id() {
    // 这里必须写绝对路径：本 crate 自己也有一个叫 `windows` 的模块
    use ::windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
    use ::windows::core::HSTRING;

    // 失败也只是任务栏图标退回到旧行为，不影响启动，忽略即可
    let _ = unsafe { SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(APP_USER_MODEL_ID)) };
}

#[cfg(not(windows))]
fn set_app_user_model_id() {}

/// 任务栏身份，与 `tauri.conf.json` 的 `identifier` 一致
const APP_USER_MODEL_ID: &str = "com.coloryr.mml";

/// 启动 Tauri 应用
///
/// 挂接图片协议、窗口事件、Java 列表变更与下载器回调，
/// 注册全部 IPC 命令后进入事件循环。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 命令行 / 协议带来的启动请求先记下来，前端初始化完成后取走执行
    start_arg::collect(std::env::args());

    // 进程身份要在建窗口之前钉好，见 [`set_app_user_model_id`]
    set_app_user_model_id();


    let app = tauri::Builder::default()
        // webview 就绪后的统一装配，都在 on_webview_ready 里拿 ICoreWebView2 做一次：
        // ① 文件拖放的"路径桥"，见 windows/file_drop.rs 头部说明
        // ② 发布模式下关掉浏览器快捷键与 DevTools，如 F5 / F12，见 windows/webview_keys.rs
        .plugin(
            tauri::plugin::Builder::<tauri::Wry>::new("mml-webview")
                .on_webview_ready(|webview| {
                    #[cfg(windows)]
                    {
                        let app = webview.app_handle().clone();
                        let label = webview.label().to_string();
                        let _ = webview.with_webview(move |platform| {
                            if let Ok(core) = unsafe { platform.controller().CoreWebView2() } {
                                windows::file_drop::install(&app, label, &core);
                                windows::webview_keys::disable_browser_keys(&core);
                            }
                        });
                    }
                    #[cfg(not(windows))]
                    {
                        let _ = webview;
                    }
                })
                .build(),
        )
        // 必须在任何 webview 创建之前注册：插件要拿到窗口的原生句柄才能替换装饰。
        // 只服务主窗口这一试点，其余窗口不调 activate_decoration，自然无影响
        .plugin(tauri_plugin_decoration::init())
        .register_asynchronous_uri_scheme_protocol("mml-image", move |_app, request, responder| {
            tauri::async_runtime::spawn(async move {
                image_manager::url_image(request, responder).await;
            });
        })
        .register_asynchronous_uri_scheme_protocol("mml-home", move |_app, request, responder| {
            tauri::async_runtime::spawn(async move {
                windows::custom_home::url_custom_home(request, responder).await;
            });
        })
        // 自定义界面：读 CSS 与解压包内条目都是阻塞 IO，丢到阻塞线程，别占住协议回调
        .register_asynchronous_uri_scheme_protocol("mml-ui", move |_app, request, responder| {
            tauri::async_runtime::spawn_blocking(move || {
                windows::custom_ui::url_custom_ui(request, responder);
            });
        })
        // 自定义音乐：读文件是阻塞 IO，丢到阻塞线程，别占住协议回调
        .register_asynchronous_uri_scheme_protocol(
            music::MUSIC_SCHEME,
            move |_app, request, responder| {
                tauri::async_runtime::spawn_blocking(move || music::url_music(request, responder));
            },
        )
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(windows::on_window_event)
        .setup(|app| {
            // 托盘不在这里建：只有实例在运行时才出现，见 tray::sync

            // 单开与唤回：起 IPC 服务并记下端口，之后启动的实例会把请求交给本进程
            single_instance::start(app.handle());

            if let Err(e) = windows::show_main_window(app.handle()) {
                err_box::fatal_error_text(&e);
            }

            // Java 列表变更：mml_jvms 添加 / 删除 / 配置加载完成 → 通知前端刷新
            let handle = app.handle().clone();
            mml_jvms::add_jvm_change(move || {
                main::emit_java_change(&handle);
            });

            // 游戏运行日志：mml-core `add_run_log` 推送 → 转发为前端 `game-log` 事件
            let handle = app.handle().clone();
            mml_game::add_run_log(move |log| {
                // 注意用全路径：闭包参数也叫 log，会遮住模块名
                windows::log::forward_run_log(&handle, log);
            });

            // 日志攒批：内核逐行推来，这里按固定周期成批发给前端。
            // 单起一条线程，只在到点时看一眼缓冲，空的时候几乎不花什么
            let handle = app.handle().clone();
            std::thread::Builder::new()
                .name("Mml Log Flush".to_string())
                .spawn(move || {
                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(
                            windows::log::LOG_FLUSH_INTERVAL_MS,
                        ));
                        windows::log::flush_pending_logs(&handle);
                    }
                })
                .ok();

            // 游戏退出：内核退出轮询线程发的事件 → 转发为前端 `game-exit` 事件
            let handle = app.handle().clone();
            mml_game::add_exit(move |event| {
                windows::main::launch::forward_game_exit(&handle, event);
            });

            // 下载器：挂接 UI 回调并转发为前端事件，随后启动下载线程池
            let handle = app.handle().clone();
            mml_downloader::set_gui_handel(Box::new(download::DownloadGuiHook::new(handle)));
            mml_downloader::start();

            // 方块渲染：内核 `load()` 里的启动自动补渲染经此回调上报界面
            block::set_gui_handel(app.handle());

            // 收藏数据与核心无关，但必须等 `mml_core::init` 里的日志系统起来后再读，
            // 否则解析失败会没有输出、看起来像没加载
            // 方块窗口的用户数据，同样是启动时读一次
            if let Err(err) = block_groups::load() {
                mml_log::error_type(err);
            }

            if let Err(err) = collect_utils::load() {
                mml_log::error_type(err);
            }
            if let Err(err) = integrations::load() {
                mml_log::error_type(err);
            }

            // 自定义主页面：把压缩包打开成常驻句柄，不落盘解压，请求时按条目现读
            windows::custom_home::reload();

            // 自定义界面的整页包同理；CSS 是普通文件，协议按请求现读
            windows::custom_ui::reload();

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                match mml_core::load() {
                    Ok(()) => {
                        // 统计要在实例表加载完之后读：read() 会拿当前实例刷新名字快照
                        mml_game::game_count::init(mml_base::get_base_dir());
                        main::emit_load_done(&handle, None);
                    }
                    Err(err) => {
                        mml_log::error_type(err.clone());
                        main::emit_load_done(&handle, Some(i18::get_error(err)));
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri_commands!())
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app, event| {
        // 还有实例在跑时，关掉所有窗口只是退回后台：托盘可见，后台还要继续给游戏计时。
        // 没有实例在跑就照常退出 —— 那时既没有托盘，也没有要留在后台干的事。
        // code 为 Some 的是 app.exit() 发起的真退出，不拦。
        if let tauri::RunEvent::ExitRequested { api, code, .. } = event
            && code.is_none()
        {
            if tray::has_running_instance() {
                api.prevent_exit();
            } else if mml_core::get_state() {
                // 兜底：关窗时实例恰好在退出，那条路径可能没来得及停核心
                mml_core::stop();
                mml_game::game_count::stop();
            }
        }
    });
}
