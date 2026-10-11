//! 模型贴图 UV 生成模块
//!
//! 内置各部件在皮肤贴图上的 UV 布局，按皮肤类型区分，新版 64x64、旧版 64x32，
//! 偏移并归一化后生成模型各部件的贴图坐标。

use mml_skin::SkinType;

use crate::cube_model::SteveTexture;

/// 头部 UV 布局，单位皮肤像素，每面 8x8
const HEAD_TEX: [f32; 48] = [
    // 背面
    32.0, 8.0, 32.0, 16.0, 24.0, 16.0, 24.0, 8.0, // 前面
    8.0, 8.0, 8.0, 16.0, 16.0, 16.0, 16.0, 8.0, // 左面
    0.0, 8.0, 0.0, 16.0, 8.0, 16.0, 8.0, 8.0, // 右面
    16.0, 8.0, 16.0, 16.0, 24.0, 16.0, 24.0, 8.0, // 顶面
    8.0, 0.0, 8.0, 8.0, 16.0, 8.0, 16.0, 0.0, // 底面
    24.0, 0.0, 24.0, 8.0, 16.0, 8.0, 16.0, 0.0,
];

/// 手臂 / 腿部 UV 布局，单位皮肤像素，每面 4x12
const LEG_ARM_TEX: [f32; 48] = [
    // 背面
    12.0, 4.0, 12.0, 16.0, 16.0, 16.0, 16.0, 4.0, // 前面
    4.0, 4.0, 4.0, 16.0, 8.0, 16.0, 8.0, 4.0, // 左面
    0.0, 4.0, 0.0, 16.0, 4.0, 16.0, 4.0, 4.0, // 右面
    8.0, 4.0, 8.0, 16.0, 12.0, 16.0, 12.0, 4.0, // 顶面
    4.0, 0.0, 4.0, 4.0, 8.0, 4.0, 8.0, 0.0, // 底面
    12.0, 0.0, 12.0, 4.0, 8.0, 4.0, 8.0, 0.0,
];

/// 纤细手臂 UV 布局，单位皮肤像素，每面 3x12
const SLIM_ARM_TEX: [f32; 48] = [
    // 背面
    11.0, 4.0, 11.0, 16.0, 14.0, 16.0, 14.0, 4.0, // 前面
    4.0, 4.0, 4.0, 16.0, 7.0, 16.0, 7.0, 4.0, // 左面
    0.0, 4.0, 0.0, 16.0, 4.0, 16.0, 4.0, 4.0, // 右面
    7.0, 4.0, 7.0, 16.0, 10.0, 16.0, 10.0, 4.0, // 顶面
    4.0, 0.0, 4.0, 4.0, 7.0, 4.0, 7.0, 0.0, // 底面
    10.0, 0.0, 10.0, 4.0, 7.0, 4.0, 7.0, 0.0,
];

/// 身体 UV 布局，单位皮肤像素，每面 8x12
const BODY_TEX: [f32; 48] = [
    // 背面
    24.0, 4.0, 24.0, 16.0, 16.0, 16.0, 16.0, 4.0, // 前面
    4.0, 4.0, 4.0, 16.0, 12.0, 16.0, 12.0, 4.0, // 左面
    0.0, 4.0, 0.0, 16.0, 4.0, 16.0, 4.0, 4.0, // 右面
    12.0, 4.0, 12.0, 16.0, 16.0, 16.0, 16.0, 4.0, // 顶面
    4.0, 0.0, 4.0, 4.0, 12.0, 4.0, 12.0, 0.0, // 底面
    20.0, 0.0, 20.0, 4.0, 12.0, 4.0, 12.0, 0.0,
];

/// 披风 UV 布局
const CAPE_TEX: [f32; 48] = [
    // 背面
    11.0, 1.0, 11.0, 17.0, 1.0, 17.0, 1.0, 1.0, // 前面
    12.0, 1.0, 12.0, 17.0, 22.0, 17.0, 22.0, 1.0, // 左面
    11.0, 1.0, 11.0, 17.0, 12.0, 17.0, 12.0, 1.0, // 右面
    0.0, 1.0, 0.0, 17.0, 1.0, 17.0, 1.0, 1.0, // 顶面
    1.0, 0.0, 1.0, 1.0, 11.0, 1.0, 11.0, 0.0, // 底面
    21.0, 0.0, 21.0, 1.0, 11.0, 1.0, 11.0, 0.0,
];

