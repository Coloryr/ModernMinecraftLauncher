//! M²L 主题开发工具：Tauri 桌面壳
//!
//! 主窗口是编辑器（Vue + Monaco），预览是独立的子 WebView；
//! 项目、构建与预览进程都由 Rust 侧持有，关窗后一起回收。

mod commands;
mod config;
mod files;
mod messages;
mod preview;
mod process;
mod project;
mod studio;

use crate::{
    messages::{MessageKey, render},
    studio::Studio,
};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent, webview::PageLoadEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

/// 启动桌面工具
pub fn run() {
    tauri::Builder::default()
        // F12 只归预览：关掉 WebView2 自带的浏览器快捷键（含 F12 / Ctrl+Shift+I），
        // DevTools 本身不关，工具栏和预览里的 F12 仍通过 open_devtools 打开。
        .plugin(
            tauri::plugin::Builder::<tauri::Wry>::new("studio-webview")
                .on_webview_ready(|webview| {
                    #[cfg(windows)]
                    {
                        let _ = webview.with_webview(|platform| {
                            if let Ok(core) = unsafe { platform.controller().CoreWebView2() } {
                                disable_browser_accelerator_keys(&core);
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
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_root = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_root)?;
            let config_path = data_root.join("studio.json");
            let config = config::load(&config_path);
            // 源码运行的中间产物集中在仓库 target/temp；分发版放工具自己的数据目录
            let cache = if tauri::is_dev() {
                // 先去掉路径里的 ..，错误信息和日志里显示的是干净的仓库路径
                let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
                let repo = std::fs::canonicalize(&repo)
                    .map(files::clean_path)
                    .unwrap_or(repo);
                repo.join("target/temp/theme-studio")
            } else {
                data_root.join("target/temp")
            };
            let studio = Studio::new(app.handle().clone(), config_path, config.clone(), cache);
            app.manage(studio.clone());

            let window =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                    .title(render(config.locale, MessageKey::Title, &[]))
                    .inner_size(1440.0, 920.0)
                    .min_inner_size(1000.0, 640.0)
                    .center()
                    .theme(Some(config.theme.native()))
                    .background_color(config.theme.color())
                    .visible(false)
                    .on_page_load(|window, payload| {
                        if payload.event() == PageLoadEvent::Finished {
                            let _ = window.show();
                        }
                    })
                    .build()?;
            // 前端加载失败时也要能看到窗口，而不是只剩一个进程
            reveal_later(window.clone());

            let handle = app.handle().clone();
            let events = studio.clone();
            window.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    if events.allow_close() {
                        return;
                    }
                    if !events.dirty() && !events.busy() {
                        return;
                    }
                    api.prevent_close();
                    if events.asking_close() {
                        return;
                    }
                    events.set_asking_close(true);
                    let studio = events.clone();
                    let app = handle.clone();
                    tauri::async_runtime::spawn(async move {
                        let locale = studio.locale().await;
                        let key = if studio.dirty() {
                            MessageKey::ConfirmDiscard
                        } else {
                            MessageKey::ConfirmClose
                        };
                        let buttons = MessageDialogButtons::OkCancelCustom(
                            render(locale, MessageKey::Close, &[]),
                            render(locale, MessageKey::Cancel, &[]),
                        );
                        let confirmed = app
                            .dialog()
                            .message(render(locale, key, &[]))
                            .title(render(locale, MessageKey::Title, &[]))
                            .buttons(buttons)
                            .blocking_show();
                        studio.set_asking_close(false);
                        if confirmed {
                            studio.set_allow_close(true);
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.close();
                            }
                        }
                    });
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::studio_state,
            commands::studio_pick,
            commands::studio_create,
            commands::studio_open,
            commands::studio_files,
            commands::studio_read,
            commands::studio_save,
            commands::studio_catalog,
            commands::studio_install,
            commands::studio_build,
            commands::studio_preview,
            commands::studio_stop,
            commands::studio_cancel,
            commands::studio_devtools,
            commands::studio_locale,
            commands::studio_theme,
            commands::studio_dirty,
            commands::studio_reveal,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build MML Theme Studio")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event
                && let Some(studio) = app.try_state::<Arc<Studio>>()
            {
                studio.shutdown_blocking();
            }
        });
}

/// 关闭 WebView2 的浏览器专用快捷键，保留 DevTools API
///
/// 编辑器里的 F12 要留给 Monaco 的"转到定义"；预览页自己监听 F12 后
/// 通过 postMessage 通知编辑器，再走 `open_devtools`。
#[cfg(windows)]
fn disable_browser_accelerator_keys(
    core: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2,
) {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
    use windows::core::Interface;

    let Ok(settings) = (unsafe { core.Settings() }) else {
        return;
    };
    let Ok(settings3) = settings.cast::<ICoreWebView2Settings3>() else {
        return;
    };
    let _ = unsafe { settings3.SetAreBrowserAcceleratorKeysEnabled(false) };
}

/// 兜底显示主窗口，避免页面加载事件没触发时窗口一直隐藏
fn reveal_later(window: tauri::WebviewWindow) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(5));
        let _ = window.show();
    });
}
