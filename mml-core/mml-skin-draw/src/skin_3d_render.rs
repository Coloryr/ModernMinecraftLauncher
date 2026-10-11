//! 皮肤 3D 渲染参数模块
//!
//! [`SkinRenderWgpu`] 的参数方法、错误类型、渲染档位与部件矩阵；结构体本身与渲染管线
//! 在 [`crate::skin_3d_gpu`]。
//!
//! 本模块与几个兄弟模块只服务 [`crate::skin_3d_draw`] 的 TypeB 档位，不对外。

use glam::{
    Mat4, Vec3,
    camera::{lh::proj::directx, lh::view},
};
use mml_skin::SkinType;
use std::f32::consts::PI;
use tiny_skia::Pixmap;

use crate::skin_3d_cube;
use crate::skin_3d_gpu::SkinRenderWgpu;

/// 渲染错误类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorType {
    /// 皮肤贴图无效，宽度不是 64
    InvalidSkin,
    /// 没有皮肤，或皮肤类型未知
    NoSkin,
    /// 画布宽度或高度非正
    InvalidSize,
    /// 没有可用的 GPU，或渲染管线建不起来
    NoGpu,
    /// 渲染、读回或位图组装过程出错
    RenderError,
}

/// 模型部件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelPartType {
    /// 头部
    Head,
    /// 身体
    Body,
    /// 左臂
    LeftArm,
    /// 右臂
    RightArm,
    /// 左腿
    LeftLeg,
    /// 右腿
    RightLeg,
    /// 披风
    Cape,
    /// 投影矩阵
    Proj,
    /// 视图矩阵
    View,
    /// 整体模型矩阵
    Model,
}

/// 渲染档位
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkinRenderType {
    /// 普通渲染，单采样
    Normal,
    /// 4x MSAA 多重采样抗锯齿
    MSAA,
}

impl SkinRenderWgpu {
    /// 装载皮肤贴图
    ///
    /// - `skin`: 皮肤贴图，内部克隆一份保存
    ///
    /// # 返回值
    ///
    /// 贴图宽度不是 64 时返回 `Err(ErrorType::InvalidSkin)`
    pub fn set_skin_tex(&mut self, skin: &Pixmap) -> Result<(), ErrorType> {
        if skin.width() != 64 {
            return Err(ErrorType::InvalidSkin);
        }

        self.skin_tex = Some(skin.clone());
        self.have_skin = true;

        Ok(())
    }

    /// 追加模型自转，累进 [`SkinRenderWgpu::last`]，单位度
    ///
    /// - `x`: 绕 x 轴的角度，即俯仰
    /// - `y`: 绕 y 轴的角度，即偏航
    pub fn rotate(&mut self, x: f32, y: f32) {
        self.last = self.last
            * Mat4::from_rotation_x(x.to_radians())
            * Mat4::from_rotation_y(y.to_radians());
    }

    /// 计算指定部件的变换矩阵
    ///
    /// - `part_type`: 部件类型；普通部件取该部件的姿态角度，
    ///   `Proj` / `View` / `Model` 返回对应的投影 / 视图 / 模型矩阵
    ///
    /// # 返回值
    ///
    /// 返回该部件的世界变换矩阵
    pub fn get_matrix(&self, part_type: ModelPartType) -> Mat4 {
        let is_slim = self.skin_type == SkinType::NewSlim;
        let arm_width = if is_slim { 1.375 } else { 1.5 };
        let value = skin_3d_cube::VALUE;

        match part_type {
            ModelPartType::Head => {
                Mat4::from_translation(Vec3::new(0.0, value, 0.0))
                    * Mat4::from_rotation_z(self.head_rotate.x.to_radians())
                    * Mat4::from_rotation_x(self.head_rotate.y.to_radians())
                    * Mat4::from_rotation_y(self.head_rotate.z.to_radians())
                    * Mat4::from_translation(Vec3::new(0.0, value * 1.5, 0.0))
            }
            ModelPartType::LeftArm => {
                Mat4::from_translation(Vec3::new(value / 2.0, -(arm_width * value), 0.0))
                    * Mat4::from_rotation_z(self.arm_rotate.x.to_radians())
                    * Mat4::from_rotation_x(self.arm_rotate.y.to_radians())
                    * Mat4::from_translation(Vec3::new(
                        arm_width * value - value / 2.0,
                        arm_width * value,
                        0.0,
                    ))
            }
            ModelPartType::RightArm => {
                Mat4::from_translation(Vec3::new(-value / 2.0, -(arm_width * value), 0.0))
                    * Mat4::from_rotation_z(-self.arm_rotate.x.to_radians())
                    * Mat4::from_rotation_x(-self.arm_rotate.y.to_radians())
                    * Mat4::from_translation(Vec3::new(
                        -arm_width * value + value / 2.0,
                        arm_width * value,
                        0.0,
                    ))
            }
            ModelPartType::LeftLeg => {
                Mat4::from_translation(Vec3::new(0.0, -1.5 * value, 0.0))
                    * Mat4::from_rotation_z(self.leg_rotate.x.to_radians())
                    * Mat4::from_rotation_x(self.leg_rotate.y.to_radians())
                    * Mat4::from_translation(Vec3::new(value * 0.5, -value * 1.5, 0.0))
            }
            ModelPartType::RightLeg => {
                Mat4::from_translation(Vec3::new(0.0, -1.5 * value, 0.0))
                    * Mat4::from_rotation_z(-self.leg_rotate.x.to_radians())
                    * Mat4::from_rotation_x(-self.leg_rotate.y.to_radians())
                    * Mat4::from_translation(Vec3::new(-value * 0.5, -value * 1.5, 0.0))
            }
            ModelPartType::Proj => {
                let aspect = self.width as f32 / self.height as f32;
                directx::perspective(PI / 4.0, aspect, 0.1, 10.0)
            }
            // 视图必须与上面的 directx 投影同为左手系：物体落在视空间正 z 上，深度才映射进
            // 0..1；混用右手系视图会把物体甩到相机背后，深度整个落在 0..1 之外，什么都画不出来
            ModelPartType::View => view::look_at_mat4(
                Vec3::new(0.0, 0.0, 7.0),
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ),
            ModelPartType::Model => {
                let translation = Mat4::from_translation(Vec3::new(self.xy.x, self.xy.y, 0.0));
                let scale =
                    Mat4::from_scale(Vec3::new(self.distance, self.distance, self.distance));
                self.last * translation * scale
            }
            ModelPartType::Cape => {
                // 披风取固定的垂落角度，不随姿态变化
                let cape_rot = 6.3;
                Mat4::from_translation(Vec3::new(0.0, -2.0 * value, -value * 0.1))
                    * Mat4::from_rotation_x(cape_rot * PI / 180.0)
                    * Mat4::from_translation(Vec3::new(0.0, 1.6 * value, -value * 0.5))
            }
            ModelPartType::Body => Mat4::default(),
        }
    }
}

#[cfg(test)]
mod tests {
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
            assert!((a - b).abs() < 1e-5, "两次 45 度应等于一次 90 度，实际 {got:?}");
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
}
