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
                // 左手系投影叠上「画布 y 向下」，画面会整体左右镜像；把投影第一行取负抵消，
                // 管线侧 front_face 因此取 Ccw
                let aspect = self.width as f32 / self.height as f32;
                let mut proj = directx::perspective(PI / 4.0, aspect, 0.1, 10.0);
                // 透视矩阵第一行只有 x 项，取负它就等于把裁剪空间的 x 取负
                proj.x_axis.x = -proj.x_axis.x;
                proj
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
#[path = "../tests/unit/skin_3d_render_tests.rs"]
mod tests;
