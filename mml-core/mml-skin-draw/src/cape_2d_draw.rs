//! 披风 / 鞘翅正面、背面的 2D 平铺渲染
//!
//! 两种贴图都是 64x32 的盒展开：披风盒 10x16x1 的贴图块在 (0,0)，两块分别是
//! 正面与背面；鞘翅翼盒 10x20x2 的贴图块在 (22,0)，只有一片翼，
//! 另一片镜像拼出，按收拢（飞行前）与张开（飞行中）两种姿态出图。

use tiny_skia::Pixmap;

use crate::skin_draw::{BPP, SCALE_TYPEA, draw, row_bytes, scale};

/// 鞘翅翼外侧块的左上角横坐标，单位像素，即翼盒展开的后面
const WING_BLOCK_X: i32 = 36;
/// 鞘翅翼外侧块的左上角纵坐标，单位像素
const WING_BLOCK_Y: i32 = 2;
/// 鞘翅翼的宽度，单位像素
const WING_W: i32 = 10;
/// 鞘翅翼的高度，单位像素
const WING_H: i32 = 20;
/// 张开时两翼之间的间隔，单位像素，即两肩点之间的距离
const WING_GAP: i32 = 10;
/// 鞘翅 2D 的输出宽度，单位像素；收拢 320 见方，张开 300x60
const ELYTRA_WIDTH: u32 = 320;

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

/// 渲染鞘翅收拢（飞行前）的 2D 图：两片翼竖着并排，与物品图标一致
///
/// 左半用翼块原样，右半镜像，两翼贴在一起。
///
/// - `image`: 鞘翅贴图
///
/// # 返回值
///
/// 返回渲染后的位图，贴图放不下翼块时返回 `None`
pub fn elytra_folded(image: &Pixmap) -> Option<Pixmap> {
    let (w, h) = (WING_W as u32, WING_H as u32);
    let mut dest = Pixmap::new(w * 2, h)?;
    check_block(image)?;

    for y in 0..h {
        for x in 0..w {
            // 左半原样，右半取同一行第 9 - x 个像素，镜像
            for (dst_x, src_x) in [(x, x), (w + x, w - 1 - x)] {
                copy_pixel(image, src_x as i32, y as i32, &mut dest, dst_x, y);
            }
        }
    }

    scale(&dest, (ELYTRA_WIDTH / (w * 2)) as usize)
}

/// 渲染鞘翅张开（飞行中）的 2D 图：两片翼横着摊开，与 3D 的张开姿态对应
///
/// 翼盒的翼长方向横过来，翼根贴中缝、翼尖朝外，两翼之间留出肩点间隔；
/// 右翼是左翼的镜像。
///
/// - `image`: 鞘翅贴图
///
/// # 返回值
///
/// 返回渲染后的位图，贴图放不下翼块时返回 `None`
pub fn elytra_flying(image: &Pixmap) -> Option<Pixmap> {
    let (w, h) = (WING_W as u32, WING_H as u32);
    let gap = WING_GAP as u32;
    let mut dest = Pixmap::new(h * 2 + gap, w)?;
    check_block(image)?;

    for v in 0..h {
        for u in 0..w {
            // 左翼：翼根 v = 0 靠中缝，翼尖 v = h - 1 朝外；右翼整体镜像
            let left_x = h - 1 - v;
            let right_x = h + gap + v;
            copy_pixel(image, u as i32, v as i32, &mut dest, left_x, u);
            copy_pixel(image, u as i32, v as i32, &mut dest, right_x, u);
        }
    }

    scale(&dest, (ELYTRA_WIDTH / (h * 2 + gap)) as usize)
}

/// 翼块能否从贴图里取到
///
/// # 返回值
///
/// 取得到返回 `Some(())`，越界返回 `None`
fn check_block(image: &Pixmap) -> Option<()> {
    if WING_BLOCK_X + WING_W > image.width() as i32 || WING_BLOCK_Y + WING_H > image.height() as i32
    {
        return None;
    }

    Some(())
}

