//! 物品渲染：items/*.json 图标定义 → 模型解析 → extrude 挤出/元素烘焙 → GPU 出图
//!
//! 与方块共用烘焙与渲染管线，即 model.rs/block::render_baked，差异在：
//! - items/*.json 的 model 节点为类型分派：model/composite/condition/select/range_dispatch，
//!   special 为实体渲染，箱子/头颅/旗帜等，与 empty 一并跳过；
//!   引用3D模型的物品形态，如 items/stone.json → block/stone，也跳过——只输出2D平面图标，
//!   3D外观由方块表渲染；block表已有的ID，即icons.rs非Skip条目，同样不重复渲染
//! - builtin/generated 链无 elements，按反编译 ItemModelGenerator 挤出：
//!   front/back 全幅面 + 不透明像素边界裙边，gui_light=front
//! - tintindex 索引 items/*.json 的 tint 表，extrude 按层号索引
//! - glint 附魔光效：vanilla 默认注册带 ENCHANTMENT_GLINT_OVERRIDE 的物品
//! - 创造分组取自 icons.rs，与方块表同一套 itemGroup 分类，随渲染写入 Cat 字段

pub mod icons;

use rayon::prelude::*;

mod bake;
mod json;
mod model;
mod render;

pub(crate) use self::bake::{bake_item_model, items_id};
pub(crate) use self::json::*;
pub(crate) use self::model::*;
pub use self::render::{ItemIconOutput, render_item, render_items};

#[allow(unused_imports)]
pub(crate) use mml_base::serialize_tools;

/// 附魔光效贴图
pub(crate) const GLINT_TEX: &str = "assets/minecraft/textures/misc/enchanted_glint_item.png";

/// 默认带附魔光效的物品
pub(crate) const GLINT_IDS: [&str; 2] = ["enchanted_book", "enchanted_golden_apple"];

#[cfg(test)]
#[path = "../../tests/unit/item_mod_tests.rs"]
mod tests;
