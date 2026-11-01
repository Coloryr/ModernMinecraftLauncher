use crate::skin_draw::*;
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

