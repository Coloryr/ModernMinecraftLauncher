//! 像素级绘制基元
//!
//! 所有函数直接按预乘 RGBA8 字节操作，行间无填充；
//! 绘制失败如越界统一返回 `None`。

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
/// - `dest_x`: 目标区域左上角横坐标
/// - `dest_y`: 目标区域左上角纵坐标
/// - `src_x`: 源区域左上角横坐标
/// - `src_y`: 源区域左上角纵坐标
/// - `width`: 区域宽度
/// - `height`: 区域高度
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
/// - `dest_x`: 目标区域左上角横坐标
/// - `dest_y`: 目标区域左上角纵坐标
/// - `src_x`: 源区域左上角横坐标
/// - `src_y`: 源区域左上角纵坐标
/// - `width`: 区域宽度
/// - `height`: 区域高度
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
            let dst_offset = (dest_y + j) as usize * dst_row_bytes + (dest_x + i) as usize * BPP;

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
///
/// 目前没有调用方，留着与 [`fill_image`] 成对；将来仍没人用就删掉。
#[allow(dead_code)]
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
/// - `x`: 目标区域左上角横坐标
/// - `y`: 目标区域左上角纵坐标
/// - `sx`: 源区域左上角横坐标
/// - `sy`: 源区域左上角纵坐标
/// - `swidth`: 源区域宽度，每像素对应一个色块
/// - `sheight`: 源区域高度，每像素对应一个色块
/// - `width`: 单个色块宽度
/// - `height`: 单个色块高度
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
/// - `x`: 目标区域左上角横坐标
/// - `y`: 目标区域左上角纵坐标
/// - `sx`: 源区域左上角横坐标
/// - `sy`: 源区域左上角纵坐标
/// - `swidth`: 源区域宽度，每像素对应一个色块
/// - `sheight`: 源区域高度，每像素对应一个色块
/// - `width`: 单个色块宽度
/// - `height`: 单个色块高度
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

/// 注意：下面的混合函数按 BGRA 字节序解释像素内存，byte0 当作 B；
/// 而位图的内存布局是 R,G,B,A，byte0 是 R。
/// 两者通道命名相反，但所有混合运算都按字节逐通道对称进行，
/// 除 alpha 即第 4 字节外不会产生错误结果。
/// 因此本文件中的测试一律直接比较原始字节，而不是比较语义上的 R/G/B 颜色。
#[cfg(test)]
#[path = "../tests/unit/skin_draw_tests.rs"]
mod tests;
