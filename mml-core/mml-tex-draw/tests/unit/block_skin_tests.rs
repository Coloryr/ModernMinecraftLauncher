use super::*;

/// 名字校验：合法字符放行，空名/路径注入/超长拒绝
#[test]
fn skin_id_validation() {
    assert_eq!(skin_id("steve").unwrap(), "custom:steve");
    assert!(skin_id("A-b_9").is_ok());
    assert!(skin_id(&"x".repeat(64)).is_ok());

    assert!(skin_id("").is_err());
    assert!(skin_id("../evil").is_err());
    assert!(skin_id("a b").is_err());
    assert!(skin_id("minecraft:stone").is_err());
    assert!(skin_id(&"x".repeat(65)).is_err());
}

/// 完整流程：添加出图并注册 → 同名覆盖 → 注销并删文件
///
/// 走临时目录与真实 GPU 渲染，且 init 路径是进程级 OnceLock，单独跑：
/// cargo test -p mml-tex-draw --lib skin::tests::add_remove_roundtrip -- --ignored
#[test]
#[ignore]
fn add_remove_roundtrip() {
    let root = crate::unique_temp_dir("skin");
    crate::init(&root).unwrap();

    let skin_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../mml-skin-draw/tests/skin_slim.png");
    let skin = std::fs::read(&skin_path).unwrap();

    // 添加：出图 + 注册进独立分组
    let id = add_skin_block("test_skin", &skin).unwrap();
    assert_eq!(id, "custom:test_skin");
    let png = crate::get_block_path(&id).unwrap();
    assert!(png.exists(), "图标未写盘：{}", png.display());
    assert_eq!(crate::block_cat(&id).unwrap(), SKIN_CAT);
    assert!(crate::blocks().iter().any(|b| b == &id), "未注册进方块列表");

    // 同名覆盖：不报错，图标仍在
    add_skin_block("test_skin", &skin).unwrap();
    assert!(crate::get_block_path(&id).unwrap().exists());

    // 删除：注销 + 删文件；再删报不存在
    remove_skin_block("test_skin").unwrap();
    assert!(crate::get_block_path(&id).is_none());
    assert!(!png.exists(), "图标文件未删除");
    assert!(remove_skin_block("test_skin").is_err());

    let _ = std::fs::remove_dir_all(&root);
}

