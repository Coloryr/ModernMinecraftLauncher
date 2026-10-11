//! 皮肤类型识别与位图读写模块
//!
//! # 子模块
//!
//! | 模块 | 用途 |
//! |------|------|
//! | [`skin_type_checker`] | 根据尺寸与透明区域判定皮肤类型 |

use std::path::Path;

use tiny_skia::Pixmap;

pub mod skin_type_checker;

/// 皮肤类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkinType {
    /// 1.7 旧版皮肤，贴图 64x32
    Old,
    /// 1.8 新版皮肤，贴图 64x64，手臂 4 像素宽
    New,
    /// 1.8 新版纤细皮肤，贴图 64x64，手臂 3 像素宽
    NewSlim,
    /// 尺寸或纤细标记区域不符合任何已知格式
    Unknown,
}

/// 读取图片为位图
///
/// 得到的是预乘 RGBA8、行间无填充的像素缓冲，与旧实现 Skia `RGBA8888` + `Premul`
/// 的内存布局一致——绘制代码一律按裸字节操作。
///
/// - `file`: PNG 文件路径
///
/// # 返回值
///
/// 读取并解码成功返回位图，失败返回 `None`
pub fn open_bitmap(file: &Path) -> Option<Pixmap> {
    let data = std::fs::read(file).ok()?;

    Pixmap::decode_png(&data).ok()
}

/// 把位图写成 PNG
///
/// - `image`: 要写入的位图
/// - `file`: 目标文件路径
pub fn save_bitmap(image: &Pixmap, file: &Path) {
    let data = image.encode_png().expect("PNG 编码失败");
    std::fs::write(file, data).expect("PNG 写入失败");
}

#[cfg(test)]
mod tests {
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
}
