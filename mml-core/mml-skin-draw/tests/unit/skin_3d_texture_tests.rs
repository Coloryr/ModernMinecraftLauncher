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

