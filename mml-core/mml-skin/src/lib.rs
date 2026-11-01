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
#[path = "../tests/unit/lib_tests.rs"]
mod tests;
