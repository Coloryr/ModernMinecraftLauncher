//! PNG 与 APNG 编解码
//!
//! 解码贴图、逐帧拆分，以及把动画帧写成 APNG。
use std::io::Cursor;

use mml_names::i18_items::error_type::ErrorType;
use tiny_skia::Pixmap;

use super::*;

/// 解码贴图数据为预乘 RGBA8 位图
///
/// - `data`: PNG 文件字节
///
/// # 返回值
///
/// 返回解码后的位图，解码失败时返回 `None`
pub fn decode_png(data: &[u8]) -> Option<Pixmap> {
    Pixmap::decode_png(data).ok()
}

/// 预乘 RGBA 转 straight alpha，边界处供 GL 与 PNG 用
///
/// 按 a 归一化后就近取整，半值取偶；与 skia 的取值仅个别半值差 1
///
/// - `data`: 预乘 RGBA 像素
///
/// # 返回值
///
/// 返回 straight-alpha RGBA 像素
pub(crate) fn to_straight_rgba(data: &[u8]) -> Vec<u8> {
    let mut out = data.to_vec();

    for px in out.chunks_exact_mut(4) {
        let a = px[3] as u32;
        if a == 0 {
            px[0] = 0;
            px[1] = 0;
            px[2] = 0;
            continue;
        }
        if a == 255 {
            continue;
        }

        for c in px.iter_mut().take(3) {
            *c = (f32::from(*c) * 255.0 / a as f32).round_ties_even() as u8;
        }
    }

    out
}

/// 从竖排动画贴图中取出一帧
///
/// - `tex`: 竖排帧条带，帧为正方形，帧高等于宽
/// - `index`: 帧号，从 0 起
///
/// # 返回值
///
/// 返回该帧位图，帧越界时返回 `None`
pub(super) fn extract_frame(tex: &Pixmap, index: usize) -> Option<Pixmap> {
    let size = tex.width();
    let top = index as u32 * size;
    let stride = size as usize * 4;

    let mut frame = Pixmap::new(size, size)?;

    for y in 0..size as usize {
        let src = (top as usize + y) * stride;
        let dst = y * stride;
        frame.data_mut()[dst..dst + stride].copy_from_slice(&tex.data()[src..src + stride]);
    }

    Some(frame)
}

/// 把 RGBA 像素编码为 size×size 的 PNG，alpha 直通
///
/// - `size`: 输出边长，单位像素
/// - `rgba`: RGBA 像素，长度等于 size²×4
///
/// # 返回值
///
/// 返回 PNG 字节，编码失败时返回 `None`
pub(super) fn encode_png(size: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut encoder = png::Encoder::new(&mut cursor, size, size);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
        writer.finish().ok()?;
    }
    Some(cursor.into_inner())
}

/// APNG 的一帧：像素与它占用的显示时长
pub(crate) struct ApngFrame {
    /// 该帧的 RGBA 像素，长度等于边长²×4
    pub(crate) rgba: Vec<u8>,
    /// 该帧持续刻数，含采样步长跨过的刻数；渲染结果与前一帧相同时并入前一帧
    pub(crate) ticks: u16,
}

/// 把多帧合并成 APNG，每帧附带显示时长，单位刻
///
/// - `size`: 输出边长，单位像素
/// - `frames`: 帧列表，按显示顺序排列
///
/// # 返回值
///
/// 返回 APNG 字节，编码失败时返回 `None`
pub(crate) fn encode_apng(size: u32, frames: Vec<ApngFrame>) -> Option<Vec<u8>> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut encoder = png::Encoder::new(&mut cursor, size, size);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        if let Err(e) = encoder.set_animated(frames.len() as u32, 0) {
            log_apng_fail("set_animated", &e);
            return None;
        }
        let mut writer = match encoder.write_header() {
            Ok(writer) => writer,
            Err(e) => {
                log_apng_fail("write_header", &e);
                return None;
            }
        };
        for frame in &frames {
            // 帧显示时长 = 持续刻数 / FRAME_RATE 秒，采样跨过的刻数一并计入
            if let Err(e) = writer.set_frame_delay(frame.ticks, FRAME_RATE) {
                log_apng_fail("set_frame_delay", &e);
                return None;
            }
            if let Err(e) = writer.write_image_data(&frame.rgba) {
                log_apng_fail("write_image_data", &e);
                return None;
            }
        }
        if let Err(e) = writer.finish() {
            log_apng_fail("finish", &e);
            return None;
        }
    }
    Some(cursor.into_inner())
}

/// 记一条 APNG 编码失败日志
///
/// - `stage`: 失败的 png 编码步骤名，如 `set_animated`
/// - `error`: 底层编码错误
pub(super) fn log_apng_fail(stage: &str, error: &dyn std::fmt::Display) {
    mml_log::error_type(ErrorType::ApngEncodeFail {
        stage: stage.to_string(),
        error: error.to_string(),
    });
}
