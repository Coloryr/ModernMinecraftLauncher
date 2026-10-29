//! 图片加载：实例图标 / 方块 / 物品 / 截图，解码后按内容类型回给 webview
//!
//! 从 `image_manager/mod.rs` 拆出来的。每个加载器都是"查内存缓存 → 读盘 → 解码 / 缩放 →
//! 回响应"，缓存表仍在 `mod.rs`，它们是跨请求共享的窗口级状态。

use std::path::Path;
use std::str::FromStr;

use tauri::UriSchemeResponder;
use uuid::Uuid;

use mml_sys::path_helper;

use super::INSTANCE_IMAGE;
use super::respond::{read_as_png, send_bad, send_png};

/// 实例图标，URI 形如 `instance/<uuid>`：内存缓存命中直接回，未命中按图标来源取图
///
/// 图标与「方块 ID」二选一，见 `InstanceSettingObj::get_icon_file`：
/// 设了方块 ID 就按 ID 现渲染，方块贴图未渲染完时拿不到图，返回占位失败；
/// 否则读 `icon.png`，上传的自定义图片与老实例都走这条。
///
/// - `uri`: 协议路径片段，第 1 段是路由名 `instance`，第 2 段是实例 uuid
/// - `res`: 响应回写句柄
pub(super) fn load_instance_image(uri: &[&str], res: UriSchemeResponder) {
    if uri.len() != 2 {
        send_bad(res);
        return;
    }
    let Ok(uuid) = Uuid::from_str(uri[1]) else {
        send_bad(res);
        return;
    };
    let image = INSTANCE_IMAGE.read().unwrap().get(&uuid).cloned();
    if let Some(data) = image {
        send_png(res, data);
        return;
    }

    let Some(instance) = mml_game::get_instance(&uuid) else {
        send_bad(res);
        return;
    };

    // 分支一：设了方块 ID → 用方块渲染结果；锁内只读配置，不整份 clone 实例
    let block_id = {
        let inst = instance.read().unwrap();
        crate::gui_setting::block_icon_id(&inst)
    };
    if let Some(id) = block_id {
        let file = mml_tex_draw::get_block_path(&id).or_else(|| mml_tex_draw::get_item_path(&id));
        let Some(file) = file else {
            // 方块贴图还没渲染 / 该 id 不存在：此时没有可用的图标
            send_bad(res);
            return;
        };
        let Some(image) = read_as_png(file) else {
            send_bad(res);
            return;
        };
        cache_and_send(uuid, image, res);
        return;
    }

    // 分支二：没有方块 ID → 读图标文件，Icon 已清空的实例按默认 icon.png 找
    let icon = instance.read().unwrap().get_icon_file_or_default();
    if !icon.is_file() {
        send_bad(res);
        return;
    }
    let Some(image) = read_as_png(icon) else {
        send_bad(res);
        return;
    };

    cache_and_send(uuid, image, res);
}

/// 把实例图标写进内存缓存并回给请求方
///
/// - `uuid`: 实例 uuid
/// - `image`: 已解码的 PNG 字节
/// - `res`: 响应回写句柄
pub(super) fn cache_and_send(uuid: Uuid, image: Vec<u8>, res: UriSchemeResponder) {
    INSTANCE_IMAGE.write().unwrap().insert(uuid, image.clone());
    send_png(res, image);
}

/// 方块贴图，URI 形如 `block/<方块ID>`
///
/// PNG 直接读盘透传，不进内存缓存，重渲染后立即生效；
/// webview 自身的缓存由 URL 上的 `?v=<版本>` 破掉
///
/// - `uri`: 协议路径片段，第 1 段是路由名 `block`，第 2 段是方块 ID
/// - `res`: 响应回写句柄
pub(super) fn load_block_image(uri: &[&str], res: UriSchemeResponder) {
    if uri.len() != 2 {
        send_bad(res);
        return;
    }
    let Some(file) = mml_tex_draw::get_block_path(uri[1]) else {
        send_bad(res);
        return;
    };
    match path_helper::read_byte(&file) {
        Ok(data) => send_png(res, data),
        Err(_) => send_bad(res),
    }
}

/// 物品贴图，URI 形如 `item/<物品ID>`，与方块同策略
///
/// PNG 直接读盘透传，不进内存缓存，重渲染后立即生效；
/// webview 自身的缓存由 URL 上的 `?v=<版本>` 破掉
///
/// - `uri`: 协议路径片段，第 1 段是路由名 `item`，第 2 段是物品 ID
/// - `res`: 响应回写句柄
pub(super) fn load_item_image(uri: &[&str], res: UriSchemeResponder) {
    if uri.len() != 2 {
        send_bad(res);
        return;
    }
    let Some(file) = mml_tex_draw::get_item_path(uri[1]) else {
        send_bad(res);
        return;
    };
    match path_helper::read_byte(&file) {
        Ok(data) => send_png(res, data),
        Err(_) => send_bad(res),
    }
}

/// 实例截图，URI 形如 `screenshot/<实例uuid>/<文件名>`
///
/// PNG 读盘透传，不进内存缓存，游戏运行中新增截图下次请求即可见
///
/// - `uri`: 协议路径片段，第 1 段是路由名 `screenshot`，第 2 段是实例 uuid，
///   第 3 段是文件名
/// - `res`: 响应回写句柄
pub(super) fn load_screenshot_image(uri: &[&str], res: UriSchemeResponder) {
    if uri.len() != 3 {
        send_bad(res);
        return;
    }
    let Ok(uuid) = Uuid::from_str(uri[1]) else {
        send_bad(res);
        return;
    };
    // 文件名必须是纯文件名，防路径穿越
    let name = Path::new(uri[2]);
    if name.file_name() != Some(name.as_os_str()) {
        send_bad(res);
        return;
    }

    let Some(instance) = mml_game::get_instance(&uuid) else {
        send_bad(res);
        return;
    };
    // 锁内只取目录，drop 后再做文件操作
    let dir = instance.read().unwrap().get_screenshots_path();
    let file = dir.join(name);
    if !file.starts_with(&dir) {
        send_bad(res);
        return;
    }
    match path_helper::read_byte(&file) {
        Ok(data) => send_png(res, data),
        Err(_) => send_bad(res),
    }
}
