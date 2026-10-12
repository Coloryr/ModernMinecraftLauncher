//! 皮肤/披风 2D 与 3D 头像渲染模块
//!
//! # 子模块
//!
//! | 模块 | 用途 |
//! |------|------|
//! | [`skin_draw`] | 像素级绘制基元：复制 / 混合 / 缩放 / 填充 |
//! | [`skin_2d_draw`] | 皮肤整体 2D 展开，TypeA / TypeB |
//! | [`skin_3d_draw`] | 皮肤整体 3D 渲染，TypeA / TypeA下 / TypeB |
//! | [`head_2d_draw`] | 2D 头像渲染 |
//! | [`head_3d_draw`] | 3D 头像渲染 |
//! | [`cape_2d_draw`] | 披风 / 鞘翅 2D 渲染 |
//! | [`cape_3d_draw`] | 披风 / 鞘翅 3D 等距渲染 |
//!
//! 另有 `skin_3d_cube` / `skin_3d_cube_model` / `skin_3d_gpu` / `skin_3d_model` /
//! `skin_3d_render` / `skin_3d_texture` 一组内部模块，是 TypeB 档位用的透视渲染引擎。

mod cape_2d_draw;
mod cape_3d_draw;
mod gpu_3d;
mod head_2d_draw;
mod head_3d_draw;
mod skin_2d_draw;
mod skin_3d_cube;
mod skin_3d_cube_model;
mod skin_3d_draw;
mod skin_3d_gpu;
mod skin_3d_model;
mod skin_3d_render;
mod skin_3d_texture;
mod skin_draw;

use tiny_skia::{Pixmap, Point};

/// 2D渲染模式
pub enum Draw2DType {
    TypeA,
    TypeB,
}

pub enum Draw3DType {
    TypeAUp,
    TypeADown,
    TypeBRotate(Point),
}

/// 渲染模式
pub enum DrawHeadType {
    /// 渲染2D
    Type2D(Draw2DType),
    /// 渲染3D
    Type3D(Draw3DType),
}

pub enum DrawSkinType {
    /// 渲染2D
    Type2D(Draw2DType),
    /// 渲染3D
    Type3D(Draw3DType),
}

pub enum DrawCapeType {
    /// 披风正面
    Front,
    /// 披风背面
    Back,
}

/// 鞘翅的两种姿态
pub enum DrawElytraType {
    /// 飞行前，两片翼收拢在背后
    Folded,
    /// 飞行中，两片翼张开
    Flying,
}

/// 渲染头像
///
/// - `image`: 皮肤贴图
/// - `mode`: 渲染模式
///
/// # 返回值
///
/// 返回渲染后的位图，贴图尺寸异常时返回 `None`
pub fn draw_head(image: &Pixmap, mode: DrawHeadType) -> Option<Pixmap> {
    match mode {
        DrawHeadType::Type2D(draw2d_type) => match draw2d_type {
            Draw2DType::TypeA => head_2d_draw::typea(image),
            Draw2DType::TypeB => head_2d_draw::typeb(image),
        },
        DrawHeadType::Type3D(draw3d_type) => match draw3d_type {
            Draw3DType::TypeAUp => head_3d_draw::typea_up(image),
            Draw3DType::TypeADown => head_3d_draw::typea_down(image),
            Draw3DType::TypeBRotate(point) => head_3d_draw::typeb(image, point.x, point.y),
        },
    }
}

/// 渲染皮肤
///
/// - `image`: 皮肤贴图
/// - `mode`: 渲染模式
/// - `skin_type`: 角色皮肤模型
///
/// # 返回值
///
/// 返回渲染后的位图，贴图尺寸异常时返回 `None`
pub fn draw_skin(
    image: &Pixmap,
    mode: DrawSkinType,
    skin_type: Option<mml_skin::SkinType>,
) -> Option<Pixmap> {
    match mode {
        DrawSkinType::Type2D(draw2d_type) => match draw2d_type {
            Draw2DType::TypeA => skin_2d_draw::typea(image, skin_type),
            Draw2DType::TypeB => skin_2d_draw::typeb(image, skin_type),
        },
        DrawSkinType::Type3D(draw3d_type) => match draw3d_type {
            Draw3DType::TypeAUp => skin_3d_draw::typea_up(image, skin_type),
            Draw3DType::TypeADown => skin_3d_draw::typea_down(image, skin_type),
            Draw3DType::TypeBRotate(point) => {
                skin_3d_draw::typeb(image, skin_type, point.x, point.y)
            }
        },
    }
}

pub fn draw_cape(image: &Pixmap, mode: DrawCapeType) -> Option<Pixmap> {
    match mode {
        DrawCapeType::Front => cape_2d_draw::front(image),
        DrawCapeType::Back => cape_2d_draw::back(image),
    }
}

/// 渲染鞘翅的 2D 平铺图
///
/// 与 [`draw_cape`] 取同一张 64x32 的盒展开贴图：鞘翅的翼盒 10x20x2 在 (22,0)，
/// 只有一片翼，另一片由镜像拼出。内侧那块贴图一般是空的，所以不按正 / 背面分，
/// 而是按游戏里的两种姿态出图。
///
/// - `image`: 鞘翅贴图
/// - `mode`: `Folded` 收拢即飞行前，两片翼竖着并排；`Flying` 张开即飞行中，
///   两片翼横着摊开并留出肩点间隔
///
/// # 返回值
///
/// 返回渲染后的位图，贴图尺寸异常时返回 `None`
pub fn draw_elytra(image: &Pixmap, mode: DrawElytraType) -> Option<Pixmap> {
    match mode {
        DrawElytraType::Folded => cape_2d_draw::elytra_folded(image),
        DrawElytraType::Flying => cape_2d_draw::elytra_flying(image),
    }
}

/// 渲染披风本体的 3D 等距图
///
/// - `image`: 披风贴图
/// - `pitch`: 俯仰角，单位度，负 = 面朝上，正 = 面朝下
/// - `yaw`: 偏航角，单位度，0 为正对
///
/// # 返回值
///
/// 返回 256 见方的透明底位图，无可用 GPU 时返回 `None`
pub fn draw_cape_3d(image: &Pixmap, pitch: f32, yaw: f32) -> Option<Pixmap> {
    cape_3d_draw::cape(image, pitch, yaw)
}

/// 渲染鞘翅本体的 3D 等距图
///
/// 两片翼按收拢姿态摆放，角度与 [`draw_cape_3d`] 同口径。
///
/// - `image`: 鞘翅贴图
/// - `pitch`: 俯仰角，单位度
/// - `yaw`: 偏航角，单位度
///
/// # 返回值
///
/// 返回 256 见方的透明底位图，无可用 GPU 时返回 `None`
pub fn draw_elytra_3d(image: &Pixmap, pitch: f32, yaw: f32) -> Option<Pixmap> {
    cape_3d_draw::elytra(image, pitch, yaw)
}
