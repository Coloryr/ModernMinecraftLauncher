use mml_skin::{SkinType, skin_type_checker};
use tiny_skia::Pixmap;

use crate::skin_draw::{
    BPP, SCALE_TYPEB, SCALE_TYPEC, draw_mix, draw_with_fill_image, draw_with_fill_image_mix,
    fill_image, mix_pixel, row_bytes, scale,
};

/// 读取指定像素的原始 4 字节
///
/// - `image`: 源位图
/// - `x`: 像素横坐标
/// - `y`: 像素纵坐标
fn get_pixel(image: &Pixmap, x: i32, y: i32) -> [u8; 4] {
    let offset = y as usize * row_bytes(image) + x as usize * BPP;
    let mut out = [0u8; 4];
    out.copy_from_slice(&image.data()[offset..offset + BPP]);
    out
}

/// 写入指定像素的原始 4 字节
///
/// - `image`: 目标位图
/// - `x`: 像素横坐标
/// - `y`: 像素纵坐标
/// - `color`: 预乘 RGBA 颜色
fn set_pixel(image: &mut Pixmap, x: i32, y: i32, color: [u8; 4]) {
    let offset = y as usize * row_bytes(image) + x as usize * BPP;
    image.data_mut()[offset..offset + BPP].copy_from_slice(&color);
}

/// 创建 TypeA 风格的皮肤 2D 展开图
///
/// - `image`: 原始皮肤贴图
/// - `skin_type`: 皮肤类型，`None` 表示按贴图自动检测
///
/// # 返回值
///
/// 返回缩放 8 倍后的 2D 皮肤图片，尺寸 128x256；贴图尺寸异常时返回 `None`
pub fn typea(image: &Pixmap, skin_type: Option<SkinType>) -> Option<Pixmap> {
    // 创建中间 16x32 画布
    let mut image1 = Pixmap::new(16, 32)?;

    let skintype = skin_type.unwrap_or_else(|| skin_type_checker::get_skin_type(image));

    // head (8,8,8,8) -> (4,0)
    draw_mix(&mut image1, image, 4, 0, 8, 8, 8, 8)?;
    // head top (40,8,8,8) -> (4,0) 混合
    draw_mix(&mut image1, image, 4, 0, 40, 8, 8, 8)?;
    // body (20,20,8,12) -> (4,8)
    draw_mix(&mut image1, image, 4, 8, 20, 20, 8, 12)?;

    if skintype == SkinType::New || skintype == SkinType::NewSlim {
        // body over (20,36,8,12) -> (4,8) 混合
        draw_mix(&mut image1, image, 4, 8, 20, 36, 8, 12)?;
    }

    // right hand
    if skintype == SkinType::NewSlim {
        // (44,20,3,12) -> (1,8)
        draw_mix(&mut image1, image, 1, 8, 44, 20, 3, 12)?;
        // top (44,36,3,12) -> (1,8) 混合
        draw_mix(&mut image1, image, 1, 8, 44, 36, 3, 12)?;
    } else {
        // (44,20,4,12) -> (0,8)
        draw_mix(&mut image1, image, 0, 8, 44, 20, 4, 12)?;
        if skintype != SkinType::Old {
            // top (44,36,4,12) -> (0,8) 混合
            draw_mix(&mut image1, image, 0, 8, 44, 36, 4, 12)?;
        }
    }

    // left hand
    if skintype == SkinType::NewSlim {
        // (36,52,3,12) -> (12,8)
        draw_mix(&mut image1, image, 12, 8, 36, 52, 3, 12)?;
        // top (52,52,3,12) -> (12,8) 混合
        draw_mix(&mut image1, image, 12, 8, 52, 52, 3, 12)?;
    } else {
        if skintype == SkinType::Old {
            // 旧版镜像：从 image1 的右手区域镜像到左手位置，并混合 overlay 纹理
            // 源区 x 在 0..4、目标区 x 在 12..16，互不重叠，可以安全地边读边写
            for i in (0..4).rev() {
                for j in 0..12 {
                    let src_color = get_pixel(&image1, i, j + 8);
                    let mix_color = get_pixel(image, i + 44, j + 20);
                    let mixed = mix_pixel(src_color, mix_color);
                    set_pixel(&mut image1, i + 12, j + 8, mixed);
                }
            }
        } else {
            // (36,52,4,12) -> (12,8)
            draw_mix(&mut image1, image, 12, 8, 36, 52, 4, 12)?;
            // top (52,52,4,12) -> (12,8) 混合
            draw_mix(&mut image1, image, 12, 8, 52, 52, 4, 12)?;
        }
    }

    // right leg (4,20,4,12) -> (4,20)
    draw_mix(&mut image1, image, 4, 20, 4, 20, 4, 12)?;
    if skintype == SkinType::New || skintype == SkinType::NewSlim {
        // top (4,36,4,12) -> (4,20) 混合
        draw_mix(&mut image1, image, 4, 20, 4, 36, 4, 12)?;
    }

    // left leg
    if skintype == SkinType::Old {
        // 旧版镜像：从 image1 的右腿区域镜像到左腿位置，并混合 overlay 纹理
        // 源区 x 在 0..4、目标区 x 在 8..12，互不重叠
        for i in (0..4).rev() {
            for j in 0..12 {
                let src_color = get_pixel(&image1, i, j + 20);
                let mix_color = get_pixel(image, i + 4, j + 20);
                let mixed = mix_pixel(src_color, mix_color);
                set_pixel(&mut image1, i + 8, j + 20, mixed);
            }
        }
    } else {
        // (20,52,4,12) -> (8,20)
        draw_mix(&mut image1, image, 8, 20, 20, 52, 4, 12)?;
        // top (4,52,4,12) -> (8,20) 混合
        draw_mix(&mut image1, image, 8, 20, 4, 52, 4, 12)?;
    }

    // 缩放 8x
    scale(&image1, SCALE_TYPEC)
}

