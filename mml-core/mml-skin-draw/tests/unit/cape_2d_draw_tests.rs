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

