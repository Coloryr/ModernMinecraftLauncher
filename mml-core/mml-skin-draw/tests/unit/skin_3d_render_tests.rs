use super::*;
use crate::skin_3d_gpu::SkinRenderWgpu;
use glam::Vec2;
use tiny_skia::IntSize;

/// 创建指定位图的辅助函数，RGBA8 纯色填充
fn make_bitmap(w: u32, h: u32) -> Pixmap {
    let mut data = vec![0u8; (w * h * 4) as usize];
    for px in data.chunks_exact_mut(4) {
        px.copy_from_slice(&[0, 255, 0, 255]);
    }

    Pixmap::from_vec(data, IntSize::from_wh(w, h).unwrap()).unwrap()
}

/// 未渲染过的渲染器默认值
#[test]
fn test_default_state() {
    let render = SkinRenderWgpu::new();
    assert_eq!(render.width, 800);
    assert_eq!(render.height, 600);
    assert_eq!(render.distance, 1.0);
    assert_eq!(render.skin_type, SkinType::Unknown);
    assert!(!render.have_skin);
    assert!(!render.have_cape);
    assert_eq!(render.last, Mat4::IDENTITY);
}

/// rotate 按度累进自转矩阵，两次调用等价于角度相加
#[test]
fn test_rotate_accumulates_in_degrees() {
    let mut render = SkinRenderWgpu::new();
    render.rotate(0.0, 45.0);
    render.rotate(0.0, 45.0);

    let want = Mat4::from_rotation_y(90f32.to_radians()).to_cols_array();
    let got = render.last.to_cols_array();
    for (a, b) in got.iter().zip(want.iter()) {
        assert!(
            (a - b).abs() < 1e-5,
            "两次 45 度应等于一次 90 度，实际 {got:?}"
        );
    }
}

/// 装载皮肤：宽度不是 64 的贴图应被拒绝，合法的贴图落进字段
#[test]
fn test_set_skin_tex() {
    let mut render = SkinRenderWgpu::new();

    let bad = make_bitmap(32, 32);
    assert_eq!(render.set_skin_tex(&bad), Err(ErrorType::InvalidSkin));
    assert!(!render.have_skin);
    assert!(render.skin_tex.is_none());

    let good = make_bitmap(64, 64);
    render.set_skin_tex(&good).unwrap();
    assert!(render.have_skin);
    assert_eq!(render.skin_tex.as_ref().unwrap().width(), 64);
}

/// 部件矩阵：头部矩阵在零旋转下等价于平移 (0, VALUE + 1.5*VALUE, 0)
#[test]
fn test_get_matrix_head() {
    let render = SkinRenderWgpu::new();
    let m = render.get_matrix(ModelPartType::Head);
    let w = m.w_axis;
    assert!((w.x - 0.0).abs() < 1e-5);
    assert!((w.y - (skin_3d_cube::VALUE + 1.5 * skin_3d_cube::VALUE)).abs() < 1e-5);
    assert!((w.z - 0.0).abs() < 1e-5);
}

/// 手臂矩阵在零旋转下的落点：肩部枢轴 x = arm_width * VALUE，
/// 其中宽臂 arm_width = 1.5、纤细 = 1.375
#[test]
fn test_get_matrix_arms_width() {
    let mut render = SkinRenderWgpu::new();
    render.skin_type = SkinType::New;
    let wide = render.get_matrix(ModelPartType::LeftArm);
    assert!((wide.w_axis.x - 1.5 * skin_3d_cube::VALUE).abs() < 1e-5);
    assert!((wide.w_axis.y - 0.0).abs() < 1e-5);

    render.skin_type = SkinType::NewSlim;
    let slim = render.get_matrix(ModelPartType::LeftArm);
    assert!((slim.w_axis.x - 1.375 * skin_3d_cube::VALUE).abs() < 1e-5);
}

/// Model 矩阵应包含平移与等比缩放
#[test]
fn test_get_matrix_model() {
    let mut render = SkinRenderWgpu::new();
    render.xy = Vec2::new(0.3, -0.2);
    render.distance = 2.0;
    let m = render.get_matrix(ModelPartType::Model);
    assert!((m.w_axis.x - 0.3).abs() < 1e-5, "平移不应被缩放");
    assert!((m.w_axis.y - -0.2).abs() < 1e-5);
    assert!((m.x_axis.x - 2.0).abs() < 1e-5, "x 基向量应缩放 2 倍");
}

/// Proj / View / Cape 矩阵应全部为有限值
#[test]
fn test_get_matrix_finite() {
    let mut render = SkinRenderWgpu::new();
    render.arm_rotate = Vec3::new(0.0, 20.0, 0.0);
    render.leg_rotate = Vec3::new(10.0, 0.0, 0.0);
    render.head_rotate = Vec3::new(0.0, 0.0, 15.0);
    for part in [
        ModelPartType::Proj,
        ModelPartType::View,
        ModelPartType::Cape,
        ModelPartType::Body,
        ModelPartType::Head,
        ModelPartType::LeftArm,
        ModelPartType::RightArm,
        ModelPartType::LeftLeg,
        ModelPartType::RightLeg,
    ] {
        let m = render.get_matrix(part);
        for v in m.to_cols_array() {
            assert!(v.is_finite(), "{part:?} 矩阵含非有限值 {v}");
        }
    }
}

