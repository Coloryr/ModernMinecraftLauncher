//! Windows 文件拖放的"路径桥"：让 **DOM 拖放**与**真实文件路径**两者兼得
//!
//! # 为什么要这么绕
//!
//! 框架 wry 的 Windows 拖放实现会顶掉 WebView2 自己的拖放目标，而且只认 `CF_HDROP`：
//! 网页上拖下来的卡片是文本 / 链接，会被直接拒绝，webview 连 `dragenter` 都收不到，
//! 拖动时鼠标显示"禁止"。上游把这条限制定性为 "internal, alternative, file-focused,
//! **exclusive**"，见 tauri#14373；维护者也说 Windows 上"没法直接修"。
//!
//! 但这不是 Windows 的限制：只要不装 wry 那一份，WebView2 自己的拖放目标就还在，
//! **DOM 拖放全功能**，文本、`text/uri-list`、`text/html` 都能读。剩下的唯一问题是
//! DOM 拿不到文件的**真实路径** —— 浏览器的安全设计，这一处用 WebView2 官方的
//! "附加对象"通道补上：
//!
//! ```text
//! 页面 drop ──► chrome.webview.postMessageWithAdditionalObjects(msg, files)
//!                     │  File 对象原样过桥，不是它的名字
//!                     ▼
//! 原生 WebMessageReceived ──► AdditionalObjects() ──► ICoreWebView2File::Path()
//!                     ▼
//!              emit_file_drop → 前端拿到真实绝对路径
//! ```
//!
//! 两侧都是微软文档里的正式 API：`ICoreWebView2WebMessageReceivedEventArgs2::get_AdditionalObjects`
//! 自 WebView2 Win32 1.0.1774.30 起提供。
//!
//! # 与框架的关系
//!
//! 建窗时用 `disable_drag_drop_handler()` 关掉 wry 那一份，见 `windows/lifecycle.rs`；
//! 于是 `tauri://drag-drop` **不再触发**，前端只走本模块这条路径事件 + DOM 事件。
//! 两套不能并存，否则遮罩会闪两次。

use crate::log_text::LogText;
use tauri::AppHandle;
use webview2_com::{
    Microsoft::Web::WebView2::Win32::{
        ICoreWebView2, ICoreWebView2File, ICoreWebView2WebMessageReceivedEventArgs,
        ICoreWebView2WebMessageReceivedEventArgs2,
    },
    WebMessageReceivedEventHandler,
};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::core::{Interface, PWSTR};

use crate::dtos::FileDropDto;
use crate::windows::emit_file_drop;

/// 注入脚本与原生约定的消息标记
///
/// 原生按它认出"这是路径桥的消息"：Tauri 自己的 IPC 也走 `postMessage`，
/// 不认标记就会把它当 IPC 去解析。
const MESSAGE: &str = "__MML_WINDOWS_FILE_DROP__";

/// 拖放脚本：DOM 的 `drop` 里把 `File` 对象交给原生
///
/// 只在**有文件**时转发，文本 / 链接由前端自己处理，不必过桥；
/// 用捕获阶段挂 `document`，先于页面自己的处理器拿到事件。
///
/// 由建窗时的 `initialization_script` 注册，见 `windows/lifecycle.rs`：它在导航开始前
/// 就挂好了，每个新建文档都会执行。**不能**改回在 webview ready 回调里调
/// `AddScriptToExecuteOnDocumentCreated` —— 那个 API 只对之后新建的文档生效，
/// 初始文档已经建好，脚本永远不执行（文件拖入会一直拿不到路径）。
pub(crate) const INJECT_SCRIPT: &str = r#"
(() => {
  const MESSAGE = "__MML_WINDOWS_FILE_DROP__";
  const handler = (event) => {
    const webview = window.chrome && window.chrome.webview;
    const files = event.dataTransfer && event.dataTransfer.files;
    if (!webview || !webview.postMessageWithAdditionalObjects || !files || !files.length) {
      return;
    }
    // 显式转成数组再交出去：文档里 additionalObjects 就是数组，FileList 能不能被原样接住
    // 没有保证 —— 而"一次拖入多个文件"必须全部过桥，不能只过去第一个
    webview.postMessageWithAdditionalObjects(MESSAGE, Array.from(files));
  };
  document.removeEventListener("drop", handler, true);
  document.addEventListener("drop", handler, true);
})();
"#;

/// 给这个 webview 装上路径桥：脚本加消息处理器
///
/// - `app`: 应用句柄，事件经它转发
/// - `label`: 窗口标签，事件只发给这个窗口
/// - `webview2`: WebView2 的 `ICoreWebView2`，由 `on_webview_ready` 里取
pub fn install(app: &AppHandle, label: String, webview2: &ICoreWebView2) {

    let mut token = 0;
    let handle = app.clone();
    let target = label;
    unsafe {
        if let Err(err) = webview2.add_WebMessageReceived(
            &WebMessageReceivedEventHandler::create(Box::new(move |_webview, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                // 只认路径桥那条消息：其它消息一概不碰，Tauri 自己的 IPC 也在内
                let mut raw = PWSTR::null();
                args.TryGetWebMessageAsString(&mut raw)?;
                let message = take_pwstr(raw);
                if message != MESSAGE {
                    return Ok(());
                }

                let paths = file_paths(&args);
                if paths.is_empty() {
                    // 过桥了却取不到路径：用户侧会看到"没拿到拖入文件的路径"的提示
                    mml_log::error(LogText::FilePathBridgeNoPath { label: target.clone() }.text());
                    return Ok(());
                }
                emit_file_drop(&handle, &target, FileDropDto { paths });
                Ok(())
            })),
            &mut token,
        ) {
            mml_log::error(
                LogText::FilePathBridgeFailed {
                    error: format!("{err:?}"),
                }
                .text(),
            );
        }
    }
}

/// 从"附加对象"里取出被拖入文件的**真实绝对路径**
fn file_paths(args: &ICoreWebView2WebMessageReceivedEventArgs) -> Vec<String> {
    let Ok(args2) = args.cast::<ICoreWebView2WebMessageReceivedEventArgs2>() else {
        return Vec::new();
    };
    let Ok(objects) = (unsafe { args2.AdditionalObjects() }) else {
        return Vec::new();
    };

    let mut count = 0;
    if unsafe { objects.Count(&mut count) }.is_err() {
        return Vec::new();
    }

    let mut paths = Vec::new();
    for index in 0..count {
        let Ok(value) = (unsafe { objects.GetValueAtIndex(index) }) else {
            continue;
        };
        let Ok(file) = value.cast::<ICoreWebView2File>() else {
            continue;
        };
        let mut raw = PWSTR::null();
        if unsafe { file.Path(&mut raw) }.is_err() {
            continue;
        }
        let path = take_pwstr(raw);
        if !path.is_empty() {
            paths.push(path);
        }
    }
    paths
}

/// 取走一段 COM 分配的 PWSTR，读完立刻释放，别漏
fn take_pwstr(raw: PWSTR) -> String {
    if raw.is_null() {
        return String::new();
    }
    let text = unsafe { raw.to_string() }.unwrap_or_default();
    unsafe { CoTaskMemFree(Some(raw.0 as *const core::ffi::c_void)) };
    text
}
