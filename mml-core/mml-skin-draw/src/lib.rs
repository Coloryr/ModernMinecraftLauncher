//! 皮肤/披风 2D 与 3D 头像渲染模块
//!
//! # 子模块
//!
//! | 模块 | 用途 |
//! |------|------|
//! | [`skin_draw`] | 像素级绘制基元：复制 / 混合 / 缩放 / 填充 |
//! | [`skin_2d_draw`] | 皮肤整体 2D 展开，TypeA / TypeB |
//! | [`skin_3d_draw`] | 皮肤整体 3D 等距渲染 |
//! | [`head_2d_draw`] | 2D 头像渲染 |
//! | [`head_3d_draw`] | 3D 头像渲染 |
//! | [`cape_2d_draw`] | 披风 2D 渲染 |

pub mod cape_2d_draw;
pub(crate) mod gpu_3d;
pub mod head_2d_draw;
pub mod head_3d_draw;
pub mod skin_2d_draw;
pub mod skin_3d_draw;

/// 像素级绘制基元
///
/// 所有函数直接按预乘 RGBA8 字节操作，行间无填充；
/// 绘制失败如越界统一返回 `None`。
pub mod skin_draw {
    use tiny_skia::Pixmap;

    /// TypeA 头像的放大倍数，8x8 放大到 128x128
    pub const SCALE_TYPEA: usize = 16;
    /// TypeB 头像/皮肤的放大倍数
    pub const SCALE_TYPEB: usize = 2;
    /// TypeA 皮肤的放大倍数，16x32 放大到 128x256
    pub const SCALE_TYPEC: usize = 8;

    /// 每像素字节数，预乘 RGBA8，行间无填充
    pub const BPP: usize = 4;

    /// 位图一行占用的字节数
    ///
    /// - `image`: 目标位图
    ///
    /// # 返回值
    ///
    /// 返回 `宽度 * 4` 字节
    #[inline]
    pub fn row_bytes(image: &Pixmap) -> usize {
        image.width() as usize * BPP
    }

    /// 整数倍降采样，预乘像素直接求平均即可，不需要还原成直乘
    pub(crate) fn downsample(src: &Pixmap, factor: u32) -> Option<Pixmap> {
        let width = src.width() / factor;
        let height = src.height() / factor;
        let mut out = Pixmap::new(width, height)?;

        let count = (factor * factor) as u32;

        for y in 0..height {
            for x in 0..width {
                let mut sum = [0u32; 4];

                for dy in 0..factor {
                    for dx in 0..factor {
                        let px = src.pixel(x * factor + dx, y * factor + dy)?;
                        sum[0] += px.red() as u32;
                        sum[1] += px.green() as u32;
                        sum[2] += px.blue() as u32;
                        sum[3] += px.alpha() as u32;
                    }
                }

                let offset = ((y * width + x) * 4) as usize;
                let dst = &mut out.data_mut()[offset..offset + 4];
                for (i, value) in sum.iter().enumerate() {
                    dst[i] = (value / count) as u8;
                }
            }
        }

        Some(out)
    }

    /// 按行复制源区域像素到目标位置
    ///
    /// - `dest`: 目标位图
    /// - `source`: 源位图
    /// - `dest_x` / `dest_y`: 目标区域左上角
    /// - `src_x` / `src_y`: 源区域左上角
    /// - `width` / `height`: 区域尺寸
    ///
    /// # 返回值
    ///
    /// 越界返回 `None`，尺寸为 0 视为无操作返回 `Some(())`
    pub fn draw(
        dest: &mut Pixmap,
        source: &Pixmap,
        dest_x: i32,
        dest_y: i32,
        src_x: i32,
        src_y: i32,
        width: i32,
        height: i32,
    ) -> Option<()> {
        if width <= 0 || height <= 0 {
            return Some(());
        }

        if dest_x < 0
            || dest_y < 0
            || dest_x + width > dest.width() as i32
            || dest_y + height > dest.height() as i32
            || src_x < 0
            || src_y < 0
            || src_x + width > source.width() as i32
            || src_y + height > source.height() as i32
        {
            return None;
        }

        let src_row_bytes = row_bytes(source);
        let dst_row_bytes = row_bytes(dest);

        for y in 0..height {
            let src_offset = (src_y + y) as usize * src_row_bytes + src_x as usize * BPP;
            let dst_offset = (dest_y + y) as usize * dst_row_bytes + dest_x as usize * BPP;
            let len = width as usize * BPP;

            let src_slice = &source.data()[src_offset..src_offset + len];
            dest.data_mut()[dst_offset..dst_offset + len].copy_from_slice(src_slice);
        }

        Some(())
    }

