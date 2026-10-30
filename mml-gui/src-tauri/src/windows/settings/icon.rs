//! 自定义程序图标：客户端设置里选一张图，落成运行目录下的 program_icon.png
//
// 同一张图运行时还会转出 `program_icon.ico`：窗口 / 托盘用 PNG 解出来的像素就够，
// 但 Windows 的快捷方式、资源管理器只认 PE 或 `.ico`，选 PNG 时必须自己转一份。
//
// 图标跟着运行目录走（与 bg_image 同一套），所以服务器包带上这一份文件、
// 再带上界面配置里的文件名，玩家导入后启动器就用同一张图。

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use crate::gui_config;
use crate::log_text::LogText;

/// 图标文件名，落在运行目录下
pub(crate) const ICON_FILE: &str = "program_icon.png";
/// 同一张图转出来的系统图标格式，Windows 的快捷方式只认它
pub(crate) const ICO_FILE: &str = "program_icon.ico";
/// 图标长边上限，单位像素；任务栏与 Alt+Tab 用不到更大的
const ICON_MAX_SIDE: u32 = 256;

/// 已设置的程序图标文件路径
///
/// # 返回值
///
/// 配置里有文件名且文件存在时返回完整路径；没设置或文件被删掉返回 `None`
pub(crate) fn icon_path() -> Option<PathBuf> {
    let name = gui_config::get().client.program_icon;
    if name.is_empty() {
        return None;
    }
    let dir = gui_config::dir()?;
    let path = dir.join(name);
    path.is_file().then_some(path)
}

/// 读出可直接交给窗口的图标
///
/// # 返回值
///
/// 没设置、文件读不出来或图片解不开时返回 `None`，调用方回落到内置图标
pub(crate) fn custom_icon() -> Option<tauri::image::Image<'static>> {
    let path = icon_path()?;
    let bytes = std::fs::read(path).ok()?;
    let image = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    Some(tauri::image::Image::new_owned(
        image.into_raw(),
        width,
        height,
    ))
}

/// 系统图标格式（`.ico`）的路径，需要时现转
///
/// Windows 的快捷方式、资源管理器只认 PE / `.ico`，PNG 指过去是空白图标。
/// 转好的那份比 PNG 新就直接复用，否则按当前 PNG 重转一份。
///
/// # 返回值
///
/// 可用的 `.ico` 路径；没设置图标或转换失败返回 `None`
pub(crate) fn custom_ico() -> Option<PathBuf> {
    let png = icon_path()?;
    let ico = png.with_file_name(ICO_FILE);
    let fresh = match (std::fs::metadata(&ico), std::fs::metadata(&png)) {
        (Ok(ico), Ok(png)) => match (ico.modified(), png.modified()) {
            (Ok(ico), Ok(png)) => ico >= png,
            _ => false,
        },
        _ => false,
    };
    if fresh {
        return Some(ico);
    }

    let bytes = std::fs::read(&png).ok()?;
    let square = square_png(&bytes, ICON_MAX_SIDE)?;
    std::fs::write(&ico, ico_of_png(&square)).ok()?;
    Some(ico)
}

/// 把任意图片缩放到 `size` 见方的透明画布上，按 PNG 输出
///
/// 菜单 / 快捷方式这类壳层图标都要正方形，原图比例不一定对，所以居中贴到透明画布上。
///
/// - `bytes`: 原始图片字节，格式自动识别
/// - `size`: 边长，图标取 256
///
/// # 返回值
///
/// PNG 字节；解不出来返回 `None`
pub(crate) fn square_png(bytes: &[u8], size: u32) -> Option<Vec<u8>> {
    let image = image::load_from_memory(bytes).ok()?;
    let scaled = image.resize(size, size, image::imageops::FilterType::Lanczos3);
    let mut canvas = image::RgbaImage::new(size, size);
    let (width, height) = (scaled.width(), scaled.height());
    image::imageops::overlay(
        &mut canvas,
        &scaled.to_rgba8(),
        ((size - width) / 2) as i64,
        ((size - height) / 2) as i64,
    );

    let mut out = Vec::new();
    canvas
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .ok()?;
    Some(out)
}

