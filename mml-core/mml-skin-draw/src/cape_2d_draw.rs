//! 披风正面 / 背面的 2D 平铺渲染

use tiny_skia::Pixmap;

use crate::skin_draw::{SCALE_TYPEA, draw, scale};

/// 渲染披风正面 2D 图
///
/// - `image`: 披风贴图
///
/// # 返回值
///
/// 返回渲染后的位图，贴图尺寸异常时返回 `None`
pub fn front(image: &Pixmap) -> Option<Pixmap> {
    let mut dest = Pixmap::new(10, 16)?;

    draw(&mut dest, image, 0, 0, 1, 1, 10, 16)?;
    scale(&dest, SCALE_TYPEA)
}

/// 渲染披风背面 2D 图
///
/// - `image`: 披风贴图
///
/// # 返回值
///
/// 返回渲染后的位图，贴图尺寸异常时返回 `None`
pub fn back(image: &Pixmap) -> Option<Pixmap> {
    let mut dest = Pixmap::new(10, 16)?;

    draw(&mut dest, image, 0, 0, 12, 1, 10, 16)?;
    scale(&dest, SCALE_TYPEA)
}
