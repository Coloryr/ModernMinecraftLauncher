//! 账户头像与设置页预览图
//!
//! 从 `image_manager/mod.rs` 拆出来的。账户型 URI `head/<账户类型>/<uuid>` 先解析账户、
//! 下载皮肤定 sha1，再转调 sha1 型渲染；用 2D 还是 3D 由配置决定。

use tauri::UriSchemeResponder;
use tiny_skia::Pixmap;

use crate::gui_config;

use super::HEAD_IMAGE;
use super::respond::{send_bad, send_png};
use super::skin::{
    gen_head_image, load_texture_bytes, parse_skin_type, render_skin_display, resolve_skin_files,
};

/// 账户头像，URI 形如 `head/<账户类型>/<uuid>`：解析账户当前皮肤后按配置的头像类型渲染
///
/// 保持账户型：账户列表高频渲染，不能每个头像先查一次纹理列表
///
/// - `uri`: 协议路径片段，首段 `head` 已剥；第 2 段为 `preview` 时走设置页样例预览
/// - `res`: 响应回写句柄
pub(super) async fn load_head_image(uri: &[&str], res: UriSchemeResponder) {
    if uri.len() != 3 {
        send_bad(res);
        return;
    }

    // 设置页样例预览 `head/preview/<uuid>`：内置样例皮肤按当前配置渲染，不查账户
    if uri[1] == "preview" {
        send_head_preview(res);
        return;
    }

    let Some(files) = resolve_skin_files(uri).await else {
        send_bad(res);
        return;
    };
    let Some(tex) = files.skin else {
        send_bad(res);
        return;
    };

    send_head_image(&tex.sha1, res).await;
}

/// 头像渲染，sha1 入口；账户型请求解析出当前皮肤后也转调这里
///
/// 缓存键带上头像渲染配置：切换头像模式或旋转角度后立即出新图而不是旧缓存；
/// 同一皮肤的头像跨账户共享
///
/// - `skin_sha1`: 皮肤贴图内容 SHA1
/// - `res`: 响应回写句柄
pub(super) async fn send_head_image(skin_sha1: &str, res: UriSchemeResponder) {
    let config = gui_config::get().head;
    let key = (
        skin_sha1.to_string(),
        format!("{:?}/{}/{}", config.head_type, config.x, config.y),
    );

    {
        let image = HEAD_IMAGE.read().unwrap().get(&key).cloned();
        if let Some(data) = image {
            send_png(res, data);
            return;
        }
    }

    let Some(bytes) = load_texture_bytes(skin_sha1).await else {
        send_bad(res);
        return;
    };
    let Ok(bitmap) = Pixmap::decode_png(&bytes) else {
        send_bad(res);
        return;
    };

    let Some(image) = gen_head_image(&bitmap) else {
        send_bad(res);
        return;
    };

    let Ok(data) = image.encode_png() else {
        send_bad(res);
        return;
    };

    HEAD_IMAGE.write().unwrap().insert(key, data.clone());

    send_png(res, data);
}

/// 设置页头像样例预览：内置皮肤按当前头像配置渲染
///
/// 不查账户、不落缓存，设置页切换配置后靠前端 URL 版本号重取，渲染开销可忽略
///
/// - `res`: 响应回写句柄
pub(super) fn send_head_preview(res: UriSchemeResponder) {
    let Some(bitmap) = Pixmap::decode_png(PREVIEW_SKIN).ok() else {
        send_bad(res);
        return;
    };
    let Some(head) = gen_head_image(&bitmap) else {
        send_bad(res);
        return;
    };
    match head.encode_png() {
        Ok(data) => send_png(res, data),
        Err(_) => send_bad(res),
    }
}

/// 设置页皮肤样例预览：内置皮肤按当前皮肤显示模式渲染
///
/// `skin_type` 直接采用 URI 段，样例皮肤是纤细模型，前端固定传 `slim`；不查账户不落缓存
///
/// - `skin_type`: 皮肤型号段，取值 `auto` / `old` / `new` / `slim`
/// - `res`: 响应回写句柄
pub(super) fn send_skin_preview(skin_type: &str, res: UriSchemeResponder) {
    let Some(st) = parse_skin_type(skin_type) else {
        send_bad(res);
        return;
    };
    let Some(bitmap) = Pixmap::decode_png(PREVIEW_SKIN).ok() else {
        send_bad(res);
        return;
    };
    let display = gui_config::get().skin_display;
    let Some(image) = render_skin_display(&bitmap, st, display) else {
        send_bad(res);
        return;
    };
    match image.encode_png() {
        Ok(data) => send_png(res, data),
        Err(_) => send_bad(res),
    }
}

/// 设置页头像样例皮肤，纤细模型，取自 mml-skin 的测试资源
pub(super) const PREVIEW_SKIN: &[u8] =
    include_bytes!("../../../../mml-core/mml-skin/tests/skin_slim.png");
