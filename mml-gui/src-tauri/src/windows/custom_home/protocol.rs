//! `mml-home` 自定义协议：把用户导入的整包网页资源当站点伺服
//!
//! 从 `custom_home/mod.rs` 拆出来的。路径一律按"ZIP 内相对路径"处理，**必须先过
//! `web_assets::resolve_entry`**，它会拒绝 `..`、绝对路径与盘符，否则自定义主页就成了
//! 任意文件读取入口 —— 这是本模块最要紧的不变量。`url_custom_home` 由 `lib.rs`
//! 注册为协议处理器，按原路径再导出。
//!
//! 整包读取、越界防护、MIME 与桥接注入都在 `windows/web_assets.rs`，与 `mml-ui` 共用。

use tauri::UriSchemeResponder;
use tauri::http::Request;

use crate::windows::web_assets::{
    BRIDGE_JS, BRIDGE_PATH, BRIDGE_TAG, inject_bridge, mime_of, resolve_entry, send_not_found,
    send_ok,
};

use super::archive::{INDEX_FILE, open_handle};

/// `mml-home` 协议入口：从常驻的压缩包里按条目读取，按 MIME 返回
///
/// `/` 或空路径落到 `index.html`，含「整包套一层目录」的兜底；`/__mml_bridge.js`
/// 是内置资源，直接返回桥接脚本，不在包里；query string 与 fragment 由
/// `uri().path()` 天然滤掉，`?v=...` 只是破缓存；全程不落盘，内容是
/// 整包句柄的 `read` 从 zip 条目现读出来的。
///
/// - `req`: 协议请求
/// - `res`: 协议响应器
pub(crate) async fn url_custom_home(req: Request<Vec<u8>>, res: UriSchemeResponder) {
    let path = req.uri().path();

    if path == BRIDGE_PATH {
        send_ok(res, BRIDGE_JS.as_bytes().to_vec(), "application/javascript");
        return;
    }

    let Some(archive) = open_handle() else {
        send_not_found(res);
        return;
    };
    let Some(name) = resolve_entry(&archive, path, INDEX_FILE) else {
        send_not_found(res);
        return;
    };
    let Some(data) = archive.read(name) else {
        send_not_found(res);
        return;
    };

    let mime = mime_of(name);
    let data = if mime == "text/html" {
        inject_bridge(data, BRIDGE_TAG)
    } else {
        data
    };
    send_ok(res, data, mime);
}
