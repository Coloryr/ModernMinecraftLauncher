//! 披风图：供前端 skinview3d 用的原始贴图，与正面 / 背面渲染图
//!
//! 从 `image_manager/mod.rs` 拆出来的。渲染缓存键同样是内容 sha1 + 显示模式。

use mml_skin_draw::{DrawCapeType, draw_cape};
use tauri::UriSchemeResponder;
use tiny_skia::Pixmap;

use super::respond::{send_bad, send_png};
use super::skin::{is_sha1, load_texture_bytes, resolve_skin_files};
use super::{CAPE_BACK_IMAGE, CAPE_IMAGE, CAPERAW_IMAGE};

/// 原始披风贴图 PNG，URI 形如 `caperaw/<sha1>`，供前端 skinview3d 挂载
///
/// - `uri`: 协议路径片段，首段 `caperaw` 已剥
/// - `res`: 响应回写句柄
pub(super) async fn load_caperaw_image(uri: &[&str], res: UriSchemeResponder) {
    if uri.len() != 2 {
        send_bad(res);
        return;
    }

    {
        let image = CAPERAW_IMAGE.read().unwrap().get(uri[1]).cloned();
        if let Some(data) = image {
            send_png(res, data);
            return;
        }
    }

    let Some(data) = load_texture_bytes(uri[1]).await else {
        send_bad(res);
        return;
    };

    CAPERAW_IMAGE
        .write()
        .unwrap()
        .insert(uri[1].to_string(), data.clone());

    send_png(res, data);
}

/// 披风渲染图，cape_2d_draw 正面
///
/// - `uri`: 协议路径片段，首段 `cape` 已剥。`cape/<sha1>` 是 sha1 型，按内容直接取；
///   `cape/<账户类型>/<uuid>` 是账户型，解析该账户**当前选中**的披风后转 sha1 型渲染，
///   账户列表预览用
/// - `res`: 响应回写句柄
pub(super) async fn load_cape_image(uri: &[&str], res: UriSchemeResponder) {
    if uri.len() == 2 {
        if is_sha1(uri[1]) {
            send_cape_image(uri[1], res).await;
        } else {
            send_bad(res);
        }
        return;
    }
    if uri.len() != 3 {
        send_bad(res);
        return;
    }

    let Some(files) = resolve_skin_files(uri).await else {
        send_bad(res);
        return;
    };
    let Some(tex) = files.cape else {
        send_bad(res);
        return;
    };
    send_cape_image(&tex.sha1, res).await;
}

/// 披风正面渲染，sha1 入口；账户型请求解析出当前披风后也转调这里
///
/// - `sha1`: 披风贴图内容 SHA1
/// - `res`: 响应回写句柄
pub(super) async fn send_cape_image(sha1: &str, res: UriSchemeResponder) {
    {
        let image = CAPE_IMAGE.read().unwrap().get(sha1).cloned();
        if let Some(data) = image {
            send_png(res, data);
            return;
        }
    }

    let Some(bytes) = load_texture_bytes(sha1).await else {
        send_bad(res);
        return;
    };
    let Ok(bitmap) = Pixmap::decode_png(&bytes) else {
        send_bad(res);
        return;
    };
    let Some(image) = draw_cape(&bitmap, DrawCapeType::Front) else {
        send_bad(res);
        return;
    };
    let Ok(data) = image.encode_png() else {
        send_bad(res);
        return;
    };

    CAPE_IMAGE
        .write()
        .unwrap()
        .insert(sha1.to_string(), data.clone());

    send_png(res, data);
}

/// 披风背面渲染图，cape_2d_draw 背面，供账户列表悬浮大图与正面并排显示
///
/// - `uri`: 协议路径片段，首段 `capeback` 已剥。`capeback/<sha1>` 是 sha1 型，
///   按内容直接取；`capeback/<账户类型>/<uuid>` 是账户型，解析该账户**当前选中**
///   的披风后转 sha1 型渲染
/// - `res`: 响应回写句柄
pub(super) async fn load_cape_back_image(uri: &[&str], res: UriSchemeResponder) {
    if uri.len() == 2 {
        if is_sha1(uri[1]) {
            send_cape_back_image(uri[1], res).await;
        } else {
            send_bad(res);
        }
        return;
    }
    if uri.len() != 3 {
        send_bad(res);
        return;
    }

    let Some(files) = resolve_skin_files(uri).await else {
        send_bad(res);
        return;
    };
    let Some(tex) = files.cape else {
        send_bad(res);
        return;
    };
    send_cape_back_image(&tex.sha1, res).await;
}

/// 披风背面渲染，sha1 入口；账户型请求解析出当前披风后也转调这里
///
/// - `sha1`: 披风贴图内容 SHA1
/// - `res`: 响应回写句柄
pub(super) async fn send_cape_back_image(sha1: &str, res: UriSchemeResponder) {
    {
        let image = CAPE_BACK_IMAGE.read().unwrap().get(sha1).cloned();
        if let Some(data) = image {
            send_png(res, data);
            return;
        }
    }

    let Some(bytes) = load_texture_bytes(sha1).await else {
        send_bad(res);
        return;
    };
    let Ok(bitmap) = Pixmap::decode_png(&bytes) else {
        send_bad(res);
        return;
    };
    let Some(image) = draw_cape(&bitmap, DrawCapeType::Back) else {
        send_bad(res);
        return;
    };
    let Ok(data) = image.encode_png() else {
        send_bad(res);
        return;
    };

    CAPE_BACK_IMAGE
        .write()
        .unwrap()
        .insert(sha1.to_string(), data.clone());

    send_png(res, data);
}