/// 按皮肤类型偏移并归一化一组 UV 坐标
///
/// - `input`: 部件的原始 UV 布局数据，单位皮肤像素
/// - `skin_type`: 皮肤类型；旧版按 64x32 归一化，新版按 64x64 归一化
/// - `offset_u`: U 方向偏移，单位皮肤像素
/// - `offset_v`: V 方向偏移，单位皮肤像素
///
/// # 返回值
///
/// 返回偏移并归一化到 0..1 的 UV 坐标数组
pub fn get_tex(input: &[f32], skin_type: SkinType, offset_u: f32, offset_v: f32) -> Vec<f32> {
    let mut temp = vec![0.0; input.len()];

    for (a, value) in temp.iter_mut().enumerate() {
        if a % 2 == 0 {
            *value = input[a] + offset_u;
        } else {
            *value = input[a] + offset_v;
        }

        if a % 2 != 0 && skin_type == SkinType::Old {
            *value /= 32.0;
        } else {
            *value /= 64.0;
        }
    }

    temp
}

/// 归一化披风的 UV 坐标
///
/// - `input`: 披风的原始 UV 布局数据，单位皮肤像素
///
/// # 返回值
///
/// 返回归一化到 0..1 的 UV 坐标数组，u 除以 64、v 除以 32
pub fn get_cap_tex(input: &[f32]) -> Vec<f32> {
    let mut temp = vec![0.0; input.len()];

    for (a, value) in temp.iter_mut().enumerate() {
        *value = input[a];
        if a % 2 == 0 {
            *value /= 64.0;
        } else {
            *value /= 32.0;
        }
    }

    temp
}

/// 生成顶层各部件的 UV 数据
///
/// - `skin_type`: 皮肤类型；旧版皮肤只有顶层头部
///
/// # 返回值
///
/// 返回顶层各部件的 UV 数据；无数据的部件为空数组，顶层无披风
pub fn get_steve_texture_top(skin_type: SkinType) -> SteveTexture {
    let mut tex = SteveTexture::new();
    tex.head = get_tex(&HEAD_TEX, skin_type, 32.0, 0.0);

    if skin_type != SkinType::Old {
        tex.body = get_tex(&BODY_TEX, skin_type, 16.0, 32.0);

        let arm_tex = if skin_type == SkinType::NewSlim {
            &SLIM_ARM_TEX
        } else {
            &LEG_ARM_TEX
        };
        tex.left_arm = get_tex(arm_tex, skin_type, 48.0, 48.0);
        tex.right_arm = get_tex(arm_tex, skin_type, 40.0, 32.0);
        tex.left_leg = get_tex(&LEG_ARM_TEX, skin_type, 0.0, 48.0);
        tex.right_leg = get_tex(&LEG_ARM_TEX, skin_type, 0.0, 32.0);
    }

    tex
}

/// 生成本体各部件的 UV 数据
///
/// - `skin_type`: 皮肤类型，决定手臂贴图布局与归一化方式
///
/// # 返回值
///
/// 返回本体各部件的 UV 数据，含披风
pub fn get_steve_texture(skin_type: SkinType) -> SteveTexture {
    let mut tex = SteveTexture::new();
    tex.head = get_tex(&HEAD_TEX, skin_type, 0.0, 0.0);
    tex.body = get_tex(&BODY_TEX, skin_type, 16.0, 16.0);
    tex.cape = get_cap_tex(&CAPE_TEX);

    if skin_type == SkinType::Old {
        tex.left_arm = get_tex(&LEG_ARM_TEX, skin_type, 40.0, 16.0);
        tex.right_arm = get_tex(&LEG_ARM_TEX, skin_type, 40.0, 16.0);
        tex.left_leg = get_tex(&LEG_ARM_TEX, skin_type, 0.0, 16.0);
        tex.right_leg = get_tex(&LEG_ARM_TEX, skin_type, 0.0, 16.0);
    } else {
        let arm_tex = if skin_type == SkinType::NewSlim {
            &SLIM_ARM_TEX
        } else {
            &LEG_ARM_TEX
        };
        tex.left_arm = get_tex(arm_tex, skin_type, 32.0, 48.0);
        tex.right_arm = get_tex(arm_tex, skin_type, 40.0, 16.0);
        tex.left_leg = get_tex(&LEG_ARM_TEX, skin_type, 0.0, 16.0);
        tex.right_leg = get_tex(&LEG_ARM_TEX, skin_type, 16.0, 48.0);
    }

    tex
}

