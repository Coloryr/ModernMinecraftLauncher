//! `mml-ui` 协议：把自定义 CSS 包与整页包当站点伺服
//!
//! CSS 走 `/css/<id>/<包内路径>`，`<id>` 只在启动时扫描出来的句柄表里查，不在表里的一律 404，
//! 天然挡掉路径拼接；整页包同样按包内条目解析，先过 `web_assets::resolve_entry`，与
//! `mml-home` 共用同一份越界防护。

use tauri::UriSchemeResponder;
use tauri::http::Request;

use crate::windows::web_assets::{
    BRIDGE_JS, BRIDGE_PATH, BRIDGE_TAG, inject_bridge, mime_of, resolve_entry, send_not_found,
    send_ok,
};

use super::store;
use super::{CSS_DIR, CSS_FILE, INDEX_FILE};

/// `mml-ui` 协议入口
///
/// `/css/<id>/<路径>` 读该 CSS 包里的条目，包内相对路径原样传进来，入口是 `custom.css`；
/// `/__mml_bridge.js` 是内置桥接脚本；其余路径都按整页包里的条目解析，`/` 落到
/// `index.html`。HTML 响应注入桥接脚本，页面因此能拿到 `window.mml`；CSS 包按原样返回，
/// 静态资源按扩展名给 MIME。query string 由 `uri().path()` 天然滤掉，`?v=` 只破缓存。
///
/// - `req`: 协议请求
/// - `res`: 协议响应器
pub(crate) fn url_custom_ui(req: Request<Vec<u8>>, res: UriSchemeResponder) {
    let path = req.uri().path();

    if path == BRIDGE_PATH {
        send_ok(res, BRIDGE_JS.as_bytes().to_vec(), "application/javascript");
        return;
    }

    let css_prefix = format!("/{CSS_DIR}/");
    if let Some(rest) = path.strip_prefix(css_prefix.as_str()) {
        let (id, rel) = rest.split_once('/').unwrap_or((rest, ""));
        if let Some(archive) = store::css_handle(id)
            && let Some(name) = resolve_entry(&archive, rel, CSS_FILE)
            && let Some(data) = archive.read(name)
        {
            send_ok(res, data, mime_of(name));
        } else {
            send_not_found(res);
        }
        return;
    }

    let Some(archive) = store::page_handle() else {
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