/// 把一个源像素按原样复制到目标位置
///
/// 位图内部是预乘 BGRA，整字节复制即可，不涉及通道换算。
fn copy_pixel(src: &Pixmap, src_x: i32, src_y: i32, dest: &mut Pixmap, dest_x: u32, dest_y: u32) {
    let src_off =
        (WING_BLOCK_Y + src_y) as usize * row_bytes(src) + (WING_BLOCK_X + src_x) as usize * BPP;
    let dst_off = dest_y as usize * row_bytes(dest) + dest_x as usize * BPP;

    dest.data_mut()[dst_off..dst_off + BPP].copy_from_slice(&src.data()[src_off..src_off + BPP]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::IntSize;

    /// 造一张 64x32 贴图，`mark` 标记的像素不透明，其余透明
    fn make_texture(mark: impl Fn(i32, i32) -> bool) -> Pixmap {
        let mut data = vec![0u8; 64 * 32 * BPP];
        for y in 0..32 {
            for x in 0..64 {
                if mark(x, y) {
                    let off = y as usize * 64 * BPP + x as usize * BPP;
                    data[off..off + BPP].copy_from_slice(&[0, 0, 255, 255]);
                }
            }
        }

        Pixmap::from_vec(data, IntSize::from_wh(64, 32).unwrap()).unwrap()
    }

    /// 取直乘 RGBA
    fn pixel(image: &Pixmap, x: u32, y: u32) -> [u8; 4] {
        let px = image.pixel(x, y).unwrap().demultiply();

        [px.red(), px.green(), px.blue(), px.alpha()]
    }

    /// 披风正 / 背面取的是两块不同的贴图区域
    #[test]
    fn test_cape_front_back_blocks() {
        let image = make_texture(|x, y| (1..11).contains(&x) && (1..17).contains(&y));

        let front = front(&image).expect("正面应出图");
        assert_eq!((front.width(), front.height()), (160, 256));
        assert_eq!(pixel(&front, 8, 8)[3], 255, "正面块应不透明");

        let back = back(&image).expect("背面应出图");
        assert_eq!(pixel(&back, 8, 8)[3], 0, "背面块应为空");
    }

    /// 收拢：320 见方，左半用翼块原样、右半是镜像
    #[test]
    fn test_elytra_folded_layout() {
        // 翼块左半边不透明，右半边空
        let image = make_texture(|x, y| (36..41).contains(&x) && (2..22).contains(&y));

        let out = elytra_folded(&image).expect("收拢应出图");
        assert_eq!((out.width(), out.height()), (320, 320));

        // 左半（x 0..5）有图，右半（x 5..10）为空；镜像后右翼相反
        assert_eq!(pixel(&out, 2 * 16, 8 * 16)[3], 255, "左翼左半边应有图");
        assert_eq!(pixel(&out, 8 * 16, 8 * 16)[3], 0, "左翼右半边应为空");
        assert_eq!(pixel(&out, 12 * 16, 8 * 16)[3], 0, "右翼左半边应为空");
        assert_eq!(pixel(&out, 18 * 16, 8 * 16)[3], 255, "右翼右半边应有图");
    }

    /// 张开：300x60，翼长方向横过来，中间留出肩点间隔
    #[test]
    fn test_elytra_flying_layout() {
        // 翼根那端（v 小）贴图有图
        let image = make_texture(|x, y| (36..46).contains(&x) && (2..10).contains(&y));

        let out = elytra_flying(&image).expect("张开应出图");
        assert_eq!((out.width(), out.height()), (300, 60));

        // 两翼的翼根都贴中缝，所以有图的一段都在靠中缝那侧，外侧为空
        assert_eq!(pixel(&out, 18 * 6, 4 * 6)[3], 255, "左翼靠中缝侧应有图");
        assert_eq!(pixel(&out, 1 * 6, 4 * 6)[3], 0, "左翼翼尖侧应为空");
        assert_eq!(pixel(&out, 31 * 6, 4 * 6)[3], 255, "右翼靠中缝侧应有图");
        assert_eq!(pixel(&out, 48 * 6, 4 * 6)[3], 0, "右翼翼尖侧应为空");

        // 中缝（x 20..30）整块透明
        for x in (20 * 6)..(30 * 6) {
            for y in 0..60 {
                assert_eq!(pixel(&out, x, y)[3], 0, "中缝应透明 ({x},{y})");
            }
        }
    }

    /// 贴图放不下翼块时返回 None
    #[test]
    fn test_elytra_block_out_of_range() {
        let small = Pixmap::new(32, 32).unwrap();
        assert!(
            elytra_folded(&small).is_none(),
            "32 宽的贴图放不下 (36,2) 的块"
        );
        assert!(
            elytra_flying(&small).is_none(),
            "32 宽的贴图放不下 (36,2) 的块"
        );
    }
}
