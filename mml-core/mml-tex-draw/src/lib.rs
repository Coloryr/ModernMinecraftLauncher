//! 方块 / 物品贴图离线渲染模块
//!
//! 从 Minecraft 客户端 jar 提取模型与贴图，离线渲染出创造模式物品栏
//! 使用的方块图标与物品图标 PNG，前者为等轴测 3D、后者为平面精灵，
//! 并提供图标路径、创造分组、语言翻译等查询接口。
//!
//! # 子模块
//!
//! | 模块 | 用途 |
//! |------|------|
//! | [`block`] | 方块图标渲染，等轴测 3D |
//! | [`gpu`] | GPU 渲染支持 |
//! | [`item`] | 物品图标渲染，平面精灵 |
//! | [`model`] | 游戏模型解析 |
//! | [`order`] | 创造分类顺序与列表排序键 |

#[cfg(test)]
use std::path::PathBuf;

pub mod block;
pub mod gpu;
pub mod item;
pub mod model;
pub mod order;

mod lang;
mod load;
mod query;
mod state;

pub use self::lang::get_lang;
pub use self::load::{init, load, load_blocks, save};
pub use self::query::{
    Entry, EntryKind, block_2d_faces, block_cat, block_name_key, block_special_form, block_version,
    blocks, get_block_path, get_block_path_2d, get_block_path_2d_face, get_block_path_form,
    get_item_path, has_block_2d, has_rendered, item_cat, item_name_key, item_version, items,
    ordered_entries,
};
pub use self::state::{cancel, is_cancelled, is_loading, set_gui_handel};

pub use self::block::icons::SpecialForm;
pub use self::block::skin::{SKIN_CAT, add_skin_block, remove_skin_block};

pub(crate) use self::query::{
    blocks_read, blocks_write, extract_langs, get_block_dir, get_block2d_dir, get_item_dir,
    items_read, items_write,
};
pub(crate) use self::state::cancel_token;

#[cfg(test)]
fn unique_temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    mml_testutil::temp_dir().join(format!(
        "mml-tex-draw-test-{}-{}-{}",
        tag,
        std::process::id(),
        nanos
    ))
}

#[cfg(test)]
#[path = "../tests/unit/lib_tests.rs"]
mod tests;