/// 创建 TypeB 风格的皮肤 2D 展开图
///
/// - `image`: 原始皮肤贴图
/// - `skin_type`: 皮肤类型，`None` 表示按贴图自动检测
///
/// # 返回值
///
/// 返回缩放 2 倍后的 2D 皮肤图片，尺寸 272x532；贴图尺寸异常时返回 `None`
pub fn typeb(image: &Pixmap, skin_type: Option<SkinType>) -> Option<Pixmap> {
    // 创建中间 136x266 画布
    let mut image1 = Pixmap::new(136, 266)?;

    let skintype = skin_type.unwrap_or_else(|| skin_type_checker::get_skin_type(image));

    // head: 从 (4+8*4, 4) 提取 (8,8)，填充到 (8,8) 块，目标位置 (4+8*4, 4)
    draw_with_fill_image(&mut image1, image, 4 + 8 * 4, 4, 8, 8, 8, 8, 8, 8)?;

    // body: 从 (4+8*8, 4+8*8) 提取 (20,20)，填充到 (8,12) 块，目标位置 (4+8*4, 4+8*8)
    draw_with_fill_image(
        &mut image1,
        image,
        4 + 8 * 4,
        4 + 8 * 8,
        20,
        20,
        8,
        12,
        8,
        8,
    )?;

    // right hand
    if skintype == SkinType::NewSlim {
        draw_with_fill_image(
            &mut image1,
            image,
            4 + 1 * 8,
            4 + 8 * 8,
            44,
            20,
            3,
            12,
            8,
            8,
        )?;
    } else {
        draw_with_fill_image(&mut image1, image, 4, 4 + 8 * 8, 44, 20, 4, 12, 8, 8)?;
    }

    // left hand
    if skintype == SkinType::NewSlim {
        draw_with_fill_image(
            &mut image1,
            image,
            4 + 12 * 8,
            4 + 8 * 8,
            36,
            52,
            3,
            12,
            8,
            8,
        )?;
    } else {
        if skintype == SkinType::Old {
            // 旧版镜像：从源图右手区域镜像到左手位置
            for i in (0..4).rev() {
                for j in 0..12 {
                    let pix = get_pixel(image, i + 44, j + 20);
                    fill_image(
                        &mut image1,
                        4 + 12 * 8 + i * 8,
                        4 + 8 * 8 + j * 8,
                        8,
                        8,
                        pix,
                    )?;
                }
            }
        } else {
            draw_with_fill_image(
                &mut image1,
                image,
                4 + 12 * 8,
                4 + 8 * 8,
                36,
                52,
                4,
                12,
                8,
                8,
            )?;
        }
    }

    // right leg
    draw_with_fill_image(
        &mut image1,
        image,
        4 + 4 * 8,
        4 + 20 * 8,
        4,
        20,
        4,
        12,
        8,
        8,
    )?;

    // left leg
    if skintype == SkinType::Old {
        // 旧版镜像：从源图右腿区域镜像到左腿位置
        for i in (0..4).rev() {
            for j in 0..12 {
                let pix = get_pixel(image, i + 4, j + 20);
                fill_image(
                    &mut image1,
                    4 + 8 * 8 + i * 8,
                    4 + 20 * 8 + j * 8,
                    8,
                    8,
                    pix,
                )?;
            }
        }
    } else {
        draw_with_fill_image(
            &mut image1,
            image,
            4 + 8 * 8,
            4 + 20 * 8,
            20,
            52,
            4,
            12,
            8,
            8,
        )?;
    }

    // body over (仅 New / NewSlim)
    if skintype == SkinType::New || skintype == SkinType::NewSlim {
        draw_with_fill_image_mix(&mut image1, image, 4 * 8, 8 * 8 - 2, 20, 36, 8, 12, 9, 9)?;
    }

    // head top
    draw_with_fill_image_mix(&mut image1, image, 4 * 9 - 4, 0, 40, 8, 8, 8, 9, 9)?;

    if skintype == SkinType::NewSlim {
        // top: 右手 overlay
        draw_with_fill_image_mix(
            &mut image1,
            image,
            1 * 8 + 1,
            8 * 8 + 2,
            44,
            36,
            3,
            12,
            9,
            9,
        )?;
        // top: 左手 overlay
        draw_with_fill_image_mix(
            &mut image1,
            image,
            12 * 8 + 4,
            8 * 8 + 2,
            52,
            52,
            3,
            12,
            9,
            9,
        )?;
    } else if skintype == SkinType::New {
        // top: 右手 overlay
        draw_with_fill_image_mix(&mut image1, image, 0, 8 * 8 + 2, 44, 36, 4, 12, 9, 9)?;
        // top: 左手 overlay
        draw_with_fill_image_mix(
            &mut image1,
            image,
            12 * 8 + 4,
            8 * 8 + 2,
            52,
            52,
            4,
            12,
            9,
            9,
        )?;
    }

    if skintype == SkinType::New || skintype == SkinType::NewSlim {
        // top: 右腿 overlay
        draw_with_fill_image_mix(
            &mut image1,
            image,
            4 * 8 + 2,
            20 * 8 - 2,
            4,
            36,
            4,
            12,
            9,
            9,
        )?;
        // top: 左腿 overlay
        draw_with_fill_image_mix(
            &mut image1,
            image,
            8 * 8 + 2,
            20 * 8 - 2,
            4,
            52,
            4,
            12,
            9,
            9,
        )?;
    }

    scale(&image1, SCALE_TYPEB)
}

/// 注意：draw_mix 按 BGRA 字节序解释像素内存，且整个管线 mix -> scale -> PNG 存取
/// 都保持同一套字节语义，所以本测试直接在字节层面做断言，不比较语义上的 R/G/B。
#[cfg(test)]
#[path = "../tests/unit/skin_2d_draw_tests.rs"]
mod tests;
