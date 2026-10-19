//! 设置窗口的背景图：设置 / 缩放 / 清除 / 读取
//!
//! 从 `settings/mod.rs` 拆出来的。背景原图在内存里留一份 `BG_ORIGINAL`，
//! 缩放时以它为源、避免反复解码已缩放的图。命令带分组标记，值填原始组名
//! `settings`，免得组键随文件名变掉，见 AGENTS.md §4。

use std::sync::Mutex;

use tauri::{AppHandle, Emitter};

use crate::dtos::BgInfoDto;
use crate::listens;
use crate::err_keys;
use crate::log_text::LogText;

/// 一张图片的字节与它的实际 mime
#[derive(Debug, Clone)]
pub(super) struct ImageBytes {
    /// 图片字节
    pub(super) bytes: Vec<u8>,
    /// 实际 mime，取 `image/png` / `image/jpeg` / `image/webp`
    pub(super) mime: String,
}

/// 最近一次下载的原图缓存，仅网址来源且仅内存：调「原始大小」时按来源命中缓存，
/// 直接用原图重新缩放，不重新走网络；换来源或重启后失效，重新下载即可
#[derive(Debug, Clone)]
pub(super) struct BgOriginal {
    /// 来源网址，命中缓存时按它比对
    pub(super) source: String,
    /// 原图与它的 mime
    pub(super) image: ImageBytes,
}

/// 最近一次下载的原图
pub(super) static BG_ORIGINAL: Mutex<Option<BgOriginal>> = Mutex::new(None);

/// 加载背景图来源，得到原始字节与 mime
///
/// 网址以 `http` / `https` 开头时走 mml-net 客户端下载，mime 取响应的 Content-Type；
/// 其余按本地文件路径处理，mime 由扩展名推断，仅支持 png / jpeg / webp。
///
/// - `source`: 背景图来源，网址或本地文件路径
///
/// # 返回值
///
/// 见 [`ImageBytes`]；网址返回文本类型或本地扩展名不支持时返回错误信息
pub(super) async fn load_bg_bytes(source: &str) -> Result<ImageBytes, String> {
    if source.starts_with("http://") || source.starts_with("https://") {
        let client = mml_net::Client::new(mml_config::config_obj::ProxyState::Auto);
        let resp = client.get(source).await.map_err(|err| err.to_string())?;
        let status = resp.status();
        if !status.is_success() {
            return Err(format!("HTTP {status}"));
        }
        let mime = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("image/png")
            .split(';')
            .next()
            .unwrap_or("image/png")
            .trim()
            .to_string();
        // 文本响应基本是错误页，服务挂了会返回 HTML，别再往下走解码报不出所以然
        if mime.starts_with("text/") {
            return Err(format!("not an image (mime: {mime})"));
        }
        let bytes = resp.bytes().await.map_err(|err| err.to_string())?;
        Ok(ImageBytes {
            bytes: bytes.to_vec(),
            mime,
        })
    } else {
        let mime = match source
            .rsplit('.')
            .next()
            .map(|e| e.to_lowercase())
            .as_deref()
        {
            Some("png") => "image/png",
            Some("jpg") | Some("jpeg") => "image/jpeg",
            Some("webp") => "image/webp",
            _ => return Err(err_keys::BG_UNSUPPORTED.to_string()),
        };
        let path = source.to_string();
        let bytes = tauri::async_runtime::spawn_blocking(move || {
            std::fs::read(&path).map_err(|err| err.to_string())
        })
        .await
        .map_err(|err| err.to_string())??;
        Ok(ImageBytes {
            bytes,
            mime: mime.to_string(),
        })
    }
}

