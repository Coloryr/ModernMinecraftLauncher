use std::path::Path;

use mml_skin_draw::{
    Draw2DType, Draw3DType, DrawCapeType, DrawElytraType, DrawHeadType, DrawSkinType, draw_cape,
    draw_cape_3d, draw_elytra, draw_elytra_3d, draw_head, draw_skin,
};
use tiny_skia::{Pixmap, Point};

/// 比较两个位图的内容是否完全一致
fn assert_bitmap_eq(a: &Pixmap, b: &Pixmap) {
    assert_eq!(
        a.width(),
        b.width(),
        "bitmap width mismatch: {} != {}",
        a.width(),
        b.width()
    );
    assert_eq!(
        a.height(),
        b.height(),
        "bitmap height mismatch: {} != {}",
        a.height(),
        b.height()
    );
    assert_eq!(
        a.data().len(),
        b.data().len(),
        "bitmap data length mismatch"
    );
    assert_eq!(a.data(), b.data(), "bitmap pixel data mismatch");
}

/// 读取参考图
fn load_reference(name: &str) -> Pixmap {
    let file = Path::new("tests").join(name);
    let out = mml_skin::open_bitmap(file.as_path());
    assert!(out.is_some(), "{name} 应能读取");

    out.unwrap()
}

/// 读取测试用皮肤
fn load_skin(name: &str) -> Pixmap {
    let file = Path::new("tests").join(name);
    let image = mml_skin::open_bitmap(file.as_path());
    assert!(image.is_some(), "{name} 应能读取");

    image.unwrap()
}

/// 披风正面 2D 图应与参考图逐字节一致
#[test]
fn test_cape_draw() {
    let image = load_skin("cape.png");

    let res = draw_cape(&image, DrawCapeType::Front);
    assert!(res.is_some());
    let res = res.unwrap();

    assert_bitmap_eq(&res, &load_reference("out_cape.png"));
}

/// 披风背面 2D 图应与参考图逐字节一致
#[test]
fn test_cape_back_draw() {
    let image = load_skin("cape.png");

    let res = draw_cape(&image, DrawCapeType::Back);
    assert!(res.is_some());
    let res = res.unwrap();

    assert_bitmap_eq(&res, &load_reference("out_cape_back.png"));
}

/// TypeA 2D 头像应与参考图逐字节一致
#[test]
fn test_head_draw_typea() {
    let image = load_skin("skin_slim.png");

    let res = draw_head(&image, DrawHeadType::Type2D(Draw2DType::TypeA));
    assert!(res.is_some());
    let res = res.unwrap();

    assert_bitmap_eq(&res, &load_reference("out_head_a.png"));
}

/// TypeB 2D 头像应与参考图逐字节一致
#[test]
fn test_head_draw_typeb() {
    let image = load_skin("skin_slim.png");

    let res = draw_head(&image, DrawHeadType::Type2D(Draw2DType::TypeB));
    assert!(res.is_some());
    let res = res.unwrap();

    assert_bitmap_eq(&res, &load_reference("out_head_b.png"));
}

/// TypeA 皮肤 2D 展开应与参考图逐字节一致
#[test]
fn test_skin_draw_typea() {
    let image = load_skin("skin_slim.png");

    let res = draw_skin(&image, DrawSkinType::Type2D(Draw2DType::TypeA), None);
    assert!(res.is_some());
    let res = res.unwrap();

    assert_bitmap_eq(&res, &load_reference("out_skin_2d_a.png"));
}

/// TypeB 皮肤 2D 展开应与参考图逐字节一致
#[test]
fn test_skin_draw_typeb() {
    let image = load_skin("skin_slim.png");

    let res = draw_skin(&image, DrawSkinType::Type2D(Draw2DType::TypeB), None);
    assert!(res.is_some());
    let res = res.unwrap();

    assert_bitmap_eq(&res, &load_reference("out_skin_2d_b.png"));
}

/// 面朝下视角的 3D 头像应与参考图逐字节一致
#[test]
fn test_head_3d_draw_typea() {
    let image = load_skin("skin_slim.png");

    let res = draw_head(&image, DrawHeadType::Type3D(Draw3DType::TypeADown));
    assert!(res.is_some());
    let res = res.unwrap();

    assert_bitmap_eq(&res, &load_reference("out_head_3d_a.png"));
}

/// 指定俯仰与偏航角的 3D 头像应与参考图逐字节一致
#[test]
fn test_head_3d_draw_typeb() {
    let image = load_skin("skin_slim.png");

    let res = draw_head(
        &image,
        DrawHeadType::Type3D(Draw3DType::TypeBRotate(Point::from_xy(15.0, 65.0))),
    );
    assert!(res.is_some());
    let res = res.unwrap();

    assert_bitmap_eq(&res, &load_reference("out_head_3d_b.png"));
}

/// 鞘翅收拢（飞行前）的 2D 图应与参考图逐字节一致
#[test]
fn test_elytra_2d_folded() {
    let image = load_skin("cape.png");

    let res = draw_elytra(&image, DrawElytraType::Folded);
    assert!(res.is_some());
    let res = res.unwrap();

    assert_bitmap_eq(&res, &load_reference("out_elytra_2d_folded.png"));
}

/// 鞘翅张开（飞行中）的 2D 图应与参考图逐字节一致
#[test]
fn test_elytra_2d_flying() {
    let image = load_skin("cape.png");

    let res = draw_elytra(&image, DrawElytraType::Flying);
    assert!(res.is_some());
    let res = res.unwrap();

    assert_bitmap_eq(&res, &load_reference("out_elytra_2d_flying.png"));
}

/// 重新生成 3D 参考图
///
/// 3D 输出带抗锯齿，换光栅器后不可能与旧图逐字节一致，所以这两张图由本实现重新生成：
/// `cargo test -p mml-skin-draw --test skin_draw_tests regen_3d_refs -- --ignored`
#[test]
#[ignore]
fn regen_3d_refs() {
    let image = load_skin("skin_slim.png");

    let a =
        draw_head(&image, DrawHeadType::Type3D(Draw3DType::TypeADown)).expect("3d typea 应成功");
    mml_skin::save_bitmap(&a, Path::new("tests").join("out_head_3d_a.png").as_path());

    let b = draw_head(
        &image,
        DrawHeadType::Type3D(Draw3DType::TypeBRotate(Point::from_xy(15.0, 65.0))),
    )
    .expect("3d typeb 应成功");
    mml_skin::save_bitmap(&b, Path::new("tests").join("out_head_3d_b.png").as_path());
}

/// 渲染披风与鞘翅的输出样例，供人眼确认，默认不跑
///
/// 要出图时：
/// `cargo test -p mml-skin-draw --test skin_draw_tests render_samples -- --ignored`
///
/// PNG 写到 `target/temp/skin_samples/`，走 `mml_testutil` 的测试临时目录约定；
/// 3D 那几张需要可用 GPU。2D 出图都有逐字节参考图钉着（见上面的用例），
/// 3D 输出带抗锯齿、换光栅器就不一致，所以只出样不比对。
#[test]
#[ignore]
fn render_samples() {
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
    }
}
