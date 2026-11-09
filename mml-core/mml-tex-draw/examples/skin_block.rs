//! 皮肤方块图标的手动验证：添加出图并注册 → 同名覆盖 → 注销并删文件
//!
//! 运行：`cargo run -p mml-tex-draw --example skin_block`
//! 用独立的临时目录，跑完删掉，不碰真实运行目录

use std::path::PathBuf;

use mml_tex_draw::{
    SKIN_CAT, add_skin_block, block_cat, blocks, get_block_path, remove_skin_block,
};

fn main() {
    let root = mml_testutil::temp_dir().join(format!("mml-tex-draw-skin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    mml_tex_draw::init(&root).unwrap();

    let skin_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../mml-skin-draw/tests/skin_slim.png");
    let skin = std::fs::read(&skin_path).expect("皮肤文件应存在");

    // 添加：出图 + 注册进独立分组
    let id = add_skin_block("test_skin", &skin).expect("添加皮肤方块失败");
    assert_eq!(id, "custom:test_skin");
    let png = get_block_path(&id).expect("图标应已写盘");
    assert!(png.exists(), "图标未写盘：{}", png.display());
    assert_eq!(block_cat(&id).as_deref(), Some(SKIN_CAT));
    assert!(blocks().iter().any(|b| b == &id), "未注册进方块列表");

    // 同名覆盖：不报错，图标仍在
    add_skin_block("test_skin", &skin).expect("同名覆盖应成功");
    assert!(get_block_path(&id).unwrap().exists());

    // 删除：注销 + 删文件；再删报不存在
    remove_skin_block("test_skin").expect("注销失败");
    assert!(get_block_path(&id).is_none());
    assert!(!png.exists(), "图标文件未删除");
    assert!(remove_skin_block("test_skin").is_err());

    let _ = std::fs::remove_dir_all(&root);
    println!("皮肤方块增删流程通过");
}
