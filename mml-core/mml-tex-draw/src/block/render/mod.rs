//! 方块图标渲染
//!
//! PNG/APNG 编解码、贴图动画、皮肤头与图标渲染，以及渲染管线。

pub mod anim;
pub mod head;
pub mod icon;
pub mod pipeline;
pub mod png_codec;

pub(super) use self::icon::*;
pub(super) use self::png_codec::*;

pub use self::anim::{AnimFrame, AnimMeta, read_anim_meta};
pub(crate) use self::anim::{AnimTimeline, mml_tex_draw_id};
pub use self::head::{bake_head, bake_head_model};
pub use self::icon::use_first_frame;
pub(crate) use self::icon::{fit_composite, render_baked};
pub use self::pipeline::{TextureRefObj, render_blocks};
pub use self::png_codec::decode_png;
pub(crate) use self::png_codec::{ApngFrame, encode_apng, to_straight_rgba};
pub(super) use super::{BLOCK_SIZE, FRAME_RATE};
