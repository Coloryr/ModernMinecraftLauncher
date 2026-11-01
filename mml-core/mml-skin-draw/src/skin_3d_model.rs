//! 史蒂夫模型生成模块
//!
//! 按皮肤类型生成本体模型与第二层顶层模型，皮肤类型分宽臂、纤细与旧版。

use mml_skin::SkinType;

use crate::{
    skin_3d_cube,
    skin_3d_cube_model::{CubeModelItemObj, SteveModel},
};

/// 按皮肤类型生成本体模型
///
/// - `skin_type`: 皮肤类型，决定手臂宽度；纤细模型手臂为 0.375
///
/// # 返回值
///
/// 返回按 Minecraft 标准比例拼装的本体模型，披风也在其中
pub fn get_steve(skin_type: SkinType) -> SteveModel {
    let is_slim = skin_type == SkinType::NewSlim;

    // 公共部件
    let head = CubeModelItemObj::new(
        skin_3d_cube::get_square_default(),
        skin_3d_cube::get_square_indices_default(),
    );

    let body = CubeModelItemObj::new(
        skin_3d_cube::get_square(1.0, 1.5, 0.5, 0.0, 0.0, 0.0, 1.0),
        skin_3d_cube::get_square_indices_default(),
    );

    let left_leg = CubeModelItemObj::new(
        skin_3d_cube::get_square(0.5, 1.5, 0.5, 0.0, 0.0, 0.0, 1.0),
        skin_3d_cube::get_square_indices_default(),
    );

    let right_leg = CubeModelItemObj::new(
        skin_3d_cube::get_square(0.5, 1.5, 0.5, 0.0, 0.0, 0.0, 1.0),
        skin_3d_cube::get_square_indices_default(),
    );

    let cape = CubeModelItemObj::new(
        skin_3d_cube::get_square(1.25, 2.0, 0.1, 0.0, 0.0, 0.0, 1.0),
        skin_3d_cube::get_square_indices_default(),
    );

    // 手臂：按类型决定宽度
    let arm_width = if is_slim { 0.375 } else { 0.5 };
    let arm = CubeModelItemObj::new(
        skin_3d_cube::get_square(arm_width, 1.5, 0.5, 0.0, 0.0, 0.0, 1.0),
        skin_3d_cube::get_square_indices_default(),
    );

    SteveModel::new(head, body, arm.clone(), arm, left_leg, right_leg, cape)
}

/// 按皮肤类型生成第二层模型
///
/// 所有存在的部件均放大 1.125 倍；旧版 1.7 皮肤只有顶层头部。
///
/// - `skin_type`: 皮肤类型
///
/// # 返回值
///
/// 返回顶层模型；旧版皮肤下无数据的部件为空数组
pub fn get_steve_top(skin_type: SkinType) -> SteveModel {
    let is_slim = skin_type == SkinType::NewSlim;
    let is_old = skin_type == SkinType::Old;

    // 头部：总是存在
    let head = CubeModelItemObj::new(
        skin_3d_cube::get_square(1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.125),
        skin_3d_cube::get_square_indices_default(),
    );

    let mut body = CubeModelItemObj::new(Vec::new(), Vec::new());
    let mut left_arm = CubeModelItemObj::new(Vec::new(), Vec::new());
    let mut right_arm = CubeModelItemObj::new(Vec::new(), Vec::new());
    let mut left_leg = CubeModelItemObj::new(Vec::new(), Vec::new());
    let mut right_leg = CubeModelItemObj::new(Vec::new(), Vec::new());
    let cape = CubeModelItemObj::new(Vec::new(), Vec::new());

    if !is_old {
        body = CubeModelItemObj::new(
            skin_3d_cube::get_square(1.0, 1.5, 0.5, 0.0, 0.0, 0.0, 1.125),
            skin_3d_cube::get_square_indices_default(),
        );

        let arm_width = if is_slim { 0.375 } else { 0.5 };
        let arm_model = skin_3d_cube::get_square(arm_width, 1.5, 0.5, 0.0, 0.0, 0.0, 1.125);
        let arm_indices = skin_3d_cube::get_square_indices_default();

        left_arm = CubeModelItemObj::new(arm_model.clone(), arm_indices.clone());
        right_arm = CubeModelItemObj::new(arm_model, arm_indices);

        left_leg = CubeModelItemObj::new(
            skin_3d_cube::get_square(0.5, 1.5, 0.5, 0.0, 0.0, 0.0, 1.125),
            skin_3d_cube::get_square_indices_default(),
        );

        right_leg = CubeModelItemObj::new(
            skin_3d_cube::get_square(0.5, 1.5, 0.5, 0.0, 0.0, 0.0, 1.125),
            skin_3d_cube::get_square_indices_default(),
        );
    }

    SteveModel::new(head, body, left_arm, right_arm, left_leg, right_leg, cape)
}

#[cfg(test)]
#[path = "../tests/unit/skin_3d_model_tests.rs"]
mod tests;
