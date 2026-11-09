//! 渲染披风与鞘翅的样图，供人眼确认
//!
//! 运行：`cargo run -p mml-skin-draw --example render_samples`
//! PNG 写到 `target/temp/skin_samples/`；3D 那几张需要可用 GPU

use std::path::PathBuf;

use mml_skin_draw::{draw_cape_3d, draw_elytra_3d};
use tiny_skia::Pixmap;

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
    let dir = mml_testutil::temp_path("skin_samples");
    std::fs::create_dir_all(&dir).unwrap();
    let image = load_skin("cape.png");

    let samples: [(&str, Option<Pixmap>); 4] = [
        ("cape_3d_yaw0.png", draw_cape_3d(&image, 0.0, 0.0)),
        ("cape_3d_yaw30.png", draw_cape_3d(&image, 25.0, 45.0)),
        ("elytra_3d_yaw0.png", draw_elytra_3d(&image, 0.0, 0.0)),
        ("elytra_3d_yaw30.png", draw_elytra_3d(&image, 0.0, 30.0)),
    ];

    for (name, bitmap) in samples {
        let Some(bitmap) = bitmap else {
            panic!("{name} 应出图")
        };
        mml_skin::save_bitmap(&bitmap, dir.join(name).as_path());
        println!("输出：{}", dir.join(name).display());
    }
}
