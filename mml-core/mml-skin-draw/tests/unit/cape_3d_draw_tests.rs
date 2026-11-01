use super::*;
use crate::skin_draw::BPP;
use tiny_skia::IntSize;

/// 造一张 64x32 贴图，标记的像素不透明，其余透明
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

/// 统计不透明像素数
fn opaque(image: &Pixmap) -> usize {
    image
        .data()
        .chunks_exact(BPP)
        .filter(|px| px[3] > 0)
        .count()
}

/// 统计不透明像素在 x / y 方向各跨多少列 / 行
fn spans(image: &Pixmap) -> (u32, u32) {
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (SIZE, 0, SIZE, 0);
    for y in 0..SIZE {
        for x in 0..SIZE {
            if image.pixel(x, y).unwrap().alpha() > 0 {
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                max_y = max_y.max(y);
            }
        }
    }

    (max_x - min_x + 1, max_y - min_y + 1)
}

/// 披风 3D：整块布上色时应画满一块，四角透明
#[test]
fn test_cape_3d_draws_cloth() {
    // 正面块 (1,1)，yaw 为 0 时看的正是这一面
    let image = make_texture(|x, y| (1..11).contains(&x) && (1..17).contains(&y));

    let out = cape(&image, 0.0, 0.0).expect("披风 3D 应出图");
    assert_eq!((out.width(), out.height()), (SIZE, SIZE));
    assert!(opaque(&out) > 20_000, "正面看应画出一整块布");

    // 四角留白，说明模型没铺满画布
    for (x, y) in [(0, 0), (SIZE - 1, 0), (0, SIZE - 1), (SIZE - 1, SIZE - 1)] {
        assert_eq!(out.pixel(x, y).unwrap().alpha(), 0, "四角应透明 ({x},{y})");
    }
}

/// 鞘翅 3D：收拢姿态是一片竖长的壳，居中且撑到画布高度
#[test]
fn test_elytra_3d_folded() {
    let image = make_texture(|x, y| (36..46).contains(&x) && (2..22).contains(&y));

    let out = elytra(&image, 0.0, 0.0).expect("鞘翅 3D 应出图");
    assert_eq!((out.width(), out.height()), (SIZE, SIZE));
    assert!(opaque(&out) > 5_000, "两片翼应有可观面积");

    // 收拢后竖长，最长边撑到画布的 FILL
    let (width, height) = spans(&out);
    assert!(height > width, "收拢姿态应竖长，实际 {width} x {height}");
    assert!(
        height as f32 >= SIZE as f32 * FILL - 4.0,
        "最长边应撑到画布的 FILL，实际高 {height}"
    );

    // 居中：中列有像素，左右边缘留白
    let middle = SIZE / 2;
    assert!(
        (0..SIZE).any(|y| out.pixel(middle, y).unwrap().alpha() > 0),
        "正中应画到翼"
    );
    for x in [0, SIZE - 1] {
        assert!(
            (0..SIZE).all(|y| out.pixel(x, y).unwrap().alpha() == 0),
            "边缘 {x} 应留白"
        );
    }
}

/// 角度是入参：换个偏航角结果必须不同
#[test]
fn test_cape_3d_applies_angle() {
    let image = make_texture(|x, y| (1..11).contains(&x) && (1..17).contains(&y));

    let front = cape(&image, 0.0, 0.0).expect("应出图");
    let turned = cape(&image, 0.0, 40.0).expect("应出图");

    assert_ne!(front.data(), turned.data(), "偏航角变化后画面必须变化");
}

/// 两片翼都要画出来：收拢时两片叠在一起，正中那一列也该有像素
#[test]
fn test_elytra_3d_two_wings() {
    let image = make_texture(|x, y| (22..46).contains(&x) && (0..22).contains(&y));

    let out = elytra(&image, 0.0, 0.0).expect("鞘翅 3D 应出图");

    let middle = SIZE / 2;
    let drawn = (0..SIZE)
        .filter(|y| out.pixel(middle, *y).unwrap().alpha() > 0)
        .count();
    assert!(drawn > 20, "正中应有翼，实际不透明 {drawn} 像素");
}

