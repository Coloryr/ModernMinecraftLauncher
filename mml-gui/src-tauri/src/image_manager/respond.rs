//! `mml-image` 协议的响应构造与图片归一化：状态码 / CORS 头 / MIME 嗅探 / 解码转 PNG
//!
//! 从 `image_manager/mod.rs` 拆出来的。协议回调**不能 panic**，一个请求炸掉就是一张图
//! 空白且无痕迹，所以这里的失败路径一律回 400，不 unwrap。

use std::fs;
use std::io::Cursor;
use std::path::Path;

use tauri::UriSchemeResponder;
use tauri::http::{Response, StatusCode};

/// 把请求路径按 `/` 拆成非空片段
///
/// Tauri 传入的 path 形如 `/instance/<uuid>`，始终带前导 `/`，
/// 直接 `split('/')` 会在首位多出一个空串。
///
/// - `path`: 请求的 URI 路径
///
/// # 返回值
///
/// 不含空串的片段列表
pub(super) fn split_path(path: &str) -> Vec<&str> {
    path.split('/').filter(|item| !item.is_empty()).collect()
}

/// 以 `image/png` 响应
///
/// - `res`: 响应回写句柄
/// - `data`: PNG 字节
pub(super) fn send_png(res: UriSchemeResponder, data: Vec<u8>) {
    res.respond(
        Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "image/png")
            // skinview3d 取 skinraw/caperaw 上传 WebGL 纹理要过 CORS，必须带本头
            .header("Access-Control-Allow-Origin", "*")
            .body(data)
            .unwrap(),
    );
}

/// 以 400 响应，参数不合法或资源不存在时用
///
/// - `res`: 响应回写句柄
pub(super) fn send_bad(res: UriSchemeResponder) {
    res.respond(
        Response::builder()
            .status(StatusCode::BAD_REQUEST)
            // 失败响应同样带 CORS 头，前端 fetch 才能读到失败状态而不是一律 ERR_FAILED
            .header("Access-Control-Allow-Origin", "*")
            .body(&[0u8; 0])
            .unwrap(),
    );
}

/// 按文件头嗅探图片类型
///
/// - `data`: 图片字节
///
/// # 返回值
///
/// 认不出格式返回 `None`
pub(super) fn sniff_mime(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("image/png")
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if data.starts_with(b"GIF8") {
        Some("image/gif")
    } else if data.len() > 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Some("image/webp")
    } else if data.len() > 12
        && &data[4..8] == b"ftyp"
        && (&data[8..12] == b"avif" || &data[8..12] == b"avis")
    {
        Some("image/avif")
    } else if data.starts_with(b"<?xml") || data.starts_with(b"<svg") {
        Some("image/svg+xml")
    } else if data.starts_with(b"BM") {
        Some("image/bmp")
    } else {
        None
    }
}

/// 归一化图标：优先转 PNG，解不动再原样透传
///
/// - `data`: 原始图片字节
///
/// # 返回值
///
/// 认不出格式返回 `None`
pub(super) fn normalize_icon(data: &[u8]) -> Option<IconBytes> {
    // 磁盘缓存里存的就是 PNG，见 write_icon_file 的调用点：先按文件头认一下直接透传，
    // 省掉一次"解码 + 重新编码"，收藏夹每次刷新图标都会走这条路径
    if sniff_mime(data) == Some("image/png") {
        return Some(IconBytes {
            data: data.to_vec(),
            mime: "image/png",
        });
    }

    if let Some(png) = decode_as_png(data) {
        return Some(IconBytes {
            data: png,
            mime: "image/png",
        });
    }

    let mime = sniff_mime(data)?;
    Some(IconBytes {
        data: data.to_vec(),
        mime,
    })
}

/// 按图标自己的内容类型响应
///
/// - `res`: 响应回写句柄
/// - `icon`: 已归一化的图标内容
pub(super) fn send_icon(res: UriSchemeResponder, icon: &IconBytes) {
    res.respond(
        Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", icon.mime)
            // 与 send_png 同理：webview 里跨源取图，如 canvas 绘制，需要 CORS 头
            .header("Access-Control-Allow-Origin", "*")
            .body(icon.data.clone())
            .unwrap(),
    );
}

/// 读文件并解码转 PNG
///
/// - `file`: 图片文件路径
///
/// # 返回值
///
/// 返回 PNG 数据；读取或解码失败返回 `None`
pub(super) fn read_as_png<P: AsRef<Path>>(file: P) -> Option<Vec<u8>> {
    decode_as_png(&fs::read(file).ok()?)
}

/// 把图片数据解码后统一转成 PNG，无法解码视为损坏
///
/// 用 `image` 按内容嗅探格式，覆盖 png / jpeg / webp：skia-safe 的预编译包只带
/// jpeg / png 解码，Modrinth 的图标是 webp，解不出来。
///
/// - `data`: 原始图片字节
///
/// # 返回值
///
/// 解码失败返回 `None`
pub(super) fn decode_as_png(data: &[u8]) -> Option<Vec<u8>> {
    let image = image::load_from_memory(data).ok()?;
    let mut out = Cursor::new(Vec::new());
    image.write_to(&mut out, image::ImageFormat::Png).ok()?;

    Some(out.into_inner())
}

/// 图标内容：能解码的统一转 PNG，解不动的 gif / avif / 动图 webp / svg 等
/// 按内容嗅探后原样透传，交给 webview 自己渲染
pub(super) struct IconBytes {
    /// PNG 或嗅探出的原始格式字节；字段给同级的 icon 等模块读，故 pub(super)
    pub(super) data: Vec<u8>,
    /// 内容类型
    pub(super) mime: &'static str,
}
