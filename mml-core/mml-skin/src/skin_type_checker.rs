use tiny_skia::Pixmap;

use crate::SkinType;

/// 全透明的预乘 RGBA 字节，四个分量均为 0
const TRANSPARENT: [u8; 4] = [0, 0, 0, 0];

/// 获取皮肤类型
///
/// - `image`: 皮肤位图
///
/// # 返回值
///
/// 返回按尺寸与纤细标记区域判定的皮肤类型
pub fn get_skin_type(image: &Pixmap) -> SkinType {
    let width = image.width() as i32;
    let height = image.height() as i32;

    if width >= 64 && height >= 64 && width == height {
        if is_slim_skin(image) {
            SkinType::NewSlim
        } else {
            SkinType::New
        }
    } else if width == height * 2 {
        SkinType::Old
    } else {
        SkinType::Unknown
    }
}

/// 是否为 1.8 新版纤细皮肤，按右臂与左臂的标记区域是否全透明判定
///
/// - `image`: 皮肤位图
fn is_slim_skin(image: &Pixmap) -> bool {
    let scale = image.width() as i32 / 64;

    // 检查右臂上方的透明像素
    check_pixel_area(image, 50 * scale, 16 * scale, 2 * scale, 4 * scale, &[TRANSPARENT])
            // 检查右臂下方的透明像素
            && check_pixel_area(image, 54 * scale, 20 * scale, 2 * scale, 12 * scale, &[TRANSPARENT])
            // 检查左臂上方的透明像素
            && check_pixel_area(image, 42 * scale, 48 * scale, 2 * scale, 4 * scale, &[TRANSPARENT])
            // 检查左臂下方的透明像素
            && check_pixel_area(image, 46 * scale, 52 * scale, 2 * scale, 12 * scale, &[TRANSPARENT])
}

/// 检查像素区域是否所有像素都匹配指定颜色
///
/// `colors` 里给的是**预乘** RGBA 字节，不透明颜色下两者相同
///
/// - `image`: 皮肤位图
/// - `x`: 区域左上角横坐标
/// - `y`: 区域左上角纵坐标
/// - `w`: 区域宽度
/// - `h`: 区域高度
/// - `colors`: 允许的颜色列表
fn check_pixel_area(image: &Pixmap, x: i32, y: i32, w: i32, h: i32, colors: &[[u8; 4]]) -> bool {
    if x < 0 || y < 0 || x + w > image.width() as i32 || y + h > image.height() as i32 {
        return false;
    }

    for wi in 0..w {
        for hi in 0..h {
            let Some(pixel) = image.pixel((x + wi) as u32, (y + hi) as u32) else {
                return false;
            };
            if !colors.contains(&[pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]) {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
#[path = "../tests/unit/skin_type_checker_tests.rs"]
mod tests;
