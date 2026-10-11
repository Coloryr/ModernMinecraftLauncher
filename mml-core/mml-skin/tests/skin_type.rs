use std::path::Path;

use mml_skin::{SkinType, skin_type_checker};

/// 64x32 的样本皮肤应判定为 1.7 旧版
#[test]
fn test_skin_old() {
    let file = Path::new("tests").join("skin_old.png");
    let file = file.as_path();
    let skin = mml_skin::open_bitmap(file);
    assert!(skin.is_some());
    assert_eq!(
        skin_type_checker::get_skin_type(&skin.unwrap()),
        SkinType::Old
    );
}

/// 64x64 的样本皮肤应判定为 1.8 新版
#[test]
fn test_skin_new() {
    let file = Path::new("tests").join("skin_new.png");
    let file = file.as_path();
    let skin = mml_skin::open_bitmap(file);
    assert!(skin.is_some());
    assert_eq!(
        skin_type_checker::get_skin_type(&skin.unwrap()),
        SkinType::New
    );
}

/// 64x64 且纤细标记区域透明的样本皮肤应判定为 1.8 新版纤细
#[test]
fn test_skin_slim() {
    let file = Path::new("tests").join("skin_slim.png");
    let file = file.as_path();
    let skin = mml_skin::open_bitmap(file);
    assert!(skin.is_some());
    assert_eq!(
        skin_type_checker::get_skin_type(&skin.unwrap()),
        SkinType::NewSlim
    );
}

/// 尺寸不符合已知格式的样本皮肤应判定为未知
#[test]
fn test_skin_unknown() {
    let file = Path::new("tests").join("skin_unknown.png");
    let file = file.as_path();
    let skin = mml_skin::open_bitmap(file);
    assert!(skin.is_some());
    assert_eq!(
        skin_type_checker::get_skin_type(&skin.unwrap()),
        SkinType::Unknown
    );
}
