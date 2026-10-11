//! 头像的 2D 平铺渲染，正面叠加头顶层

use tiny_skia::Pixmap;

use crate::skin_draw::{
    SCALE_TYPEA, SCALE_TYPEB, draw, draw_mix, draw_with_fill_image, draw_with_fill_image_mix, scale,
};

/// 渲染 TypeA 风格的 2D 头像，平铺且无外扩边距
///
/// - `image`: 皮肤贴图
///
/// # 返回值
///
/// 返回渲染后的位图，贴图尺寸异常时返回 `None`
pub fn head_2d_draw_typea(image: &Pixmap) -> Option<Pixmap> {
    let mut dest = Pixmap::new(8, 8)?;

    draw(&mut dest, image, 0, 0, 8, 8, 8, 8)?;
    draw_mix(&mut dest, image, 0, 0, 40, 8, 8, 8)?;
    scale(&dest, SCALE_TYPEA)
}

/// 渲染 TypeB 风格的 2D 头像，带外扩边距的放大版
///
/// - `image`: 皮肤贴图
///
/// # 返回值
///
/// 返回渲染后的位图，贴图尺寸异常时返回 `None`
pub fn head_2d_draw_typeb(image: &Pixmap) -> Option<Pixmap> {
    let mut dest = Pixmap::new(72, 72)?;

    draw_with_fill_image(&mut dest, image, 4, 4, 8, 8, 8, 8, 8, 8)?;
    draw_with_fill_image_mix(&mut dest, image, 0, 0, 40, 8, 8, 8, 9, 9)?;

    scale(&dest, SCALE_TYPEB)
}
