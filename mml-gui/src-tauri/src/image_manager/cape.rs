//! 披风 / 鞘翅图：原始披风贴图，与披风 2D 正 / 背面、鞘翅 2D 收拢 / 张开、鞘翅 3D 渲染图
//!
//! 从 `image_manager/mod.rs` 拆出来的。渲染缓存键是内容 sha1 加渲染档位，与皮肤、头像同款；
//! 账户型 URI `*/<账户类型>/<uuid>` 先解析该账户当前选中的披风，再转 sha1 型渲染。

use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

use mml_skin_draw::{
    DrawCapeType, DrawElytraType, draw_cape, draw_cape_3d, draw_elytra, draw_elytra_3d,
};
use tauri::UriSchemeResponder;
use tiny_skia::Pixmap;

use crate::gui_config;

use super::respond::{send_bad, send_png};
use super::skin::{is_sha1, load_texture_bytes, resolve_skin_files};
use super::{
    CAPE_3D_IMAGE, CAPE_BACK_IMAGE, CAPE_IMAGE, CAPERAW_IMAGE, ELYTRA_3D_IMAGE,
    ELYTRA_FOLDED_IMAGE, ELYTRA_FLYING_IMAGE,
};

/// 披风贴图的渲染档位，决定缓存与出图方式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum CapeRender {
    /// 披风 2D 正面
    CapeFront,
    /// 披风 2D 背面
    CapeBack,
    /// 披风 3D 等距图，角度读配置
    Cape3D,
    /// 鞘翅 2D 收拢，飞行前
    ElytraFolded,
    /// 鞘翅 2D 张开，飞行中
    ElytraFlying,
    /// 鞘翅 3D 收拢
    Elytra3D,
}

impl CapeRender {
    /// 按档位出图
    ///
    /// - `image`: 披风贴图
    ///
    /// # 返回值
    ///
    /// 返回渲染后的位图；贴图尺寸异常，或 3D 档位没有可用 GPU 时返回 `None`
    fn render(&self, image: &Pixmap) -> Option<Pixmap> {
        match self {
            CapeRender::CapeFront => draw_cape(image, DrawCapeType::Front),
            CapeRender::CapeBack => draw_cape(image, DrawCapeType::Back),
            // 角度跟设置走：账户界面与设置页样例读的是同一份配置，缓存随配置变更清空
            CapeRender::Cape3D => {
                let cape = gui_config::get().cape;
                draw_cape_3d(image, cape.pitch, cape.yaw)
            }
            CapeRender::ElytraFolded => draw_elytra(image, DrawElytraType::Folded),
            CapeRender::ElytraFlying => draw_elytra(image, DrawElytraType::Flying),
            // 3D 角度固定为正对
            CapeRender::Elytra3D => draw_elytra_3d(image, 0.0, 0.0),
        }
    }

    /// 该档位的渲染缓存
    fn cache(&self) -> &'static LazyLock<RwLock<HashMap<String, Vec<u8>>>> {
        match self {
            CapeRender::CapeFront => &CAPE_IMAGE,
            CapeRender::CapeBack => &CAPE_BACK_IMAGE,
            CapeRender::Cape3D => &CAPE_3D_IMAGE,
            CapeRender::ElytraFolded => &ELYTRA_FOLDED_IMAGE,
            CapeRender::ElytraFlying => &ELYTRA_FLYING_IMAGE,
            CapeRender::Elytra3D => &ELYTRA_3D_IMAGE,
        }
    }

    /// 该档位在缓存里的键
    ///
    /// - `sha1`: 披风贴图内容 SHA1
    ///
    /// # 返回值
    ///
    /// 除 3D 外就是 sha1；披风 3D 的角度参与渲染，键里带上角度，
    /// 改角度自然换键，不必清缓存
    fn cache_key(&self, sha1: &str) -> String {
        match self {
            CapeRender::Cape3D => {
                let cape = gui_config::get().cape;
                format!("{sha1}@{}/{}", cape.pitch, cape.yaw)
            }
            _ => sha1.to_string(),
        }
    }
}

/// 原始披风贴图 PNG，URI 形如 `caperaw/<sha1>`，供前端 skinview3d 挂载
///
/// - `uri`: 协议路径片段，第 1 段是路由名 `caperaw`
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

/// 披风正面渲染图，URI 形如 `cape/<sha1>` 或 `cape/<账户类型>/<uuid>`
///
/// - `uri`: 协议路径片段，第 1 段是路由名 `cape`
/// - `res`: 响应回写句柄
pub(super) async fn load_cape_image(uri: &[&str], res: UriSchemeResponder) {
    load_cape_render(uri, CapeRender::CapeFront, res).await;
}

/// 披风背面渲染图，URI 形如 `capeback/<sha1>` 或 `capeback/<账户类型>/<uuid>`
///
/// - `uri`: 协议路径片段，第 1 段是路由名 `capeback`
/// - `res`: 响应回写句柄
pub(super) async fn load_cape_back_image(uri: &[&str], res: UriSchemeResponder) {
    load_cape_render(uri, CapeRender::CapeBack, res).await;
}

