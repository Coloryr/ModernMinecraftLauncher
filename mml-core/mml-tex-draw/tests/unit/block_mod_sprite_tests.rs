use super::*;

use tiny_skia::Pixmap;

/// 构造竖排动画条带，n 帧 16x16，每帧灰度渐变便于区分
fn anim_strip(frames: usize) -> Pixmap {
    let mut data = vec![0u8; 16 * 16 * frames * 4];
    for (i, px_chunk) in data.chunks_exact_mut(4).enumerate() {
        // 每帧整体一种不透明颜色，帧间可区分，去重不会合并
        let frame = (i / (16 * 16)) as u8;
        px_chunk.copy_from_slice(&[frame * 60, 128, 64, 255]);
    }

    Pixmap::from_vec(
        data,
        tiny_skia::IntSize::from_wh(16, 16 * frames as u32).unwrap(),
    )
    .unwrap()
}

/// 动画条带时间线逐帧取图与 APNG 编码，render_baked 的核心环节
#[test]
fn sprite_apng_smoke() {
    let strip = anim_strip(4);
    let meta = AnimMeta {
        frametime: 20,
        interpolate: true,
        frames: None,
    };
    // 逐步定位失败环节
    let tl = AnimTimeline::build(&strip, &meta);
    assert!(tl.is_some(), "timeline构建失败");
    let tl = tl.unwrap();
    assert_eq!(tl.total_ticks(), 80, "total_ticks应=80");
    let mut frames = Vec::new();
    for k in (0..tl.total_ticks()).step_by(20) {
        let bm = tl.frame_at(k).expect("frame_at应成功");
        let crate::model::BitmapRgba {
            data: px,
            width: w,
            height: h,
            ..
        } = crate::model::bitmap_rgba(&bm).expect("位图读出应成功");
        assert_eq!((w, h), (16, 16));
        frames.push(ApngFrame {
            rgba: px,
            ticks: 20,
        });
    }
    let data = encode_apng(16, frames);
    assert!(data.is_some(), "APNG应成功");
    // 16×16纯色帧压缩后很小，仅验证编码产物有效
    assert!(data.unwrap().len() > 100, "APNG应有实际内容");
}
