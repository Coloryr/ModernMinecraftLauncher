//! 创造模式分类顺序与列表排序键
//!
//! 分类**值**来自 `block/icons.rs`，从游戏 `CreativeModeTabs.class` 字节码提取，
//! 本模块只补「**顺序**」。为什么必须由 crate 自己带：
//!
//! - 方块 / 物品 id 名单是从语言文件键推出来的 `extract_langs`，而语言文件是**字母序**；
//! - 图标表 `BLOCK_ICONS` 按 **id 字母序**排列，`acacia_button` 开头；
//! - 客户端 jar 的语言 / 资源里没有"标签页顺序 + 组内顺序"这种东西——游戏里写在代码里。
//!
//! 组内顺序，即标签页内的物品先后，目前**没有**数据源，用 id 兜底：
//! 顺序稳定、且与界面语言无关，但**不等于游戏内顺序**。
//! 补法见 `TASK.md`：重新提取 `CreativeModeTabs.class`，按 tab 方法里 `Items.X` / `Blocks.X`
//! 的出现顺序得到带序表，届时把 [`order_key`] 中间那一项换成条目序号即可，其它代码不用动。

/// 创造栏标签页顺序，与游戏一致
///
/// 游戏里 `hotbar` / `search` / `inventory` 不是物品分类，前者只是界面页签，**不入表**。
/// `playerSkin` 是启动器自己的分组，即用户添加的皮肤方块，固定排最后。
pub const CAT_ORDER: &[&str] = &[
    "buildingBlocks",
    "coloredBlocks",
    "natural",
    "functional",
    "redstone",
    "tools",
    "combat",
    "foodAndDrink",
    "ingredients",
    "spawnEggs",
    "op",
    "playerSkin",
];

/// 分类的排序序号
///
/// 不在表里的分类，即将来的新分组，排到所有已知分类之后，再按分类名排——顺序仍然稳定。
///
/// - `cat`: 分类名，即 itemGroup lang 键尾段
///
/// # 返回值
///
/// 在 `CAT_ORDER` 里的下标；不在表里时是该表的长度
pub fn cat_index(cat: &str) -> usize {
    CAT_ORDER
        .iter()
        .position(|c| *c == cat)
        .unwrap_or(CAT_ORDER.len())
}

/// 列表排序键
///
/// 派生 `Ord` 逐字段比较，所以**字段顺序就是优先级**：
/// - `rank` 决定分组先后，即游戏创造栏标签页顺序；
/// - `cat` 只对"未登记分类"起作用，保证同一个未知分类的条目聚在一起；
/// - `id` 是组内兜底序：同分类内按 id 字典序，稳定、与语言无关。
///
/// 注意：**不要**改成用翻译后的显示名排序——那样换界面语言顺序就会变。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct OrderKey<'a> {
    /// 分类序号，见 [`cat_index`]
    pub rank: usize,
    /// 分类名
    pub cat: &'a str,
    /// 条目 id
    pub id: &'a str,
}

/// 造一个列表排序键
///
/// - `cat`: 分类名
/// - `id`: 条目 id
///
/// # 返回值
///
/// 见 [`OrderKey`]
pub fn order_key<'a>(cat: &'a str, id: &'a str) -> OrderKey<'a> {
    OrderKey {
        rank: cat_index(cat),
        cat,
        id,
    }
}

#[cfg(test)]
mod tests {
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
}