/// 把一张 PNG 包成只有一条目的 ICO
///
/// 条目里直接放 PNG 数据（Vista 起 shell 认），省掉 BMP 那套掩码。
///
/// - `png`: PNG 字节，边长 256 时宽度 / 高度字段写 0
///
/// # 返回值
///
/// ICO 文件字节
pub(crate) fn ico_of_png(png: &[u8]) -> Vec<u8> {
    const HEADER: u32 = 6 + 16;
    let mut out = Vec::with_capacity(png.len() + HEADER as usize);
    out.extend_from_slice(&0u16.to_le_bytes()); // 保留
    out.extend_from_slice(&1u16.to_le_bytes()); // 1 = 图标
    out.extend_from_slice(&1u16.to_le_bytes()); // 条目数
    out.extend_from_slice(&[0, 0, 0, 0]); // 宽 256 / 高 256 / 调色板 / 保留
    out.extend_from_slice(&1u16.to_le_bytes()); // 颜色平面
    out.extend_from_slice(&32u16.to_le_bytes()); // 位深
    out.extend_from_slice(&(png.len() as u32).to_le_bytes());
    out.extend_from_slice(&HEADER.to_le_bytes()); // 数据偏移
    out.extend_from_slice(png);
    out
}

/// 把图标应用到当前所有窗口；`None` 表示换回内置图标
///
/// - `app`: 应用句柄，遍历它开着的窗口
/// - `icon`: 新图标，`None` 用内置的
pub(crate) fn apply_to_windows(app: &AppHandle, icon: Option<tauri::image::Image<'static>>) {
    let fallback = app.default_window_icon().cloned();
    for window in app.webview_windows().values() {
        let next = icon.clone().or_else(|| fallback.clone());
        if let Some(next) = next {
            let _ = window.set_icon(next);
        }
    }
}

/// 把任意格式的图片整理成方形图标大小并重新编码成 PNG
///
/// - `bytes`: 用户选的原图
///
/// # 返回值
///
/// PNG 编码后的字节；解码失败返回错误文案
fn normalize(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let image = image::load_from_memory(bytes)
        .map_err(|err| err.to_string())?
        .into_rgba8();
    let (width, height) = image.dimensions();
    let longest = width.max(height);
    let image = if longest > ICON_MAX_SIDE {
        let scale = ICON_MAX_SIDE as f32 / longest as f32;
        let width = ((width as f32 * scale) as u32).max(1);
        let height = ((height as f32 * scale) as u32).max(1);
        image::imageops::resize(&image, width, height, image::imageops::FilterType::Lanczos3)
    } else {
        image
    };
    let mut out = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|err| err.to_string())?;
    Ok(out.into_inner())
}

/// 设置自定义程序图标：复制到运行目录并立即应用到所有窗口
///
/// - `app`: 应用句柄，落盘后换图标
/// - `source`: 用户选的图片路径
///
/// # 返回值
///
/// 落盘的文件名，前端写进客户端设置后随配置一起保存
#[gui_macros::ipc_group("settings")]
#[gui_macros::command]
pub async fn settings_set_program_icon(app: AppHandle, source: String) -> Result<String, String> {
    let dir = gui_config::dir().ok_or_else(|| String::from("config dir missing"))?;
    let bytes = tokio::fs::read(&source).await.map_err(|err| err.to_string())?;
    let png = tokio::task::spawn_blocking(move || normalize(&bytes))
        .await
        .map_err(|err| err.to_string())??;

    let file = dir.join(ICON_FILE);
    tokio::fs::write(&file, &png)
        .await
        .map_err(|err| err.to_string())?;

    let mut config = gui_config::get();
    config.client.program_icon = ICON_FILE.to_string();
    gui_config::set(config);
    // 顺手把系统图标格式转出来：快捷方式 / 资源管理器只认 .ico，等用到时再转会卡一下
    let _ = custom_ico();

    mml_log::info(
        LogText::ProgramIconSet {
            path: file.display().to_string(),
        }
        .text(),
    );
    apply_to_windows(&app, custom_icon());
    Ok(ICON_FILE.to_string())
}

/// 读已设置的程序图标，设置窗口打开时用它显示预览
///
/// # 返回值
///
/// PNG 的 data URL；没设置或文件读不出来返回 `None`
#[gui_macros::ipc_group("settings")]
#[gui_macros::command]
pub fn settings_get_program_icon() -> Option<String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

    let bytes = std::fs::read(icon_path()?).ok()?;
    Some(format!("data:image/png;base64,{}", BASE64.encode(bytes)))
}

/// 清除自定义程序图标：删文件、清配置并换回内置图标
///
/// - `app`: 应用句柄，清完换回内置图标
#[gui_macros::ipc_group("settings")]
#[gui_macros::command]
pub async fn settings_clear_program_icon(app: AppHandle) -> Result<(), String> {
    if let Some(dir) = gui_config::dir() {
        let _ = tokio::fs::remove_file(dir.join(ICON_FILE)).await;
        let _ = tokio::fs::remove_file(dir.join(ICO_FILE)).await;
    }
    let mut config = gui_config::get();
    config.client.program_icon.clear();
    gui_config::set(config);
    apply_to_windows(&app, None);
    Ok(())
}