/// 源图字节 → 处理后的图片字节 + 实际 mime
///
/// **始终重新编码，不透传原图**：落盘的永远是处理过的版本，percent=100 只压缩不缩放。
/// JPEG 保持 JPEG 并用 q90，编码快；PNG 保持 PNG；WebP 解码后转 PNG，
/// 因为 image 的 WebP 编码只支持无损，又大又慢。
///
/// CPU 密集，调用方放阻塞线程池。
///
/// - `bytes`: 源图原始字节
/// - `mime`: 源图 mime，取 `image/png` / `image/jpeg` / `image/webp`
/// - `percent`: 缩放百分比，取值 10..=100
///
/// # 返回值
///
/// 见 [`ImageBytes`]，mime 是实际写出的那个，即 `image/jpeg` 或 `image/png`
pub(super) fn resize_bg_image(
    bytes: Vec<u8>,
    mime: &str,
    percent: u32,
) -> Result<ImageBytes, String> {
    let format = match mime {
        "image/png" => image::ImageFormat::Png,
        "image/jpeg" => image::ImageFormat::Jpeg,
        "image/webp" => image::ImageFormat::WebP,
        other => return Err(format!("unsupported mime: {other}")),
    };
    let img = image::load_from_memory_with_format(&bytes, format).map_err(|err| err.to_string())?;
    let scaled = if percent >= 100 {
        img
    } else {
        // 最小 1px 防止归零
        let scale = percent as f64 / 100.0;
        let w = ((img.width() as f64) * scale).round().max(1.0) as u32;
        let h = ((img.height() as f64) * scale).round().max(1.0) as u32;
        img.resize_exact(w, h, image::imageops::FilterType::CatmullRom)
    };
    let mut out: Vec<u8> = Vec::new();
    let out_mime = match format {
        image::ImageFormat::Jpeg => {
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90)
                .encode_image(&image::DynamicImage::ImageRgb8(scaled.to_rgb8()))
                .map_err(|err| err.to_string())?;
            "image/jpeg"
        }
        // PNG 原样保持；WebP 等转 PNG
        _ => {
            scaled
                .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
                .map_err(|err| err.to_string())?;
            "image/png"
        }
    };
    Ok(ImageBytes {
        bytes: out,
        mime: out_mime.to_string(),
    })
}

/// 缩放 → 落盘 → 记配置 → 广播刷新，设置与调大小共用的后半段流程
///
/// - `app`: 应用句柄，落盘后广播背景变更
/// - `source`: 背景图来源，写入配置供之后重新加载
/// - `image`: 源图字节与 mime
/// - `percent`: 缩放百分比，取值 10..=100
///
/// # 返回值
///
/// 处理后的 dataURL 供前端立即显示；其他窗口经 `bg-change` 事件刷新
pub(super) async fn store_bg(
    app: &AppHandle,
    source: String,
    image: ImageBytes,
    percent: u32,
) -> Result<String, String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

    let image = tauri::async_runtime::spawn_blocking(move || {
        resize_bg_image(image.bytes, &image.mime, percent)
    })
    .await
    .map_err(|err| err.to_string())
    .inspect_err(|err| {
        mml_log::error(LogText::BgResizeJoinFailed { error: err.to_string() }.text())
    })?
    .inspect_err(|err| {
        mml_log::error(LogText::BgResizeFailed { error: err.to_string() }.text())
    })?;

    // 处理结果落盘到配置目录，文件名不带扩展名、读取时按魔数识别格式，各窗口启动时从这里取图
    let Some(dir) = crate::gui_config::dir() else {
        mml_log::error(LogText::BgConfigDirMissing.text());
        return Err(err_keys::BG_CONFIG_DIR.to_string());
    };
    let write_data = image.bytes.clone();
    tauri::async_runtime::spawn_blocking(move || {
        std::fs::write(dir.join("bg_image"), write_data).map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| format!("set_bg write join failed: {err}"))
    .inspect_err(|err| mml_log::error(err.to_string()))?
    .inspect_err(|err| mml_log::error(LogText::BgWriteFailed { error: err.to_string() }.text()))?;

    // 来源与分辨率记入配置；set 会异步落盘 gui_config.json
    let mut config = crate::gui_config::get();
    config.bg_source = source;
    config.bg_native_size = percent;
    crate::gui_config::set(config);

    emit_bg_change(app);
    Ok(format!(
        "data:{};base64,{}",
        image.mime,
        BASE64.encode(&image.bytes)
    ))
}

/// 把下载结果写入原图缓存，仅网址来源用
///
/// - `source`: 背景图来源网址，命中缓存时按它比对
/// - `image`: 原图与它的 mime
pub(super) fn cache_bg_original(source: &str, image: &ImageBytes) {
    *BG_ORIGINAL.lock().unwrap() = Some(BgOriginal {
        source: source.to_string(),
        image: image.clone(),
    });
}

/// 设置背景图：每次都按来源重新加载 → 缩放到 `percent` → 落盘 `bg_image` → 记入配置；
/// 下载结果进 `BG_ORIGINAL` 供之后「调原始大小」复用。随机图网址每次拿到的是新图，
/// 所以不能吃缓存。
///
/// - `app`: 应用句柄，落盘后广播背景变更
/// - `source`: 背景图来源，网址或本地文件路径
/// - `percent`: 缩放百分比，超出 10..=100 会被夹到边界
///
/// # 返回值
///
/// 处理后的 dataURL 供前端立即显示；其他窗口经 `bg-change` 事件刷新
#[gui_macros::ipc_group("settings")]
#[gui_macros::command]
pub async fn settings_set_bg(
    app: AppHandle,
    source: String,
    percent: u32,
) -> Result<String, String> {
    let percent = percent.clamp(10, 100);
    mml_log::info(LogText::BgSet { percent, source: source.clone() }.text());

    let image = load_bg_bytes(&source).await.inspect_err(|err| {
        mml_log::error(LogText::BgLoadFailed { error: err.to_string() }.text());
    })?;
    if source.starts_with("http://") || source.starts_with("https://") {
        cache_bg_original(&source, &image);
    }
    store_bg(&app, source, image, percent).await
}

