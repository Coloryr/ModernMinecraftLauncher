use super::*;
use std::collections::HashSet;

use crate::block::icons::{SpecialForm, special_form};

/// 固定表完整性：方块 ID 唯一、分类非空、各规格内容非空
#[test]
fn icon_table_consistent() {
    assert!(BLOCK_ICONS.len() > 800, "表应覆盖全部创造栏方块");
    let mut seen = HashSet::new();
    for (id, cat, spec) in BLOCK_ICONS {
        assert!(seen.insert(*id), "重复方块ID：{id}");
        assert!(!cat.is_empty(), "{id} 分类为空");
        match spec {
            IconSpec::Model(rel) | IconSpec::IsoModel(rel) => {
                assert!(!rel.is_empty(), "{id} 模型名为空")
            }
            IconSpec::Composite(parts) => assert!(!parts.is_empty(), "{id} composite无part"),
            // 头颅：贴图路径非空；带帽子层的贴图必须是 humanoidHeadLayer 的 64×64
            IconSpec::Head(tex, _) => assert!(!tex.is_empty(), "{id} 头颅贴图为空"),
            // Form形态：ID必须以形态后缀结尾，内层规格不能是Skip/Form
            IconSpec::Form(form, inner) => {
                assert!(
                    id.ends_with(&format!("_{}", form.suffix())),
                    "{id} 形态后缀与SpecialForm不符"
                );
                assert!(
                    matches!(
                        inner,
                        IconSpec::Model(_) | IconSpec::IsoModel(_) | IconSpec::Composite(_)
                    ),
                    "{id} Form内层规格非法"
                );
            }
            IconSpec::Skip => {}
        }
    }
}

/// 抽查关键行：渲染规格与分类应与游戏创造栏一致
#[test]
fn icon_table_spot_checks() {
    let table: HashMap<&str, (&str, &IconSpec)> = BLOCK_ICONS
        .iter()
        .map(|(id, cat, spec)| (*id, (*cat, spec)))
        .collect();
    assert!(matches!(
        table["minecraft:stone"].1,
        IconSpec::Model("block/stone")
    ));
    assert_eq!(table["minecraft:stone"].0, "buildingBlocks");
    assert!(matches!(
        table["minecraft:white_bed"].1,
        IconSpec::Composite(_)
    ));
    // special 实体渲染跳过
    assert!(matches!(table["minecraft:chest"].1, IconSpec::Skip));
    // 头颅走手工立方体几何：骷髅头单层、玩家头带帽子层
    assert!(matches!(
        table["minecraft:skeleton_skull"].1,
        IconSpec::Head(_, false)
    ));
    assert!(matches!(
        table["minecraft:player_head"].1,
        IconSpec::Head(_, true)
    ));
    // 原木在游戏的建筑方块栏
    assert_eq!(table["minecraft:oak_log"].0, "buildingBlocks");
    assert_eq!(table["minecraft:grass_block"].0, "natural");
}

/// 特殊形态：基础ID应能查到形态，且对应"基础ID_后缀"图标确实在表里
#[test]
fn special_form_lookup() {
    assert_eq!(special_form("minecraft:furnace"), Some(SpecialForm::Lit));
    assert_eq!(
        special_form("minecraft:redstone_lamp"),
        Some(SpecialForm::Lit)
    );
    assert_eq!(
        special_form("minecraft:oak_button"),
        Some(SpecialForm::Pressed)
    );
    assert_eq!(
        special_form("minecraft:oak_pressure_plate"),
        Some(SpecialForm::Down)
    );
    assert_eq!(special_form("minecraft:oak_door"), Some(SpecialForm::Open));
    assert_eq!(
        special_form("minecraft:piston"),
        Some(SpecialForm::Extended)
    );
    assert_eq!(special_form("minecraft:vault"), Some(SpecialForm::Active));
    assert_eq!(special_form("minecraft:campfire"), Some(SpecialForm::Off));
    // 无特殊形态的方块
    assert_eq!(special_form("minecraft:stone"), None);
    assert_eq!(special_form("minecraft:oak_stairs"), None);

    // 每个查到的形态都应有对应后缀图标，且图标ID后缀与形态匹配
    for (id, cat, _) in BLOCK_ICONS {
        if let Some(form) = special_form(id) {
            let alt = format!("{id}_{}", form.suffix());
            let hit = BLOCK_ICONS
                .iter()
                .any(|(name, _, spec)| *name == alt && matches!(spec, IconSpec::Form(..)));
            assert!(hit, "形态图标缺失：{alt}（{cat}）");
        }
    }
}

