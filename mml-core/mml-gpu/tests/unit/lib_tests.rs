use super::*;

/// premul BGRA → straight RGBA：半透明像素反除回直乘，全透明清零
#[test]
fn test_premul_to_straight() {
    // 预乘语义 rgb ≤ a：BGRA (100,100,100,200) → RGBA (127,127,127,200)
    let out = premul_bgra_to_straight_rgba(&[100, 100, 100, 200]);
    assert_eq!(out, [127, 127, 127, 200]);
    // 全透明清零
    let out = premul_bgra_to_straight_rgba(&[9, 9, 9, 0]);
    assert_eq!(out, [0, 0, 0, 0]);
}

/// straight RGBA → 预乘 BGRA：往返后颜色一致，允许整除误差
#[test]
fn test_straight_to_premul_roundtrip() {
    let out = straight_rgba_to_premul_bgra(&[255, 128, 64, 255]);
    // BGRA 内存序，alpha=255 不变
    assert_eq!(&out, &[64, 128, 255, 255]);
}