    /// 按字节混合一组像素，`over` 叠在 `under` 之上
    ///
    /// - `under`: 底色，按内存顺序的 4 字节
    /// - `over`: 源色，按内存顺序的 4 字节
    ///
    /// # 返回值
    ///
    /// 返回混合后的 4 字节；只有双方 alpha 都为 0 时结果的 alpha 才为 0
    ///
    /// 逐字节对称运算：三个颜色通道各自只和同位置的字节混合，
    /// 所以不受"内存里到底是 RGBA 还是 BGRA"的影响。
    pub fn mix_pixel(under: [u8; 4], over: [u8; 4]) -> [u8; 4] {
        let ap = over[3] as f32 / 255.0;
        let dp = 1.0 - ap;

        let mut out = [0u8; 4];
        for i in 0..3 {
            out[i] = (over[i] as f32 * ap + under[i] as f32 * dp) as u8;
        }

        out[3] = if under[3] == 0 && over[3] == 0 {
            0
        } else {
            255
        };

        out
    }

    /// 复制源区域像素到目标位置，并与目标已有像素做 alpha 混合
    ///
    /// - `dest`: 目标位图
    /// - `source`: 源位图
    /// - `dest_x` / `dest_y`: 目标区域左上角
    /// - `src_x` / `src_y`: 源区域左上角
    /// - `width` / `height`: 区域尺寸
    ///
    /// # 返回值
    ///
    /// 越界返回 `None`，尺寸为 0 视为无操作返回 `Some(())`
    pub fn draw_mix(
        dest: &mut Pixmap,
        source: &Pixmap,
        dest_x: i32,
        dest_y: i32,
        src_x: i32,
        src_y: i32,
        width: i32,
        height: i32,
    ) -> Option<()> {
        if width <= 0 || height <= 0 {
            return Some(());
        }

        if dest_x < 0
            || dest_y < 0
            || dest_x + width > dest.width() as i32
            || dest_y + height > dest.height() as i32
            || src_x < 0
            || src_y < 0
            || src_x + width > source.width() as i32
            || src_y + height > source.height() as i32
        {
            return None;
        }

        let src_row_bytes = row_bytes(source);
        let dst_row_bytes = row_bytes(dest);

        for j in 0..height {
            for i in 0..width {
                let src_offset = (src_y + j) as usize * src_row_bytes + (src_x + i) as usize * BPP;
                let dst_offset =
                    (dest_y + j) as usize * dst_row_bytes + (dest_x + i) as usize * BPP;

                let mut src_px = [0u8; 4];
                src_px.copy_from_slice(&source.data()[src_offset..src_offset + BPP]);

                let mut dst_px = [0u8; 4];
                dst_px.copy_from_slice(&dest.data()[dst_offset..dst_offset + BPP]);

                let mixed = mix_pixel(dst_px, src_px);

                dest.data_mut()[dst_offset..dst_offset + BPP].copy_from_slice(&mixed);
            }
        }

        Some(())
    }

    /// 最近邻整数倍放大
    ///
    /// - `source`: 源位图
    /// - `scale`: 放大倍数，每个源像素复制为 scale x scale 块
    ///
    /// # 返回值
    ///
    /// 返回放大后的位图，分配失败返回 `None`
    pub fn scale(source: &Pixmap, scale: usize) -> Option<Pixmap> {
        let src_width = source.width() as usize;
        let src_height = source.height() as usize;
        let dst_width = src_width * scale;
        let dst_height = src_height * scale;

        let src_row_bytes = row_bytes(source);

        let mut dst = Pixmap::new(dst_width as u32, dst_height as u32)?;

        let dst_row_bytes = row_bytes(&dst);

        // 最近邻缩放：批量复制
        for src_y in 0..src_height {
            let src_row_offset = src_y * src_row_bytes;

            // 目标行范围：每个源行重复 scale 次
            for repeat_y in 0..scale {
                let dst_y = src_y * scale + repeat_y;
                let dst_row_offset = dst_y * dst_row_bytes;

                for src_x in 0..src_width {
                    let src_offset = src_row_offset + src_x * BPP;
                    let mut color = [0u8; 4];
                    color.copy_from_slice(&source.data()[src_offset..src_offset + BPP]);

                    for repeat_x in 0..scale {
                        let dst_x = src_x * scale + repeat_x;
                        let dst_offset = dst_row_offset + dst_x * BPP;

                        dst.data_mut()[dst_offset..dst_offset + BPP].copy_from_slice(&color);
                    }
                }
            }
        }

        Some(dst)
    }

