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

