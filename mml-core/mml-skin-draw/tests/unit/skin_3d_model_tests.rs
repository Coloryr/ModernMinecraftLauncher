use super::*;

/// 计算模型顶点在某个轴上的最大绝对值
fn max_abs_axis(model: &[f32], axis: usize) -> f32 {
    model
        .iter()
        .skip(axis)
        .step_by(3)
        .fold(0.0f32, |m, v| m.max(v.abs()))
}

/// 史蒂夫模型的部件尺寸应与 Minecraft 标准模型一致，
/// 以 VALUE = 0.5 为单位：头 1x1x1、身体 1x1.5x0.5、手臂 0.5x1.5x0.5
#[test]
fn test_get_steve_dimensions() {
    let m = get_steve(SkinType::New);

    // 头部：1 x 1 x 1
    assert_eq!(max_abs_axis(&m.head.model, 0), skin_3d_cube::VALUE);
    assert_eq!(max_abs_axis(&m.head.model, 1), skin_3d_cube::VALUE);
    assert_eq!(max_abs_axis(&m.head.model, 2), skin_3d_cube::VALUE);

    // 身体：1 x 1.5 x 0.5
    assert_eq!(max_abs_axis(&m.body.model, 0), skin_3d_cube::VALUE);
    assert_eq!(max_abs_axis(&m.body.model, 1), 1.5 * skin_3d_cube::VALUE);
    assert_eq!(max_abs_axis(&m.body.model, 2), 0.5 * skin_3d_cube::VALUE);

    // 手臂：0.5 x 1.5 x 0.5，宽臂
    assert_eq!(
        max_abs_axis(&m.left_arm.model, 0),
        0.5 * skin_3d_cube::VALUE
    );
    assert_eq!(
        max_abs_axis(&m.left_arm.model, 1),
        1.5 * skin_3d_cube::VALUE
    );
    assert_eq!(
        max_abs_axis(&m.right_arm.model, 0),
        0.5 * skin_3d_cube::VALUE
    );

    // 腿：0.5 x 1.5 x 0.5
    assert_eq!(
        max_abs_axis(&m.left_leg.model, 0),
        0.5 * skin_3d_cube::VALUE
    );
    assert_eq!(
        max_abs_axis(&m.left_leg.model, 1),
        1.5 * skin_3d_cube::VALUE
    );

    // 披风：1.25 x 2 x 0.1
    assert_eq!(max_abs_axis(&m.cape.model, 0), 1.25 * skin_3d_cube::VALUE);
    assert_eq!(max_abs_axis(&m.cape.model, 1), 2.0 * skin_3d_cube::VALUE);
    assert_eq!(max_abs_axis(&m.cape.model, 2), 0.1 * skin_3d_cube::VALUE);

    // 左右手臂使用同一份模型数据
    assert_eq!(m.left_arm.model, m.right_arm.model);
    assert_eq!(m.left_arm.point, m.right_arm.point);
}

/// 纤细模型手臂宽度应为 0.375，其余部件不变
#[test]
fn test_get_steve_slim_arm() {
    let slim = get_steve(SkinType::NewSlim);
    assert_eq!(
        max_abs_axis(&slim.left_arm.model, 0),
        0.375 * skin_3d_cube::VALUE
    );
    assert_eq!(
        max_abs_axis(&slim.left_arm.model, 1),
        1.5 * skin_3d_cube::VALUE
    );

    // slim 与宽臂的身体、头部相同
    let normal = get_steve(SkinType::New);
    assert_eq!(slim.body.model, normal.body.model);
    assert_eq!(slim.head.model, normal.head.model);
}

/// 第二层即顶层模型应放大 1.125 倍
#[test]
fn test_get_steve_top_enlarged() {
    let top = get_steve_top(SkinType::New);

    // 头部：1.125 倍 -> ±0.5625
    assert_eq!(
        max_abs_axis(&top.head.model, 0),
        1.125 * skin_3d_cube::VALUE
    );
    assert_eq!(
        max_abs_axis(&top.head.model, 1),
        1.125 * skin_3d_cube::VALUE
    );

    // 身体：1 x 1.5 x 0.5，放大 1.125
    assert_eq!(
        max_abs_axis(&top.body.model, 1),
        1.5 * 1.125 * skin_3d_cube::VALUE
    );
    assert_eq!(
        max_abs_axis(&top.body.model, 2),
        0.5 * 1.125 * skin_3d_cube::VALUE
    );

    // 顶层没有披风数据
    assert!(top.cape.model.is_empty());
    assert!(top.cape.point.is_empty());
}

/// 旧版 1.7 皮肤没有顶层身体部件，只有顶层头部
#[test]
fn test_get_steve_top_old_only_head() {
    let top = get_steve_top(SkinType::Old);
    assert!(!top.head.model.is_empty(), "旧版仍有顶层头部");
    assert!(top.body.model.is_empty(), "旧版没有顶层身体");
    assert!(top.left_arm.model.is_empty(), "旧版没有顶层手臂");
    assert!(top.right_arm.model.is_empty());
    assert!(top.left_leg.model.is_empty());
    assert!(top.right_leg.model.is_empty());
}