    /// 在指定区域填充颜色，`pix` 按内存顺序原样写入
    ///
    /// - `dest`: 目标位图
    /// - `x` / `y`: 区域左上角
    /// - `width` / `height`: 区域尺寸
    /// - `pix`: 填充颜色
    ///
    /// # 返回值
    ///
    /// 越界返回 `None`，尺寸为 0 视为无操作返回 `Some(())`
    pub fn fill_image(
        dest: &mut Pixmap,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        pix: [u8; 4],
    ) -> Option<()> {
        if width <= 0 || height <= 0 {
            return Some(());
        }

        if x < 0 || y < 0 || x + width > dest.width() as i32 || y + height > dest.height() as i32 {
            return None;
        }

        let dst_row_bytes = row_bytes(dest);

        for j in 0..height {
            let dst_offset = (y + j) as usize * dst_row_bytes + x as usize * BPP;
            let row = &mut dest.data_mut()[dst_offset..dst_offset + width as usize * BPP];
            for chunk in row.chunks_exact_mut(BPP) {
                chunk.copy_from_slice(&pix);
            }
        }

        Some(())
    }

    /// 带混合的填充
    ///
    /// - `dest`: 目标位图
    /// - `x` / `y`: 区域左上角
    /// - `width` / `height`: 区域尺寸
    /// - `pix`: 填充颜色，作为源色与目标像素混合
    ///
    /// # 返回值
    ///
    /// 越界返回 `None`，尺寸为 0 视为无操作返回 `Some(())`
    pub fn fill_image_mix(
        dest: &mut Pixmap,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        pix: [u8; 4],
    ) -> Option<()> {
        if width <= 0 || height <= 0 {
            return Some(());
        }

        if x < 0 || y < 0 || x + width > dest.width() as i32 || y + height > dest.height() as i32 {
            return None;
        }

        let dst_row_bytes = row_bytes(dest);

        for j in 0..height {
            for i in 0..width {
                let dst_offset = (y + j) as usize * dst_row_bytes + (x + i) as usize * BPP;

                let mut dst_px = [0u8; 4];
                dst_px.copy_from_slice(&dest.data()[dst_offset..dst_offset + BPP]);

                let mixed = mix_pixel(dst_px, pix);

                dest.data_mut()[dst_offset..dst_offset + BPP].copy_from_slice(&mixed);
            }
        }

        Some(())
    }

    /// 把源区域的每个源像素放大填充为 width x height 的色块
    ///
    /// - `dest`: 目标位图
    /// - `source`: 源位图
    /// - `x` / `y`: 目标区域左上角
    /// - `sx` / `sy`: 源区域左上角
    /// - `swidth` / `sheight`: 源区域尺寸，每像素对应一个色块
    /// - `width` / `height`: 单个色块尺寸
    ///
    /// # 返回值
    ///
    /// 越界返回 `None`，尺寸为 0 视为无操作返回 `Some(())`
    pub fn draw_with_fill_image(
        dest: &mut Pixmap,
        source: &Pixmap,
        x: i32,
        y: i32,
        sx: i32,
        sy: i32,
        swidth: i32,
        sheight: i32,
        width: i32,
        height: i32,
    ) -> Option<()> {
        if swidth <= 0 || sheight <= 0 || width <= 0 || height <= 0 {
            return Some(());
        }

        if x < 0
            || y < 0
            || sx < 0
            || sy < 0
            || sx + swidth > source.width() as i32
            || sy + sheight > source.height() as i32
            || x + swidth * width > dest.width() as i32
            || y + sheight * height > dest.height() as i32
        {
            return None;
        }

        let src_row_bytes = row_bytes(source);
        let dst_row_bytes = row_bytes(dest);

        for i in 0..swidth {
            for j in 0..sheight {
                let src_offset = (sy + j) as usize * src_row_bytes + (sx + i) as usize * BPP;
                let mut color = [0u8; 4];
                color.copy_from_slice(&source.data()[src_offset..src_offset + BPP]);

                let dest_x = i * width + x;
                let dest_y = j * height + y;
                for fy in 0..height {
                    let dst_offset = (dest_y + fy) as usize * dst_row_bytes + dest_x as usize * BPP;
                    let row = &mut dest.data_mut()[dst_offset..dst_offset + width as usize * BPP];
                    for chunk in row.chunks_exact_mut(BPP) {
                        chunk.copy_from_slice(&color);
                    }
                }
            }
        }

        Some(())
    }

