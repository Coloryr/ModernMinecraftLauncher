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
mod tests {
    use super::*;
    use tiny_skia::IntSize;

    /// 创建指定尺寸的位图，并对每个像素调用填充函数得到预乘 RGBA 字节
    fn make_bitmap(w: u32, h: u32, fill: impl Fn(i32, i32) -> [u8; 4]) -> Pixmap {
        let mut data = vec![0u8; (w * h * 4) as usize];
        for (i, px) in data.chunks_exact_mut(4).enumerate() {
            let (x, y) = (i as i32 % w as i32, i as i32 / w as i32);
            px.copy_from_slice(&fill(x, y));
        }

        Pixmap::from_vec(data, IntSize::from_wh(w, h).unwrap()).unwrap()
    }

    /// 不透明颜色：预乘值与原色相同
    const OPAQUE_RED: [u8; 4] = [255, 0, 0, 255];
    const OPAQUE_GREEN: [u8; 4] = [0, 255, 0, 255];
    const OPAQUE_BLUE: [u8; 4] = [0, 0, 255, 255];
    const OPAQUE_WHITE: [u8; 4] = [255, 255, 255, 255];

    /// 64x64 全不透明：右臂/左臂标记区域不透明，应识别为新版皮肤
    #[test]
    fn test_get_skin_type_new() {
        let bm = make_bitmap(64, 64, |_, _| OPAQUE_RED);
        assert_eq!(get_skin_type(&bm), SkinType::New);
    }

    /// 64x64 全透明：纤细标记区域全透明，应识别为新版纤细
    #[test]
    fn test_get_skin_type_new_slim() {
        let bm = make_bitmap(64, 64, |_, _| TRANSPARENT);
        assert_eq!(get_skin_type(&bm), SkinType::NewSlim);
    }

    /// 128x128 全透明：放大两倍的新版纤细皮肤
    #[test]
    fn test_get_skin_type_new_slim_scaled() {
        let bm = make_bitmap(128, 128, |_, _| TRANSPARENT);
        assert_eq!(get_skin_type(&bm), SkinType::NewSlim);
    }

    /// 纤细标记区域不透明、其余透明：应识别为新版，非纤细
    #[test]
    fn test_get_skin_type_slim_marker_opaque_is_new() {
        let bm = make_bitmap(64, 64, |x, y| {
            // 纤细判定区域：右臂 (50,16,2,4)、(54,20,2,12)，左臂 (42,48,2,4)、(46,52,2,12)
            let in_marker = (x >= 50 && x < 52 && y >= 16 && y < 20)
                || (x >= 54 && x < 56 && y >= 20 && y < 32)
                || (x >= 42 && x < 44 && y >= 48 && y < 52)
                || (x >= 46 && x < 48 && y >= 52 && y < 64);
            if in_marker { OPAQUE_GREEN } else { TRANSPARENT }
        });
        assert_eq!(get_skin_type(&bm), SkinType::New);
    }

    /// 64x32，宽为高两倍：应识别为 1.7 旧版皮肤
    #[test]
    fn test_get_skin_type_old() {
        let bm = make_bitmap(64, 32, |_, _| OPAQUE_BLUE);
        assert_eq!(get_skin_type(&bm), SkinType::Old);
    }

    /// 尺寸 32x64 不符合任何已知格式，应返回 Unknown
    #[test]
    fn test_get_skin_type_unknown() {
        let bm = make_bitmap(32, 64, |_, _| OPAQUE_BLUE);
        assert_eq!(get_skin_type(&bm), SkinType::Unknown);
    }

    /// 63x63 小于 64 的正方形也不匹配旧版比例，应为 Unknown
    #[test]
    fn test_get_skin_type_unknown_small() {
        let bm = make_bitmap(63, 63, |_, _| OPAQUE_BLUE);
        assert_eq!(get_skin_type(&bm), SkinType::Unknown);
    }

    /// 测试 check_pixel_area 的边界检查：区域越界应返回 false
    #[test]
    fn test_check_pixel_area_out_of_bounds() {
        let bm = make_bitmap(8, 8, |_, _| OPAQUE_WHITE);
        assert!(!check_pixel_area(&bm, 6, 6, 4, 4, &[OPAQUE_WHITE]));
        assert!(!check_pixel_area(&bm, -1, 0, 2, 2, &[OPAQUE_WHITE]));
        assert!(!check_pixel_area(&bm, 0, -1, 2, 2, &[OPAQUE_WHITE]));
        // 区域完全在界内且颜色匹配应返回 true
        assert!(check_pixel_area(&bm, 0, 0, 4, 4, &[OPAQUE_WHITE]));
        // 颜色不匹配应返回 false
        assert!(!check_pixel_area(&bm, 0, 0, 2, 2, &[TRANSPARENT]));
    }
}