/// 披风 3D 渲染图，URI 形如 `cape3d/<sha1>` 或 `cape3d/<账户类型>/<uuid>`
///
/// 角度读 `gui_config` 的 `cape`，前端不传参。
///
/// - `uri`: 协议路径片段，第 1 段是路由名 `cape3d`
/// - `res`: 响应回写句柄
pub(super) async fn load_cape_3d_image(uri: &[&str], res: UriSchemeResponder) {
    load_cape_render(uri, CapeRender::Cape3D, res).await;
}

/// 鞘翅 2D 收拢（飞行前）渲染图，URI 形如 `elytrafolded/<sha1>` 或 `elytrafolded/<账户类型>/<uuid>`
///
/// - `uri`: 协议路径片段，第 1 段是路由名 `elytrafolded`
/// - `res`: 响应回写句柄
pub(super) async fn load_elytra_folded_image(uri: &[&str], res: UriSchemeResponder) {
    load_cape_render(uri, CapeRender::ElytraFolded, res).await;
}

/// 鞘翅 2D 张开（飞行中）渲染图，URI 形如 `elytraflying/<sha1>`
///
/// - `uri`: 协议路径片段，第 1 段是路由名 `elytraflying`
/// - `res`: 响应回写句柄
pub(super) async fn load_elytra_flying_image(uri: &[&str], res: UriSchemeResponder) {
    load_cape_render(uri, CapeRender::ElytraFlying, res).await;
}

/// 鞘翅 3D 渲染图，URI 形如 `elytra3d/<sha1>`
///
/// - `uri`: 协议路径片段，第 1 段是路由名 `elytra3d`
/// - `res`: 响应回写句柄
pub(super) async fn load_elytra_3d_image(uri: &[&str], res: UriSchemeResponder) {
    load_cape_render(uri, CapeRender::Elytra3D, res).await;
}

/// 披风 / 鞘翅渲染图的通用取图流程
///
/// - `uri`: 协议路径片段，首段路由名已剥。`<sha1>` 按内容直接取，
///   `<账户类型>/<uuid>` 解析该账户**当前选中**的披风后转 sha1 型渲染，账户列表预览用
/// - `kind`: 渲染档位
/// - `res`: 响应回写句柄
pub(super) async fn load_cape_render(uri: &[&str], kind: CapeRender, res: UriSchemeResponder) {
    // 设置页样例 `*/preview/<占位段>`：内置样例披风按当前档位与配置渲染，不查账户
    if uri.get(1) == Some(&"preview") {
        send_cape_preview(kind, res);
        return;
    }

    let Some(sha1) = resolve_cape_sha1(uri).await else {
        send_bad(res);
        return;
    };

    let cache = kind.cache();
    let key = kind.cache_key(&sha1);
    {
        let image = cache.read().unwrap().get(&key).cloned();
        if let Some(data) = image {
            send_png(res, data);
            return;
        }
    }

    let Some(bytes) = load_texture_bytes(&sha1).await else {
        send_bad(res);
        return;
    };
    let Ok(bitmap) = Pixmap::decode_png(&bytes) else {
        send_bad(res);
        return;
    };
    let Some(image) = kind.render(&bitmap) else {
        send_bad(res);
        return;
    };
    let Ok(data) = image.encode_png() else {
        send_bad(res);
        return;
    };

    cache.write().unwrap().insert(key, data.clone());

    send_png(res, data);
}

/// 设置页披风 / 鞘翅样例预览：内置样例披风按当前档位与配置渲染
///
/// 不查账户、不落缓存，设置页切换模式或角度后靠前端 URL 版本号重取，渲染开销可忽略。
///
/// - `kind`: 渲染档位
/// - `res`: 响应回写句柄
fn send_cape_preview(kind: CapeRender, res: UriSchemeResponder) {
    let Some(bitmap) = Pixmap::decode_png(PREVIEW_CAPE).ok() else {
        send_bad(res);
        return;
    };
    let Some(image) = kind.render(&bitmap) else {
        send_bad(res);
        return;
    };
    match image.encode_png() {
        Ok(data) => send_png(res, data),
        Err(_) => send_bad(res),
    }
}

/// 设置页样例披风贴图，取自 mml-skin-draw 的测试资源
///
/// 这张图同时含披风盒展开与鞘翅翼块，所以披风与鞘翅两档都能拿它出样。
const PREVIEW_CAPE: &[u8] =
    include_bytes!("../../../../mml-core/mml-skin-draw/tests/cape.png");

/// 解析披风贴图的内容 SHA1
///
/// - `uri`: 协议路径片段，第 1 段是路由名
///
/// # 返回值
///
/// `<路由>/<sha1>` 直接返回该 sha1；`<路由>/<账户类型>/<uuid>` 返回该账户当前
/// 选中的披风；段数不对、sha1 非法、账户不存在或该账户没有披风时返回 `None`
async fn resolve_cape_sha1(uri: &[&str]) -> Option<String> {
    if uri.len() == 2 {
        return is_sha1(uri[1]).then(|| uri[1].to_string());
    }
    if uri.len() != 3 {
        return None;
    }

    resolve_skin_files(uri).await?.cape.map(|tex| tex.sha1)
}