    /// 把源区域的每个源像素放大填充为 width x height 的色块，并与目标像素混合
    ///
    /// - `dest`: 目标位图
    /// - `source`: 源位图
    /// - `x` / `y`: 目标区域左上角
    /// - `sx` / `sy`: 源区域左上角
    /// - `swidth` / `sheight`: 源区域尺寸，每像素对应一个色块
    /// - `width` / `height`: 单个色块尺寸
    ///
    /// # 返回值
    ///
    /// 越界返回 `None`，尺寸为 0 视为无操作返回 `Some(())`
    pub fn draw_with_fill_image_mix(
        dest: &mut Pixmap,
        source: &Pixmap,
        x: i32,
        y: i32,
        sx: i32,
        sy: i32,
        swidth: i32,
        sheight: i32,
        width: i32,
        height: i32,
    ) -> Option<()> {
        if swidth <= 0 || sheight <= 0 || width <= 0 || height <= 0 {
            return Some(());
        }

        if x < 0
            || y < 0
            || sx < 0
            || sy < 0
            || sx + swidth > source.width() as i32
            || sy + sheight > source.height() as i32
            || x + swidth * width > dest.width() as i32
            || y + sheight * height > dest.height() as i32
        {
            return None;
        }

        let src_row_bytes = row_bytes(source);
        let dst_row_bytes = row_bytes(dest);

        for i in 0..swidth {
            for j in 0..sheight {
                let src_offset = (sy + j) as usize * src_row_bytes + (sx + i) as usize * BPP;
                let mut src_px = [0u8; 4];
                src_px.copy_from_slice(&source.data()[src_offset..src_offset + BPP]);

                let dest_x = i * width + x;
                let dest_y = j * height + y;
                for fy in 0..height {
                    for fx in 0..width {
                        let dst_offset =
                            (dest_y + fy) as usize * dst_row_bytes + (dest_x + fx) as usize * BPP;

                        let mut dst_px = [0u8; 4];
                        dst_px.copy_from_slice(&dest.data()[dst_offset..dst_offset + BPP]);

                        let mixed = mix_pixel(dst_px, src_px);

                        dest.data_mut()[dst_offset..dst_offset + BPP].copy_from_slice(&mixed);
                    }
                }
            }
        }

        Some(())
    }
}

/// 注意：下面的混合函数按 BGRA 字节序解释像素内存，byte0 当作 B；
/// 而位图的内存布局是 R,G,B,A，byte0 是 R。
/// 两者通道命名相反，但所有混合运算都按字节逐通道对称进行，
/// 除 alpha 即第 4 字节外不会产生错误结果。
/// 因此本文件中的测试一律直接比较原始字节，而不是比较语义上的 R/G/B 颜色。
#[cfg(test)]
mod tests {
    use super::skin_draw::*;
    use tiny_skia::{IntSize, Pixmap};

    /// 创建位图，并对每个像素调用填充函数得到 4 字节，按内存顺序
    fn make_bitmap(w: u32, h: u32, fill: impl Fn(i32, i32) -> [u8; 4]) -> Pixmap {
        let mut data = vec![0u8; (w * h) as usize * BPP];
        for (i, chunk) in data.chunks_exact_mut(BPP).enumerate() {
            let (x, y) = (i as i32 % w as i32, i as i32 / w as i32);
            chunk.copy_from_slice(&fill(x, y));
        }

        Pixmap::from_vec(data, IntSize::from_wh(w, h).unwrap()).unwrap()
    }

    /// 读取位图某像素的原始 4 字节
    fn get_bytes(bm: &Pixmap, x: i32, y: i32) -> [u8; 4] {
        let off = y as usize * row_bytes(bm) + x as usize * BPP;
        let mut out = [0u8; 4];
        out.copy_from_slice(&bm.data()[off..off + BPP]);
        out
    }

