use super::*;

/// 校验 CAT_ORDER 的标签页先后与游戏一致，并确认皮肤分组固定最后、未登记分类排到表尾
#[test]
fn cat_order_follows_game_tabs() {
    // 游戏创造栏的标签页先后，首项与末项都能确认即可
    assert_eq!(cat_index("buildingBlocks"), 0);
    assert!(cat_index("buildingBlocks") < cat_index("coloredBlocks"));
    assert!(cat_index("coloredBlocks") < cat_index("natural"));
    assert!(cat_index("tools") < cat_index("combat"));
    assert!(cat_index("combat") < cat_index("foodAndDrink"));
    assert!(cat_index("foodAndDrink") < cat_index("ingredients"));
    assert!(cat_index("ingredients") < cat_index("spawnEggs"));
    assert!(cat_index("spawnEggs") < cat_index("op"));
    // 皮肤分组是启动器自己的，固定最后
    assert_eq!(cat_index("playerSkin"), CAT_ORDER.len() - 1);
    // 未登记分类排到最后
    assert_eq!(cat_index("somethingNew"), CAT_ORDER.len());
}

/// 校验 order_key 只按分类序号与条目 id 排序，换界面语言或显示名都不会改变结果
#[test]
fn order_key_is_language_independent() {
    // 同分类内按 id 排，与显示名和语言无关
    let mut v = vec![
        ("buildingBlocks", "minecraft:stone"),
        ("buildingBlocks", "minecraft:oak_planks"),
        ("natural", "minecraft:stone"),
    ];
    v.sort_by(|a, b| order_key(a.0, a.1).cmp(&order_key(b.0, b.1)));
    assert_eq!(
        v,
        vec![
            ("buildingBlocks", "minecraft:oak_planks"),
            ("buildingBlocks", "minecraft:stone"),
            ("natural", "minecraft:stone"),
        ]
    );
    // 未知分类聚在最后，并在其后按分类名排
    assert!(cat_index("zzz") > cat_index("playerSkin"));
    let key = order_key("aaa", "x");
    assert_eq!(key.rank, CAT_ORDER.len());
    assert_eq!(key.cat, "aaa");
    assert_eq!(key.id, "x");
}

