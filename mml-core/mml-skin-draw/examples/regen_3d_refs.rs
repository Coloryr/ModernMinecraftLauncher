//! 重新生成 3D 参考图：3D 输出带抗锯齿，换光栅器后不可能与旧图逐字节一致，只能由本实现重生成
//!
//! 运行：`cargo run -p mml-skin-draw --example regen_3d_refs`
//! 直接覆盖 `tests/out_head_3d_a.png` 与 `tests/out_head_3d_b.png`，其它用例拿它们做参考图

use std::path::PathBuf;

use mml_skin_draw::{Draw3DType, DrawHeadType, draw_head};
use tiny_skia::{Pixmap, Point};

/// tests 目录，用清单目录拼，不依赖运行时的当前目录
fn tests_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests")
}

/// 读取 tests 下的皮肤文件
fn load_skin(name: &str) -> Pixmap {
    let file = tests_dir().join(name);
    mml_skin::open_bitmap(file.as_path()).expect("皮肤应能读取")
}

fn main() {
    let image = load_skin("skin_slim.png");

    let a =
        draw_head(&image, DrawHeadType::Type3D(Draw3DType::TypeADown)).expect("3d typea 应成功");
    mml_skin::save_bitmap(&a, tests_dir().join("out_head_3d_a.png").as_path());

    let b = draw_head(
        &image,
        DrawHeadType::Type3D(Draw3DType::TypeBRotate(Point::from_xy(15.0, 65.0))),
    )
    .expect("3d typeb 应成功");
    mml_skin::save_bitmap(&b, tests_dir().join("out_head_3d_b.png").as_path());

    println!("已重生成 tests/out_head_3d_a.png 与 tests/out_head_3d_b.png");
}
