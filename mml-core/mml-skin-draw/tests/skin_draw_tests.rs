use std::path::Path;

use mml_skin_draw::{cape_2d_draw, head_2d_draw, head_3d_draw, skin_2d_draw, skin_3d_draw};
use tiny_skia::Pixmap;

/// 比较两个位图的内容是否完全一致
fn assert_bitmap_eq(a: &Pixmap, b: &Pixmap) {
    assert_eq!(a.width(), b.width(), "bitmap width mismatch: {} != {}", a.width(), b.width());
    assert_eq!(
        a.height(),
        b.height(),
        "bitmap height mismatch: {} != {}",
        a.height(),
        b.height()
    );
    assert_eq!(a.data().len(), b.data().len(), "bitmap data length mismatch");
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

    let res = cape_2d_draw::draw_cape_2d(&image);
    assert!(res.is_some());
    let res = res.unwrap();

    assert_bitmap_eq(&res, &load_reference("out_cape.png"));
}

/// 披风背面 2D 图应与参考图逐字节一致
#[test]
fn test_cape_back_draw() {
    let image = load_skin("cape.png");

    let res = cape_2d_draw::draw_cape_back_2d(&image);
    assert!(res.is_some());
    let res = res.unwrap();

    assert_bitmap_eq(&res, &load_reference("out_cape_back.png"));
}

/// TypeA 2D 头像应与参考图逐字节一致
#[test]
fn test_head_draw_typea() {
    let image = load_skin("skin_slim.png");

    let res = head_2d_draw::head_2d_draw_typea(&image);
    assert!(res.is_some());
    let res = res.unwrap();

    assert_bitmap_eq(&res, &load_reference("out_head_a.png"));
}

/// TypeB 2D 头像应与参考图逐字节一致
#[test]
fn test_head_draw_typeb() {
    let image = load_skin("skin_slim.png");

    let res = head_2d_draw::head_2d_draw_typeb(&image);
    assert!(res.is_some());
    let res = res.unwrap();

    assert_bitmap_eq(&res, &load_reference("out_head_b.png"));
}

/// TypeA 皮肤 2D 展开应与参考图逐字节一致
#[test]
fn test_skin_draw_typea() {
    let image = load_skin("skin_slim.png");

    let res = skin_2d_draw::skin_2d_draw_typea(&image, None);
    assert!(res.is_some());
    let res = res.unwrap();

    assert_bitmap_eq(&res, &load_reference("out_skin_2d_a.png"));
}

/// TypeB 皮肤 2D 展开应与参考图逐字节一致
#[test]
fn test_skin_draw_typeb() {
    let image = load_skin("skin_slim.png");

    let res = skin_2d_draw::skin_2d_draw_typeb(&image, None);
    assert!(res.is_some());
    let res = res.unwrap();

    assert_bitmap_eq(&res, &load_reference("out_skin_2d_b.png"));
}

/// 面朝下视角的 3D 头像应与参考图逐字节一致
#[test]
fn test_head_3d_draw_typea() {
    let image = load_skin("skin_slim.png");

    let res = head_3d_draw::draw_head_3d_typea_down(&image);
    assert!(res.is_some());
    let res = res.unwrap();

    assert_bitmap_eq(&res, &load_reference("out_head_3d_a.png"));
}

/// 指定俯仰与偏航角的 3D 头像应与参考图逐字节一致
#[test]
fn test_head_3d_draw_typeb() {
    let image = load_skin("skin_slim.png");

    let res = head_3d_draw::draw_head_3d_typeb(&image, 15.0, 65.0);
    assert!(res.is_some());
    let res = res.unwrap();

    assert_bitmap_eq(&res, &load_reference("out_head_3d_b.png"));
}

/// 重新生成 3D 参考图
///
/// 3D 输出带抗锯齿，换光栅器后不可能与旧图逐字节一致，所以这两张图由本实现重新生成：
/// `cargo test -p mml-skin-draw --test skin_draw_tests regen_3d_refs -- --ignored`
#[test]
#[ignore]
fn regen_3d_refs() {
    let image = load_skin("skin_slim.png");

    let a = head_3d_draw::draw_head_3d_typea_down(&image).expect("3d typea 应成功");
    mml_skin::save_bitmap(&a, Path::new("tests").join("out_head_3d_a.png").as_path());

    let b = head_3d_draw::draw_head_3d_typeb(&image, 15.0, 65.0).expect("3d typeb 应成功");
    mml_skin::save_bitmap(&b, Path::new("tests").join("out_head_3d_b.png").as_path());
}