/// 调整原始大小：只按新 `percent` 重新缩放已设置的背景图，不改来源
///
/// 网址来源用 `BG_ORIGINAL` 里缓存的原图重新缩放，就是当前显示这张，不重新下载；
/// 缓存未命中即重启后才重新下载，随机图网址此时会换一张图。
/// 本地文件来源直接重读文件，同一文件每次内容一致。
///
/// - `app`: 应用句柄，落盘后广播背景变更
/// - `percent`: 缩放百分比，超出 10..=100 会被夹到边界
///
/// # 返回值
///
/// 处理后的 dataURL 供前端立即显示；没设置过背景图时返回 `no background image`
#[gui_macros::ipc_group("settings")]
#[gui_macros::command]
pub async fn settings_resize_bg(app: AppHandle, percent: u32) -> Result<String, String> {
    let percent = percent.clamp(10, 100);
    let source = crate::gui_config::get().bg_source;
    if source.is_empty() {
        return Err(err_keys::BG_NONE.to_string());
    }
    mml_log::info(LogText::BgResize { percent, source: source.clone() }.text());

    let is_url = source.starts_with("http://") || source.starts_with("https://");
    let cached = if is_url {
        BG_ORIGINAL
            .lock()
            .unwrap()
            .as_ref()
            .filter(|c| c.source == source)
            .map(|c| c.image.clone())
    } else {
        None
    };
    let image = match cached {
        Some(hit) => hit,
        None => {
            let image = load_bg_bytes(&source).await.inspect_err(|err| {
                mml_log::error(LogText::BgResizeLoadFailed { error: err.to_string() }.text());
            })?;
            if is_url {
                cache_bg_original(&source, &image);
            }
            image
        }
    };
    store_bg(&app, source, image, percent).await
}

/// 清除背景图：删除落盘文件并清空配置里的来源
///
/// - `app`: 应用句柄，清除后广播背景变更
///
/// # 返回值
///
/// 恒为 `Ok(())`，删文件失败也按清除成功处理
#[gui_macros::ipc_group("settings")]
#[gui_macros::command]
pub async fn settings_clear_bg(app: AppHandle) -> Result<(), String> {
    if let Some(dir) = crate::gui_config::dir() {
        let _ = tauri::async_runtime::spawn_blocking(move || {
            std::fs::remove_file(dir.join("bg_image"))
        })
        .await;
    }
    let mut config = crate::gui_config::get();
    config.bg_source = String::new();
    crate::gui_config::set(config);
    emit_bg_change(&app);
    Ok(())
}

/// 读取背景图，窗口启动与 `bg-change` 时调用：源地址 + 处理后的图 + 显示参数
///
/// # 返回值
///
/// 未设置背景图、或配置目录还没初始化时返回 `None`；读文件失败返回错误信息
#[gui_macros::ipc_group("settings")]
#[gui_macros::command]
pub fn settings_get_bg() -> Result<Option<BgInfoDto>, String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

    let config = crate::gui_config::get();
    if config.bg_source.is_empty() {
        return Ok(None);
    }
    let Some(dir) = crate::gui_config::dir() else {
        return Ok(None);
    };
    let bytes = std::fs::read(dir.join("bg_image"))
        .inspect_err(|err| mml_log::error(LogText::BgReadFailed { error: err.to_string() }.text()))
        .map_err(|err| err.to_string())?;
    let mime = sniff_bg_mime(&bytes);
    Ok(Some(BgInfoDto {
        source: config.bg_source,
        data_url: format!("data:{mime};base64,{}", BASE64.encode(bytes)),
        opacity: config.bg_opacity,
        blur: config.bg_blur,
        native_size: config.bg_native_size,
    }))
}

/// 从文件头嗅探图片 mime，识别 PNG / JPEG / WebP 魔数
///
/// - `bytes`: 图片文件字节
///
/// # 返回值
///
/// 识别到的 mime；未识别时按 PNG 处理
pub(super) fn sniff_bg_mime(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        "image/png"
    }
}

/// 广播背景图变更事件，跨窗口同步刷新背景层
///
/// 只改显示参数时也走这条，见 `window_save_gui_config`：不透明度 / 模糊没有专门的命令。
///
/// - `app`: 应用句柄，事件发往所有窗口
#[gui_macros::emit]
pub(crate) fn emit_bg_change(app: &AppHandle) {
    let _ = app.emit(listens::BG_CHANGE, ());
}
