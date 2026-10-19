//! 自定义音乐：把配置里那一份音频伺服给主窗口的 `<audio>`
//!
//! webview 读不到本地文件，走命令返回字节又要 base64（几 MB 的歌不值当），所以照
//! `mml-image` / `mml-home` 的办法再开一个自定义协议：前端把
//! [`music_base_url`] 的地址交给 `<audio>`，文件由本模块现读现吐。
//!
//! **地址里不带路径**：歌就一份、位置来自配置，因此这个协议没有"任意文件读取"的面
//! （对比 `mml-home` 那条路径穿越防护）。来源填的是 http(s) 直链时这里一律 404 ——
//! 那种情况前端直接把链接给 `<audio>`，不经过本协议。

use std::path::Path;

use tauri::UriSchemeResponder;
use tauri::http::{Request, Response, StatusCode};

use crate::gui_config;

/// `mml-music` 协议名，lib.rs 注册与前端地址前缀共用
pub const MUSIC_SCHEME: &str = "mml-music";

/// `mml-music` 自定义协议的访问前缀
///
/// 与 `image_base_url()` 同规则：Windows 与 Android 上自定义协议被映射到
/// `http://<scheme>.localhost`，其余平台是 `<scheme>://localhost`。
///
/// # 返回值
///
/// 可直接交给 `<audio>` 的地址前缀
pub fn music_base_url() -> &'static str {
    if cfg!(windows) || cfg!(target_os = "android") {
        "http://mml-music.localhost"
    } else {
        "mml-music://localhost"
    }
}

/// 按扩展名给 Content-Type
///
/// webview 的 `<audio>` 认这个头；认不出来时给 `application/octet-stream`，
/// 浏览器还会自己嗅探，不至于放不出来。
///
/// - `path`: 音频文件路径
///
/// # 返回值
///
/// MIME 字符串
fn mime_of(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("mp3") => "audio/mpeg",
        Some("m4a") | Some("mp4") => "audio/mp4",
        Some("wav") => "audio/wav",
        Some("ogg") | Some("oga") => "audio/ogg",
        Some("flac") => "audio/flac",
        Some("aac") => "audio/aac",
        Some("webm") => "audio/webm",
        _ => "application/octet-stream",
    }
}

/// 配置里填的是不是 http(s) 直链
///
/// - `source`: 配置里的音乐来源
///
/// # 返回值
///
/// 是直链返回 `true`，其余（本地路径、空串）返回 `false`
fn is_remote(source: &str) -> bool {
    let lower = source.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// 以 200 响应一份音频字节
///
/// - `responder`: 协议响应器
/// - `data`: 音频字节
/// - `mime`: 响应体的 Content-Type
fn send_ok(responder: UriSchemeResponder, data: Vec<u8>, mime: &str) {
    responder.respond(
        Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", mime)
            .body(data)
            .unwrap(),
    );
}

/// 以 404 响应：没配音乐、配的是直链、或文件读不出来
///
/// - `responder`: 协议响应器
fn send_not_found(responder: UriSchemeResponder) {
    responder.respond(
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Vec::new())
            .unwrap(),
    );
}

/// `mml-music` 协议请求：把配置里那份自定义音乐原样吐回去
///
/// - `_request`: 请求；本协议不看路径，音乐只有一份
/// - `responder`: 协议响应器
pub(crate) fn url_music(_request: Request<Vec<u8>>, responder: UriSchemeResponder) {
    let source = gui_config::get().client.music_source;
    let source = source.trim().to_string();
    if source.is_empty() || is_remote(&source) {
        send_not_found(responder);
        return;
    }

    match std::fs::read(&source) {
        Ok(data) => send_ok(responder, data, mime_of(Path::new(&source))),
        Err(err) => {
            // 文件被挪走 / 删掉是常事：记一条日志，前端那边表现为放不出来
            mml_log::warn(
                crate::log_text::LogText::MusicReadFailed {
                    source,
                    error: err.to_string(),
                }
                .text(),
            );
            send_not_found(responder);
        }
    }
}