#[cfg(test)]
mod tests {
    use super::*;

    /// get_tex 对新版 64x64 皮肤应把 UV 坐标偏移后除以 64 归一化
    #[test]
    fn test_get_tex_new() {
        let uv = get_tex(&[8.0, 16.0], SkinType::New, 4.0, 2.0);
        assert_eq!(uv.len(), 2);
        assert_eq!(uv[0], (8.0 + 4.0) / 64.0);
        assert_eq!(uv[1], (16.0 + 2.0) / 64.0);
    }

    /// get_tex 对旧版 64x32 皮肤的归一化：u 恒除以 64，v 除以 32
    #[test]
    fn test_get_tex_old() {
        let uv = get_tex(&[8.0, 16.0], SkinType::Old, 0.0, 0.0);
        assert_eq!(uv[0], 8.0 / 64.0);
        assert_eq!(uv[1], 16.0 / 32.0);
    }

    /// 旧版皮肤带偏移的归一化：u 仍除以 64，坐标落在 0..1 内
    #[test]
    fn test_get_tex_old_with_offset() {
        // 旧版右臂 UV：LEG_ARM_TEX 的 u=12 加偏移 40 -> (12+40)/64 = 0.8125
        let uv = get_tex(&LEG_ARM_TEX, SkinType::Old, 40.0, 16.0);
        assert_eq!(uv[0], (12.0 + 40.0) / 64.0);
        assert!(uv[0] <= 1.0, "旧版手臂 UV 应在 0..1 内：{}", uv[0]);
    }

    /// 新版纤细手臂应使用 SLIM_ARM_TEX，3 像素宽；新版普通手臂 4 像素宽
    #[test]
    fn test_get_steve_texture_arm_variants() {
        let new = get_steve_texture(SkinType::New);
        let slim = get_steve_texture(SkinType::NewSlim);

        // 新版普通右臂：LEG_ARM_TEX + (40, 16)
        assert_eq!(new.right_arm[0], (12.0 + 40.0) / 64.0);
        // 新版纤细右臂：SLIM_ARM_TEX + (40, 16)，SLIM_ARM_TEX 的 u=11
        assert_eq!(slim.right_arm[0], (11.0 + 40.0) / 64.0);

        // 两者的左臂纹理不同：LEG_ARM_TEX u=12、SLIM_ARM_TEX u=11，且偏移不同
        assert_ne!(new.left_arm, slim.left_arm);

        // 每个部件都应有完整的 24 个顶点 x 2 的 UV 数据
        for part in [
            &new.head,
            &new.body,
            &new.left_arm,
            &new.right_arm,
            &new.left_leg,
            &new.right_leg,
        ] {
            assert_eq!(part.len(), 48);
        }
    }

    /// 本体纹理应包含披风 UV，顶层纹理不应包含
    #[test]
    fn test_get_steve_texture_cape() {
        let tex = get_steve_texture(SkinType::New);
        assert_eq!(tex.cape.len(), 48, "本体纹理应有披风 UV");

        let top = get_steve_texture_top(SkinType::New);
        assert!(top.cape.is_empty(), "顶层纹理不应有披风 UV");
        // 顶层身体 UV：BODY_TEX u=24 加偏移 16 -> (24+16)/64
        assert_eq!(top.body[0], (24.0 + 16.0) / 64.0);
        assert_eq!(top.body[1], (4.0 + 32.0) / 64.0);
    }

    /// get_cap_tex：u 坐标除以 64，v 坐标除以 32
    #[test]
    fn test_get_cap_tex() {
        let uv = get_cap_tex(&[32.0, 16.0]);
        assert_eq!(uv[0], 32.0 / 64.0);
        assert_eq!(uv[1], 16.0 / 32.0);

        // 完整披风 UV 应全部落在 0..1 内
        let cape = get_cap_tex(&CAPE_TEX);
        for v in &cape {
            assert!(*v >= 0.0 && *v <= 1.0, "披风 UV 越界: {v}");
        }
    }
}
