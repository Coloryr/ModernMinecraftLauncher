use super::*;
use tiny_skia::{ColorU8, IntSize};

/// 创建一个纯色填充的位图，颜色按预乘 RGBA 字节给出
fn make_bitmap(w: u32, h: u32, color: [u8; 4]) -> Pixmap {
    let mut data = vec![0u8; (w * h * 4) as usize];
    for px in data.chunks_exact_mut(4) {
        px.copy_from_slice(&color);
    }

    Pixmap::from_vec(data, IntSize::from_wh(w, h).unwrap()).unwrap()
}

/// 测试 SkinType 枚举的基本相等性
#[test]
fn test_skin_type_equality() {
    assert_ne!(SkinType::Old, SkinType::New);
    assert_ne!(SkinType::New, SkinType::NewSlim);
    assert_eq!(SkinType::Unknown, SkinType::Unknown);
}

/// 测试 save_bitmap / open_bitmap 的 PNG 存取往返
#[test]
fn test_save_and_open_bitmap_round_trip() {
    // 不透明红：预乘值与原色一致
    let bm = make_bitmap(64, 32, [255, 0, 0, 255]);
    let red = ColorU8::from_rgba(255, 0, 0, 255);

    // 临时目录中的唯一文件名，按进程 ID 区分，测完清理
    let path =
        mml_testutil::temp_dir().join(format!("mml_skin_roundtrip_{}.png", std::process::id()));
    save_bitmap(&bm, &path);

    let loaded = open_bitmap(&path);
    // 无论断言结果如何都清理临时文件
    let result = loaded.map(|b| {
        assert_eq!(b.width(), 64);
        assert_eq!(b.height(), 32);
        // 重新读出的像素颜色应与写入时一致
        assert_eq!(b.pixel(10, 10).unwrap().demultiply(), red);
        assert_eq!(b.pixel(63, 31).unwrap().demultiply(), red);
    });
    let _ = std::fs::remove_file(&path);

    // open_bitmap 应成功返回
    assert!(result.is_some(), "open_bitmap 应能读回保存的 PNG");

    // open_bitmap 对不存在的文件应返回 None
    let missing =
        mml_testutil::temp_dir().join(format!("mml_skin_missing_{}.png", std::process::id()));
    assert!(open_bitmap(&missing).is_none());
}

/// 测试 open_bitmap 对非图片文件返回 None
#[test]
fn test_open_bitmap_invalid_file() {
    let path =
        mml_testutil::temp_dir().join(format!("mml_skin_invalid_{}.txt", std::process::id()));
    std::fs::write(&path, b"this is not an image").unwrap();
    let result = open_bitmap(&path);
    let _ = std::fs::remove_file(&path);
    assert!(result.is_none(), "非图片内容应返回 None");
}