    /// 根据坐标生成一个唯一且不透明的字节模式，避免全图同色掩盖拷贝错位
    fn pattern(x: i32, y: i32) -> [u8; 4] {
        [
            (x * 7 + 1) as u8,
            (y * 11 + 2) as u8,
            (x * 13 + 3) as u8,
            255,
        ]
    }

    /// draw 应按行复制源区域像素到目标位置
    #[test]
    fn test_draw_copies_region() {
        let src = make_bitmap(8, 8, pattern);
        let mut dst = make_bitmap(8, 8, |_, _| [0, 0, 0, 0]);

        // 整图复制
        let r = draw(&mut dst, &src, 0, 0, 0, 0, 8, 8);
        assert!(r.is_some());
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(get_bytes(&dst, x, y), pattern(x, y), "整图复制 ({x},{y})");
            }
        }
    }

    /// draw 带偏移的子区域复制
    #[test]
    fn test_draw_sub_region_with_offset() {
        let src = make_bitmap(8, 8, pattern);
        let mut dst = make_bitmap(8, 8, |_, _| [0, 0, 0, 0]);

        // 源区域 (2,3) 4x5 复制到目标 (1,1)
        let r = draw(&mut dst, &src, 1, 1, 2, 3, 4, 5);
        assert!(r.is_some());
        for j in 0..5 {
            for i in 0..4 {
                assert_eq!(
                    get_bytes(&dst, 1 + i, 1 + j),
                    pattern(2 + i, 3 + j),
                    "子区域复制 ({i},{j})"
                );
            }
        }
        // 目标其他位置应保持为 0
        assert_eq!(get_bytes(&dst, 0, 0), [0, 0, 0, 0]);
        assert_eq!(get_bytes(&dst, 7, 7), [0, 0, 0, 0]);
    }

    /// draw 越界应返回 None，尺寸为 0 是无操作返回 Some
    #[test]
    fn test_draw_bounds_and_zero_size() {
        let src = make_bitmap(8, 8, pattern);
        let mut dst = make_bitmap(8, 8, |_, _| [0, 0, 0, 0]);

        // 目标越界
        assert!(draw(&mut dst, &src, 5, 5, 0, 0, 4, 4).is_none());
        // 源越界
        assert!(draw(&mut dst, &src, 0, 0, 5, 5, 4, 4).is_none());
        // 负坐标
        assert!(draw(&mut dst, &src, -1, 0, 0, 0, 2, 2).is_none());
        assert!(draw(&mut dst, &src, 0, 0, -1, 0, 2, 2).is_none());
        // 尺寸为 0：无操作，返回 Some
        assert!(draw(&mut dst, &src, 0, 0, 0, 0, 0, 8).is_some());
        // 目标未被修改
        assert_eq!(get_bytes(&dst, 3, 3), [0, 0, 0, 0]);
    }

    /// mix_pixel：不透明源完全覆盖，半透明按比例混合，双方都透明时结果透明
    #[test]
    fn test_color_mix() {
        // 不透明源覆盖：黑底 + 白源 = 白
        assert_eq!(
            mix_pixel([0, 0, 0, 255], [255, 255, 255, 255]),
            [255, 255, 255, 255]
        );

        // 半透明混合：黑底 + 50% 白源，各通道约为 128
        let mixed = mix_pixel([0, 0, 0, 255], [255, 255, 255, 128]);
        for ch in mixed.iter().take(3) {
            assert!((*ch as i32 - 128).abs() <= 1, "半透明混合通道 = {ch}");
        }
        assert_eq!(mixed[3], 255, "混合后应为不透明");

        // 双方都透明：结果 alpha 为 0
        let mixed = mix_pixel([10, 20, 30, 0], [40, 50, 60, 0]);
        assert_eq!(mixed[3], 0, "双方透明时结果应透明");
    }

    /// draw_mix：不透明源覆盖目标，透明源保持目标不变
    #[test]
    fn test_draw_mix_overwrite_and_keep() {
        let src = make_bitmap(4, 4, |_, _| [10, 20, 30, 255]);
        let mut dst = make_bitmap(4, 4, |_, _| [200, 210, 220, 255]);

        // 不透明源覆盖目标
        assert!(draw_mix(&mut dst, &src, 0, 0, 0, 0, 4, 4).is_some());
        assert_eq!(get_bytes(&dst, 2, 2), [10, 20, 30, 255]);

        // 透明源不改变目标
        let transparent = make_bitmap(4, 4, |_, _| [0, 0, 0, 0]);
        assert!(draw_mix(&mut dst, &transparent, 0, 0, 0, 0, 4, 4).is_some());
        assert_eq!(get_bytes(&dst, 2, 2), [10, 20, 30, 255]);
    }

    /// draw_mix 的 alpha 混合：50% 透明源在黑底上得到约半值
    #[test]
    fn test_draw_mix_half_alpha() {
        let src = make_bitmap(1, 1, |_, _| [255, 255, 255, 128]);
        let mut dst = make_bitmap(1, 1, |_, _| [0, 0, 0, 255]);

        assert!(draw_mix(&mut dst, &src, 0, 0, 0, 0, 1, 1).is_some());
        let b = get_bytes(&dst, 0, 0);
        // 255 * (1 - 128/255) ≈ 127
        for ch in b.iter().take(3) {
            assert!((*ch as i32 - 127).abs() <= 1, "半透明混合通道值 = {ch}");
        }
        assert_eq!(b[3], 255, "混合在黑底上结果应不透明");
    }

    /// draw_mix 边界与 draw 一致
    #[test]
    fn test_draw_mix_bounds() {
        let src = make_bitmap(4, 4, pattern);
        let mut dst = make_bitmap(4, 4, |_, _| [0, 0, 0, 0]);
        assert!(draw_mix(&mut dst, &src, 3, 3, 0, 0, 2, 2).is_none());
        assert!(draw_mix(&mut dst, &src, 0, 0, 0, 0, 0, 0).is_some());
    }

    /// scale 最近邻缩放：每个源像素被复制为 scale x scale 块
    #[test]
    fn test_scale_nearest_neighbor() {
        let src = make_bitmap(2, 2, |x, y| {
            if x == 0 && y == 0 {
                [1, 2, 3, 255]
            } else if x == 1 && y == 0 {
                [4, 5, 6, 255]
            } else if x == 0 && y == 1 {
                [7, 8, 9, 255]
            } else {
                [10, 11, 12, 255]
            }
        });

        let dst = scale(&src, 2).expect("scale 应成功");
        assert_eq!(dst.width(), 4);
        assert_eq!(dst.height(), 4);

        // 每个源像素放大为 2x2 块
        assert_eq!(get_bytes(&dst, 0, 0), [1, 2, 3, 255]);
        assert_eq!(get_bytes(&dst, 1, 1), [1, 2, 3, 255]);
        assert_eq!(get_bytes(&dst, 2, 0), [4, 5, 6, 255]);
        assert_eq!(get_bytes(&dst, 3, 1), [4, 5, 6, 255]);
        assert_eq!(get_bytes(&dst, 0, 2), [7, 8, 9, 255]);
        assert_eq!(get_bytes(&dst, 1, 3), [7, 8, 9, 255]);
        assert_eq!(get_bytes(&dst, 2, 2), [10, 11, 12, 255]);
        assert_eq!(get_bytes(&dst, 3, 3), [10, 11, 12, 255]);

        // scale 为 1 时应得到内容相同的副本
        let same = scale(&src, 1).expect("scale 1 应成功");
        assert_eq!(same.width(), 2);
        assert_eq!(get_bytes(&same, 1, 0), [4, 5, 6, 255]);
    }

    /// fill_image 按字节填充区域
    #[test]
    fn test_fill_image() {
        let mut dst = make_bitmap(8, 8, |_, _| [0, 0, 0, 0]);
        let pix = [10, 20, 30, 255];

        let r = fill_image(&mut dst, 2, 3, 4, 2, pix);
        assert!(r.is_some());

        for y in 3..5 {
            for x in 2..6 {
                assert_eq!(get_bytes(&dst, x, y), pix, "填充区域 ({x},{y})");
            }
        }
        // 区域外保持为 0
        assert_eq!(get_bytes(&dst, 0, 0), [0, 0, 0, 0]);
        assert_eq!(get_bytes(&dst, 7, 7), [0, 0, 0, 0]);
    }

    /// fill_image 边界检查
    #[test]
    fn test_fill_image_bounds() {
        let mut dst = make_bitmap(8, 8, |_, _| [0, 0, 0, 0]);
        let pix = [255, 255, 255, 255];
        assert!(fill_image(&mut dst, 6, 6, 4, 4, pix).is_none());
        assert!(fill_image(&mut dst, -1, 0, 2, 2, pix).is_none());
        assert!(fill_image(&mut dst, 0, 0, 0, 0, pix).is_some());
    }

    /// fill_image_mix：不透明填充覆盖，半透明填充按比例混合
    #[test]
    fn test_fill_image_mix() {
        let mut dst = make_bitmap(4, 4, |_, _| [0, 0, 0, 255]);
        let pix = [255, 255, 255, 255];

        // 不透明白色填充黑底 → 白
        assert!(fill_image_mix(&mut dst, 0, 0, 4, 4, pix).is_some());
        assert_eq!(get_bytes(&dst, 1, 1), [255, 255, 255, 255]);

        // 50% 黑填充白底 → 约半值
        let half_black = [0, 0, 0, 128];
        assert!(fill_image_mix(&mut dst, 0, 0, 4, 4, half_black).is_some());
        for ch in get_bytes(&dst, 2, 2).iter().take(3) {
            assert!((*ch as i32 - 127).abs() <= 1, "半透明填充通道值 = {ch}");
        }
        assert_eq!(get_bytes(&dst, 2, 2)[3], 255);
    }

    /// fill_image_mix 边界检查
    #[test]
    fn test_fill_image_mix_bounds() {
        let mut dst = make_bitmap(4, 4, |_, _| [0, 0, 0, 0]);
        let pix = [255, 255, 255, 255];
        assert!(fill_image_mix(&mut dst, 3, 3, 2, 2, pix).is_none());
        assert!(fill_image_mix(&mut dst, 0, 0, 0, 0, pix).is_some());
    }

    /// draw_with_fill_image：每个源像素填充为 width x height 的块
    #[test]
    fn test_draw_with_fill_image() {
        let src = make_bitmap(2, 2, pattern);
        let mut dst = make_bitmap(8, 8, |_, _| [0, 0, 0, 0]);

        // 源 2x2，每个像素填充为 4x4 块，铺满目标 8x8
        let r = draw_with_fill_image(&mut dst, &src, 0, 0, 0, 0, 2, 2, 4, 4);
        assert!(r.is_some());

        for j in 0..2 {
            for i in 0..2 {
                let expect = pattern(i, j);
                for fy in 0..4 {
                    for fx in 0..4 {
                        assert_eq!(
                            get_bytes(&dst, i * 4 + fx, j * 4 + fy),
                            expect,
                            "填充块 ({i},{j}) 内 ({fx},{fy})"
                        );
                    }
                }
            }
        }
    }

    /// draw_with_fill_image：块大小与边界检查
    #[test]
    fn test_draw_with_fill_image_bounds() {
        let src = make_bitmap(2, 2, pattern);
        let mut dst = make_bitmap(8, 8, |_, _| [0, 0, 0, 0]);

        // 2 个 4x4 块超出 8 宽度
        assert!(draw_with_fill_image(&mut dst, &src, 1, 0, 0, 0, 2, 2, 4, 4).is_none());
        // 源越界
        assert!(draw_with_fill_image(&mut dst, &src, 0, 0, 0, 0, 3, 2, 1, 1).is_none());
        // 尺寸为 0 是无操作
        assert!(draw_with_fill_image(&mut dst, &src, 0, 0, 0, 0, 2, 2, 0, 4).is_some());
    }

    /// draw_with_fill_image_mix：不透明源填充覆盖目标
    #[test]
    fn test_draw_with_fill_image_mix() {
        let src = make_bitmap(2, 2, |_, _| [10, 20, 30, 255]);
        let mut dst = make_bitmap(8, 8, |_, _| [200, 200, 200, 255]);

        let r = draw_with_fill_image_mix(&mut dst, &src, 0, 0, 0, 0, 2, 2, 4, 4);
        assert!(r.is_some());
        // 不透明源完全覆盖
        for j in 0..8 {
            for i in 0..8 {
                assert_eq!(get_bytes(&dst, i, j), [10, 20, 30, 255], "({i},{j})");
            }
        }
    }

    /// draw_with_fill_image_mix 边界检查
    #[test]
    fn test_draw_with_fill_image_mix_bounds() {
        let src = make_bitmap(2, 2, pattern);
        let mut dst = make_bitmap(8, 8, |_, _| [0, 0, 0, 0]);
        assert!(draw_with_fill_image_mix(&mut dst, &src, 0, 1, 0, 0, 2, 2, 4, 4).is_none());
        assert!(draw_with_fill_image_mix(&mut dst, &src, 0, 0, 0, 0, 0, 2, 4, 4).is_some());
    }
}
