//! 方块渲染：图标表、数据、渲染管线
//!
//! # 子模块
//!
//! | 模块 | 用途 |
//! |------|------|
//! | [`icons`] | 创造模式图标表：方块 ID、分类、渲染规格 |
//! | [`obj`] | 渲染结果数据结构 BlocksObj 与 ItemsObj |
//! | [`skin`] | 皮肤方块注册 |
pub mod icons;
pub mod obj;
pub mod skin;

pub mod render;

/// 输出图片边长，单位像素
pub(crate) const BLOCK_SIZE: i32 = 256;

/// 游戏帧率，单位刻每秒；APNG 帧延迟按 刻数 / FRAME_RATE 换算成秒
pub(crate) const FRAME_RATE: u16 = 20;

/// 平原群系染色色，与 wiki 物品图标一致：草与树叶等灰度贴图按 tintindex 取它
pub(crate) const GRASS_TINT: [u8; 3] = [145, 189, 89]; // #91BD59
/// 树叶染色色，取平原群系色
pub(crate) const FOLIAGE_TINT: [u8; 3] = [119, 171, 47]; // #77AB2F
/// 白桦树叶染色色，游戏内不随群系变化
pub(crate) const BIRCH_TINT: [u8; 3] = [128, 167, 85]; // #80A755
/// 云杉树叶染色色，游戏内不随群系变化
pub(crate) const SPRUCE_TINT: [u8; 3] = [97, 153, 97]; // #619961

pub use self::render::*;
pub(crate) use self::render::{fit_composite, mml_tex_draw_id, render_baked, to_straight_rgba};

#[cfg(test)]
pub(crate) use self::render::{AnimTimeline, ApngFrame, encode_apng};

#[cfg(test)]
#[path = "../../tests/unit/block_mod_icon_table_tests.rs"]
mod icon_table_tests;

#[cfg(test)]
#[path = "../../tests/unit/block_mod_sprite_tests.rs"]
mod sprite_tests;
