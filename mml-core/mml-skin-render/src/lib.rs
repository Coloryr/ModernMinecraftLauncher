//! 皮肤 3D 渲染模块
//!
//! 提供渲染器公共状态与交互逻辑基类 [`BaseSkinRender`]，涵盖指针交互、
//! 部件矩阵计算、动画与 FPS 统计，由 [`renders`] 中的具体渲染后端
//! 如 OpenGL 组合实现。
//!
//! # 子模块
//!
//! | 模块 | 用途 |
//! |------|------|
//! | [`cube`] | 立方体几何基元 |
//! | [`cube_model`] | 立方体模型数据结构 |
//! | [`model`] | 史蒂夫模型生成 |
//! | [`renders`] | 渲染后端实现，目前是 OpenGL |
//! | [`skin_animation`] | 部件动画演算 |
//! | [`texture`] | 模型贴图 UV 生成 |

pub mod cube;
pub mod cube_model;
pub mod model;
pub mod renders;
pub mod skin_animation;
pub mod texture;

use glam::{
    Mat4, Vec2, Vec3, Vec4,
    camera::{lh::proj::directx, rh::view},
};
use mml_skin::SkinType;
use std::f32::consts::PI;
use std::sync::{Arc, Mutex};
use tiny_skia::Pixmap;

use crate::skin_animation::SkinAnimation;

/// 渲染错误类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorType {
    /// 皮肤贴图无效，宽度不是 64 或格式错误
    InvalidSkin,
    /// 无法识别的皮肤类型
    UnknownSkin,
    /// 渲染过程出错
    RenderError,
    /// 贴图处理出错
    TextureError,
}

/// 渲染器状态类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateType {
    /// 已初始化
    Initialized,
    /// 皮肤贴图已加载
    SkinLoaded,
    /// 披风贴图已加载
    CapeLoaded,
    /// 渲染已开始
    RenderStarted,
    /// 渲染已完成
    RenderCompleted,
    /// 已释放资源
    Disposed,
}

/// 鼠标按键类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyType {
    /// 左键，拖拽旋转模型
    Left,
    /// 右键，拖拽平移模型
    Right,
    /// 中键
    Middle,
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

/// 抽象皮肤渲染器基类
///
/// 持有渲染器公共状态，涵盖交互输入、动画、回调与画布尺寸；
/// 具体渲染后端持有此结构并根据 `switch_*` 标志位刷新资源。
pub struct BaseSkinRender {
    /// 是否渲染披风
    pub enable_cape: bool,
    /// 是否渲染第二层，即顶层
    pub enable_top: bool,
    /// 模型已变化，需要重新加载模型数据
    pub switch_model: bool,
    /// 贴图已变化，需要重新上传贴图
    pub switch_skin: bool,
    /// 渲染类型已变化
    pub switch_type: bool,
    /// 背景色已变化
    pub switch_back: bool,
    /// 是否播放动画
    pub animation: bool,

    /// 背景色，RGBA 四个分量，取值范围 0.0..1.0
    pub back_color: Vec4,
    /// 皮肤类型，决定模型与贴图布局
    pub skin_type: SkinType,

    /// 皮肤贴图
    pub skin_tex: Option<Pixmap>,
    /// 披风贴图
    pub cape: Option<Pixmap>,

    /// FPS 统计累计时间，单位秒
    pub time: f64,
    /// 当前 FPS 计数
    pub fps: i32,

    /// 模型视距，即缩放距离，越大越远
    pub distance: f32,
    /// 待应用的旋转增量，单位度，`tick` 时累积到变换矩阵
    pub rot_xy: Vec2,
    /// 左键拖拽的基准点
    pub diff_xy: Vec2,
    /// 模型平移位置
    pub xy: Vec2,
    /// 右键松开时保存的平移位置
    pub save_xy: Vec2,
    /// 右键按下的起点
    pub last_xy: Vec2,

    /// 用户累积旋转的变换矩阵
    pub last: Mat4,

    /// 行走动画
    pub skin_animation: SkinAnimation,

    /// 是否已加载披风
    pub have_cape: bool,
    /// 是否已加载皮肤
    pub have_skin: bool,

    /// 手臂旋转角度，单位度
    pub arm_rotate: Vec3,
    /// 腿部旋转角度，单位度
    pub leg_rotate: Vec3,
    /// 头部旋转角度，单位度
    pub head_rotate: Vec3,

    /// 错误回调
    pub error_callback: Option<Arc<Mutex<dyn Fn(ErrorType) + Send + Sync>>>,
    /// 状态变更回调
    pub state_callback: Option<Arc<Mutex<dyn Fn(StateType) + Send + Sync>>>,
    /// FPS 更新回调
    pub fps_callback: Option<Arc<Mutex<dyn Fn(i32) + Send + Sync>>>,

    /// 画布宽度，单位像素
    pub width: i32,
    /// 画布高度，单位像素
    pub height: i32,
}

impl BaseSkinRender {
    /// 创建渲染器基类，默认画布 800x600、黑色背景
    pub fn new() -> Self {
        Self {
            enable_cape: false,
            enable_top: false,
            switch_model: false,
            switch_skin: false,
            switch_type: false,
            switch_back: false,
            animation: false,
            back_color: Vec4::new(0.0, 0.0, 0.0, 1.0),
            skin_type: SkinType::Unknown,
            skin_tex: None,
            cape: None,
            time: 0.0,
            fps: 0,
            distance: 1.0,
            rot_xy: Vec2::new(0.0, 0.0),
            diff_xy: Vec2::new(0.0, 0.0),
            xy: Vec2::new(0.0, 0.0),
            save_xy: Vec2::new(0.0, 0.0),
            last_xy: Vec2::new(0.0, 0.0),
            last: Mat4::default(),
            skin_animation: SkinAnimation::new(),
            have_cape: false,
            have_skin: false,
            arm_rotate: Vec3::new(0.0, 0.0, 0.0),
            leg_rotate: Vec3::new(0.0, 0.0, 0.0),
            head_rotate: Vec3::new(0.0, 0.0, 0.0),
            error_callback: None,
            state_callback: None,
            fps_callback: None,
            width: 800,
            height: 600,
        }
    }

    /// 指针按下：记录拖拽基准点
    ///
    /// - `key_type`: 按下的按键类型
    /// - `point`: 指针位置
    pub fn pointer_pressed(&mut self, key_type: KeyType, point: Vec2) {
        match key_type {
            KeyType::Left => {
                self.diff_xy.x = point.x;
                self.diff_xy.y = -point.y;
            }
            KeyType::Right => {
                self.last_xy.x = point.x;
                self.last_xy.y = point.y;
            }
            _ => {}
        }
    }

    /// 指针松开：右键松开时保存当前平移位置
    ///
    /// - `key_type`: 松开的按键类型
    /// - `_point`: 指针位置，未使用
    pub fn pointer_released(&mut self, key_type: KeyType, _point: Vec2) {
        if let KeyType::Right = key_type {
            self.save_xy.x = self.xy.x;
            self.save_xy.y = self.xy.y;
        }
    }

    /// 指针移动：左键拖拽计算旋转增量，右键拖拽计算平移位置
    ///
    /// - `key_type`: 按住的按键类型
    /// - `point`: 指针位置
    pub fn pointer_moved(&mut self, key_type: KeyType, point: Vec2) {
        match key_type {
            KeyType::Left => {
                self.rot_xy.y = point.x - self.diff_xy.x;
                self.rot_xy.x = point.y + self.diff_xy.y;
                self.rot_xy.y *= 2.0;
                self.rot_xy.x *= 2.0;
                self.diff_xy.x = point.x;
                self.diff_xy.y = -point.y;
            }
            KeyType::Right => {
                self.xy.x = -(self.last_xy.x - point.x) / 100.0 + self.save_xy.x;
                self.xy.y = (self.last_xy.y - point.y) / 100.0 + self.save_xy.y;
            }
            _ => {}
        }
    }

    /// 滚轮滚动：调整模型视距
    ///
    /// - `is_post`: `true` 表示向前滚动即拉远，`false` 表示向后滚动即拉近
    pub fn pointer_wheel_changed(&mut self, is_post: bool) {
        if is_post {
            self.distance += 0.1;
        } else {
            self.distance -= 0.1;
        }
    }

    /// 追加模型旋转增量，单位度，`tick` 时累积到变换矩阵
    ///
    /// - `x`: x 轴增量
    /// - `y`: y 轴增量
    pub fn rotate(&mut self, x: f32, y: f32) {
        self.rot_xy.x += x;
        self.rot_xy.y += y;
    }

    /// 追加模型平移偏移
    ///
    /// - `x`: x 轴偏移
    /// - `y`: y 轴偏移
    pub fn position(&mut self, x: f32, y: f32) {
        self.xy.x += x;
        self.xy.y += y;
    }

    /// 追加视距增量
    ///
    /// - `x`: 视距增量
    pub fn add_distance(&mut self, x: f32) {
        self.distance += x;
    }

    /// 设置皮肤贴图
    ///
    /// - `skin`: 皮肤贴图，传 `None` 表示清除皮肤
    ///
    /// # 返回值
    ///
    /// 贴图宽度不是 64 时返回 `Err(ErrorType::InvalidSkin)`，其余情况返回 `Ok(())`
    pub fn set_skin_tex(&mut self, skin: Option<Pixmap>) -> Result<(), ErrorType> {
        if let Some(skin_tex) = skin {
            if skin_tex.width() != 64 {
                return Err(ErrorType::InvalidSkin);
            }

            self.skin_tex = Some(skin_tex.clone());
            self.switch_skin = true;
            self.have_skin = true;

            self.on_state_change(StateType::SkinLoaded);
            Ok(())
        } else {
            self.have_skin = false;
            Ok(())
        }
    }

    /// 设置披风贴图
    ///
    /// - `cape`: 披风贴图，传 `None` 表示清除披风
    ///
    /// # 返回值
    ///
    /// 总是返回 `Ok(())`
    pub fn set_cape_tex(&mut self, cape: Option<Pixmap>) -> Result<(), ErrorType> {
        if let Some(cape_tex) = cape {
            self.cape = Some(cape_tex);
            self.switch_skin = true;
            self.have_cape = true;

            self.on_state_change(StateType::CapeLoaded);
            Ok(())
        } else {
            self.have_cape = false;
            Ok(())
        }
    }

    /// 重置位置状态，视距、平移与旋转矩阵恢复默认值
    pub fn reset_position(&mut self) {
        self.distance = 1.0;
        self.diff_xy = Vec2::new(0.0, 0.0);
        self.xy = Vec2::new(0.0, 0.0);
        self.save_xy = Vec2::new(0.0, 0.0);
        self.last_xy = Vec2::new(0.0, 0.0);
        self.last = Mat4::default();
    }

    /// 帧驱动：推进动画、把旋转增量累积到变换矩阵、统计 FPS
    ///
    /// - `time`: 距上帧的时间，单位秒
    pub fn tick(&mut self, time: f64) {
        if self.animation {
            self.skin_animation.tick(time);

            if self.animation {
                self.head_rotate = self.skin_animation.head;
                self.arm_rotate = self.skin_animation.arm;
                self.leg_rotate = self.skin_animation.leg;
            }
        }

        if self.rot_xy.x != 0.0 || self.rot_xy.y != 0.0 {
            let rot_x = Mat4::from_rotation_x(self.rot_xy.x / 360.0);
            let rot_y = Mat4::from_rotation_y(self.rot_xy.y / 360.0);
            self.last = self.last * rot_x * rot_y;
            self.rot_xy = Vec2::new(0.0, 0.0);
        }

        self.fps += 1;
        self.time += time;

        if self.time >= 1.0 {
            self.time -= 1.0;
            self.on_fps_update(self.fps);
            self.fps = 0;
        }
    }

    /// 触发错误回调，未设置回调时无操作
    ///
    /// - `error`: 错误类型
    pub fn on_error(&self, error: ErrorType) {
        if let Some(callback) = &self.error_callback {
            if let Ok(cb) = callback.lock() {
                cb(error);
            }
        }
    }

    /// 触发状态变更回调，未设置回调时无操作
    ///
    /// - `state`: 新状态
    pub fn on_state_change(&self, state: StateType) {
        if let Some(callback) = &self.state_callback {
            if let Ok(cb) = callback.lock() {
                cb(state);
            }
        }
    }

    /// 触发 FPS 更新回调，未设置回调时无操作
    ///
    /// - `fps`: 当前 FPS
    pub fn on_fps_update(&self, fps: i32) {
        if let Some(callback) = &self.fps_callback {
            if let Ok(cb) = callback.lock() {
                cb(fps);
            }
        }
    }

    /// 计算指定部件的变换矩阵
    ///
    /// - `part_type`: 部件类型；普通部件取动画或手动旋转角度，
    ///   `Proj` / `View` / `Model` 返回对应的投影 / 视图 / 模型矩阵
    ///
    /// # 返回值
    ///
    /// 返回该部件的世界变换矩阵
    pub fn get_matrix(&self, part_type: ModelPartType) -> Mat4 {
        let enable = self.animation;
        let is_slim = self.skin_type == SkinType::NewSlim;
        let arm_width = if is_slim { 1.375 } else { 1.5 };

        match part_type {
            ModelPartType::Head => {
                let head_rot = if enable {
                    self.skin_animation.head
                } else {
                    self.head_rotate
                };
                Mat4::from_translation(Vec3::new(0.0, cube::VALUE, 0.0))
                    * Mat4::from_rotation_z(head_rot.x / 360.0)
                    * Mat4::from_rotation_x(head_rot.y / 360.0)
                    * Mat4::from_rotation_y(head_rot.z / 360.0)
                    * Mat4::from_translation(Vec3::new(0.0, cube::VALUE * 1.5, 0.0))
            }
            ModelPartType::LeftArm => {
                let arm_rot = if enable {
                    self.skin_animation.arm
                } else {
                    self.arm_rotate
                };
                Mat4::from_translation(Vec3::new(
                    cube::VALUE / 2.0,
                    -(arm_width * cube::VALUE),
                    0.0,
                )) * Mat4::from_rotation_z(arm_rot.x / 360.0)
                    * Mat4::from_rotation_x(arm_rot.y / 360.0)
                    * Mat4::from_translation(Vec3::new(
                        arm_width * cube::VALUE - cube::VALUE / 2.0,
                        arm_width * cube::VALUE,
                        0.0,
                    ))
            }
            ModelPartType::RightArm => {
                let arm_rot = if enable {
                    self.skin_animation.arm
                } else {
                    self.arm_rotate
                };
                Mat4::from_translation(Vec3::new(
                    -cube::VALUE / 2.0,
                    -(arm_width * cube::VALUE),
                    0.0,
                )) * Mat4::from_rotation_z(-arm_rot.x / 360.0)
                    * Mat4::from_rotation_x(-arm_rot.y / 360.0)
                    * Mat4::from_translation(Vec3::new(
                        -arm_width * cube::VALUE + cube::VALUE / 2.0,
                        arm_width * cube::VALUE,
                        0.0,
                    ))
            }
            ModelPartType::LeftLeg => {
                let leg_rot = if enable {
                    self.skin_animation.leg
                } else {
                    self.leg_rotate
                };
                Mat4::from_translation(Vec3::new(0.0, -1.5 * cube::VALUE, 0.0))
                    * Mat4::from_rotation_z(leg_rot.x / 360.0)
                    * Mat4::from_rotation_x(leg_rot.y / 360.0)
                    * Mat4::from_translation(Vec3::new(cube::VALUE * 0.5, -cube::VALUE * 1.5, 0.0))
            }
            ModelPartType::RightLeg => {
                let leg_rot = if enable {
                    self.skin_animation.leg
                } else {
                    self.leg_rotate
                };
                Mat4::from_translation(Vec3::new(0.0, -1.5 * cube::VALUE, 0.0))
                    * Mat4::from_rotation_z(-leg_rot.x / 360.0)
                    * Mat4::from_rotation_x(-leg_rot.y / 360.0)
                    * Mat4::from_translation(Vec3::new(-cube::VALUE * 0.5, -cube::VALUE * 1.5, 0.0))
            }
            ModelPartType::Proj => {
                let aspect = self.width as f32 / self.height as f32;
                directx::perspective(PI / 4.0, aspect, 0.1, 10.0)
            }
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
                let cape_rot = if enable {
                    11.8 + self.skin_animation.cape
                } else {
                    6.3
                };
                Mat4::from_translation(Vec3::new(0.0, -2.0 * cube::VALUE, -cube::VALUE * 0.1))
                    * Mat4::from_rotation_x(cape_rot * std::f32::consts::PI / 180.0)
                    * Mat4::from_translation(Vec3::new(0.0, 1.6 * cube::VALUE, -cube::VALUE * 0.5))
            }
            ModelPartType::Body => Mat4::default(),
        }
    }

    /// 设置是否播放动画
    ///
    /// - `value`: `true` 表示播放
    pub fn set_animation(&mut self, value: bool) {
        self.skin_animation.run = value;
        self.animation = value;
    }

    /// 获取是否正在播放动画
    pub fn get_animation(&self) -> bool {
        self.animation
    }

    /// 设置皮肤类型，类型变化时标记需要重新加载模型
    ///
    /// - `value`: 皮肤类型
    pub fn set_skin_type(&mut self, value: SkinType) {
        if self.skin_type != value {
            self.skin_animation.skin_type = value;
            self.switch_model = true;
            self.skin_type = value;
        }
    }

    /// 获取当前皮肤类型
    pub fn get_skin_type(&self) -> SkinType {
        self.skin_type
    }

    /// 设置背景色
    ///
    /// - `color`: 背景色，RGBA 四个分量，取值范围 0.0..1.0
    pub fn set_back_color(&mut self, color: Vec4) {
        self.back_color = color;
        self.switch_back = true;
    }

    /// 获取背景色
    pub fn get_back_color(&self) -> Vec4 {
        self.back_color
    }

    /// 设置是否渲染披风
    ///
    /// - `value`: `true` 表示渲染
    pub fn set_enable_cape(&mut self, value: bool) {
        self.enable_cape = value;
        self.switch_type = true;
    }

    /// 获取是否渲染披风
    pub fn get_enable_cape(&self) -> bool {
        self.enable_cape
    }

    /// 设置是否渲染第二层，即顶层
    ///
    /// - `value`: `true` 表示渲染
    pub fn set_enable_top(&mut self, value: bool) {
        self.enable_top = value;
        self.switch_type = true;
    }

    /// 获取是否渲染第二层，即顶层
    pub fn get_enable_top(&self) -> bool {
        self.enable_top
    }

    /// 设置手臂旋转角度，单位度
    ///
    /// - `rotate`: 旋转角度
    pub fn set_arm_rotate(&mut self, rotate: Vec3) {
        self.arm_rotate = rotate;
    }

    /// 获取手臂旋转角度，单位度
    pub fn get_arm_rotate(&self) -> Vec3 {
        self.arm_rotate
    }

    /// 设置腿部旋转角度，单位度
    ///
    /// - `rotate`: 旋转角度
    pub fn set_leg_rotate(&mut self, rotate: Vec3) {
        self.leg_rotate = rotate;
    }

    /// 获取腿部旋转角度，单位度
    pub fn get_leg_rotate(&self) -> Vec3 {
        self.leg_rotate
    }

    /// 设置头部旋转角度，单位度
    ///
    /// - `rotate`: 旋转角度
    pub fn set_head_rotate(&mut self, rotate: Vec3) {
        self.head_rotate = rotate;
    }

    /// 获取头部旋转角度，单位度
    pub fn get_head_rotate(&self) -> Vec3 {
        self.head_rotate
    }

    /// 是否已加载披风
    pub fn have_cape(&self) -> bool {
        self.have_cape
    }

    /// 是否已加载皮肤
    pub fn have_skin(&self) -> bool {
        self.have_skin
    }
}

impl Drop for BaseSkinRender {
    fn drop(&mut self) {
        self.skin_animation.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tiny_skia::IntSize;

    /// 创建指定位图的辅助函数，RGBA8 纯色填充；不透明色下预乘值与原色相同
    fn make_bitmap(w: u32, h: u32, r: u8, g: u8, b: u8) -> Pixmap {
        let mut data = vec![0u8; (w * h * 4) as usize];
        for px in data.chunks_exact_mut(4) {
            px.copy_from_slice(&[r, g, b, 255]);
        }

        Pixmap::from_vec(data, IntSize::from_wh(w, h).unwrap()).unwrap()
    }

    /// new() 的默认值
    #[test]
    fn test_default_state() {
        let render = BaseSkinRender::new();
        assert_eq!(render.width, 800);
        assert_eq!(render.height, 600);
        assert_eq!(render.distance, 1.0);
        assert_eq!(render.skin_type, SkinType::Unknown);
        assert!(!render.have_skin);
        assert!(!render.have_cape);
        assert!(!render.animation);
        assert_eq!(render.back_color, Vec4::new(0.0, 0.0, 0.0, 1.0));
    }

    /// 左键拖拽旋转：rot_xy = (当前点 - 按下点) * 2，y 轴取反
    #[test]
    fn test_pointer_left_drag_rotation() {
        let mut render = BaseSkinRender::new();
        render.pointer_pressed(KeyType::Left, Vec2::new(10.0, 20.0));
        // 按下时 diff_xy = (10, -20)
        assert_eq!(render.diff_xy, Vec2::new(10.0, -20.0));

        render.pointer_moved(KeyType::Left, Vec2::new(15.0, 25.0));
        // rot_xy.y = (15-10)*2 = 10；rot_xy.x = (25 + (-20))*2 = 10
        // 按下时 diff_xy.y = -20，相当于以按下点 y 为基准计算增量
        assert_eq!(render.rot_xy, Vec2::new(10.0, 10.0));
        // diff_xy 更新为当前点
        assert_eq!(render.diff_xy, Vec2::new(15.0, -25.0));
    }

    /// 右键拖拽平移 + 松开保存位置
    #[test]
    fn test_pointer_right_drag_pan() {
        let mut render = BaseSkinRender::new();
        render.pointer_pressed(KeyType::Right, Vec2::new(5.0, 5.0));
        assert_eq!(render.last_xy, Vec2::new(5.0, 5.0));

        render.pointer_moved(KeyType::Right, Vec2::new(105.0, 5.0));
        // xy.x = -(5-105)/100 = 1.0
        assert_eq!(render.xy.x, 1.0);
        assert_eq!(render.xy.y, 0.0);

        render.pointer_released(KeyType::Right, Vec2::new(105.0, 5.0));
        assert_eq!(render.save_xy, render.xy);
    }

    /// 滚轮缩放与位置重置
    #[test]
    fn test_wheel_and_reset_position() {
        let mut render = BaseSkinRender::new();
        render.pointer_wheel_changed(true);
        assert_eq!(render.distance, 1.1);
        render.pointer_wheel_changed(false);
        render.pointer_wheel_changed(false);
        assert_eq!(render.distance, 0.9);

        render.position(0.5, -0.5);
        assert_eq!(render.xy, Vec2::new(0.5, -0.5));

        render.reset_position();
        assert_eq!(render.distance, 1.0);
        assert_eq!(render.xy, Vec2::ZERO);
        assert_eq!(render.diff_xy, Vec2::ZERO);
    }

    /// add_distance 与 rotate 的增量语义
    #[test]
    fn test_add_distance_and_rotate() {
        let mut render = BaseSkinRender::new();
        render.add_distance(0.5);
        assert_eq!(render.distance, 1.5);
        render.rotate(10.0, 20.0);
        assert_eq!(render.rot_xy, Vec2::new(10.0, 20.0));
    }

    /// 皮肤/披风贴图设置与状态回调
    #[test]
    fn test_set_skin_tex_and_callbacks() {
        let mut render = BaseSkinRender::new();

        // 记录状态回调
        let states = Arc::new(Mutex::new(Vec::new()));
        let sink = states.clone();
        render.state_callback = Some(Arc::new(Mutex::new(move |s: StateType| {
            sink.lock().unwrap().push(s);
        })));

        // 非 64 宽度的皮肤应被拒绝
        let bad = make_bitmap(32, 32, 255, 0, 0);
        assert_eq!(render.set_skin_tex(Some(bad)), Err(ErrorType::InvalidSkin));
        assert!(!render.have_skin);

        // None 表示清除皮肤
        render.set_skin_tex(None).unwrap();
        assert!(!render.have_skin);

        // 64x64 皮肤应成功加载并触发 SkinLoaded 回调
        let good = make_bitmap(64, 64, 0, 255, 0);
        render.set_skin_tex(Some(good)).unwrap();
        assert!(render.have_skin);
        assert!(render.switch_skin);

        // 披风
        let cape = make_bitmap(64, 32, 0, 0, 255);
        render.set_cape_tex(Some(cape)).unwrap();
        assert!(render.have_cape);
        assert!(render.switch_skin);

        let recorded = states.lock().unwrap();
        assert!(recorded.contains(&StateType::SkinLoaded));
        assert!(recorded.contains(&StateType::CapeLoaded));
    }

    /// 错误回调
    #[test]
    fn test_error_callback() {
        let mut render = BaseSkinRender::new();
        let errors = Arc::new(Mutex::new(Vec::new()));
        let sink = errors.clone();
        render.error_callback = Some(Arc::new(Mutex::new(move |e: ErrorType| {
            sink.lock().unwrap().push(e);
        })));
        render.on_error(ErrorType::TextureError);
        assert_eq!(*errors.lock().unwrap(), vec![ErrorType::TextureError]);
    }

    /// set_skin_type 只在类型变化时置位 switch_model
    #[test]
    fn test_set_skin_type() {
        let mut render = BaseSkinRender::new();
        assert!(!render.switch_model);
        render.set_skin_type(SkinType::NewSlim);
        assert_eq!(render.get_skin_type(), SkinType::NewSlim);
        assert!(render.switch_model);
        // 相同类型不再置位：先复位标志再设置一次
        render.switch_model = false;
        render.set_skin_type(SkinType::NewSlim);
        assert!(!render.switch_model);
    }

    /// tick 应把 rot_xy 累积到 last 矩阵并清零 rot_xy
    #[test]
    fn test_tick_accumulates_rotation() {
        let mut render = BaseSkinRender::new();
        render.rotate(360.0, 720.0);
        render.tick(0.016);
        assert_eq!(render.rot_xy, Vec2::ZERO);
        assert_ne!(render.last, Mat4::IDENTITY, "last 应记录了旋转");
        // 再次 tick：rot_xy 已为 0，last 不变
        let last = render.last;
        render.tick(0.016);
        assert_eq!(render.last, last);
    }

    /// tick 的 FPS 统计：每累计 1 秒触发一次 fps 回调并清零计数
    #[test]
    fn test_tick_fps_callback() {
        let mut render = BaseSkinRender::new();
        let fps_values = Arc::new(Mutex::new(Vec::new()));
        let sink = fps_values.clone();
        render.fps_callback = Some(Arc::new(Mutex::new(move |f: i32| {
            sink.lock().unwrap().push(f);
        })));

        // 0.016 * 63 = 1.008 >= 1.0，第 63 次 tick 触发回调
        for _ in 0..63 {
            render.tick(0.016);
        }
        assert_eq!(*fps_values.lock().unwrap(), vec![63]);
        assert_eq!(render.fps, 0, "触发后 fps 计数应清零");
        assert!(render.time < 1.0, "剩余时间应小于 1 秒");
    }

    /// 部件矩阵：头部矩阵在零旋转下等价于平移 (0, VALUE + 1.5*VALUE, 0)
    #[test]
    fn test_get_matrix_head() {
        let render = BaseSkinRender::new();
        let m = render.get_matrix(ModelPartType::Head);
        let w = m.w_axis;
        assert!((w.x - 0.0).abs() < 1e-5);
        assert!((w.y - (cube::VALUE + 1.5 * cube::VALUE)).abs() < 1e-5);
        assert!((w.z - 0.0).abs() < 1e-5);
    }

    /// 手臂矩阵在零旋转下的落点：肩部枢轴 x = arm_width * VALUE，
    /// 其中宽臂 arm_width = 1.5、纤细 = 1.375
    #[test]
    fn test_get_matrix_arms_width() {
        let mut render = BaseSkinRender::new();
        render.set_skin_type(SkinType::New);
        let wide = render.get_matrix(ModelPartType::LeftArm);
        assert!((wide.w_axis.x - 1.5 * cube::VALUE).abs() < 1e-5);
        assert!((wide.w_axis.y - 0.0).abs() < 1e-5);

        render.set_skin_type(SkinType::NewSlim);
        let slim = render.get_matrix(ModelPartType::LeftArm);
        assert!((slim.w_axis.x - 1.375 * cube::VALUE).abs() < 1e-5);
    }

    /// Model 矩阵应包含平移与等比缩放
    #[test]
    fn test_get_matrix_model() {
        let mut render = BaseSkinRender::new();
        render.position(0.3, -0.2);
        render.distance = 2.0;
        let m = render.get_matrix(ModelPartType::Model);
        assert!((m.w_axis.x - 0.3).abs() < 1e-5, "平移不应被缩放");
        assert!((m.w_axis.y - -0.2).abs() < 1e-5);
        assert!((m.x_axis.x - 2.0).abs() < 1e-5, "x 基向量应缩放 2 倍");
    }

    /// Proj / View / Cape 矩阵应全部为有限值
    #[test]
    fn test_get_matrix_finite() {
        let mut render = BaseSkinRender::new();
        render.skin_animation.cape = 1.5;
        render.set_animation(true);
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

    /// set_animation / getter 一致性；动画开启时 tick 会同步动画角度到部件旋转
    #[test]
    fn test_animation_sync_on_tick() {
        let mut render = BaseSkinRender::new();
        render.set_animation(true);
        assert!(render.get_animation());
        assert!(render.skin_animation.run);

        render.skin_animation.set_frame(30);
        render.skin_animation.run = true;
        render.skin_animation.tick(0.02); // frame 31
        render.tick(0.0);
        assert!(
            (render.head_rotate.z - (31.0 - 30.0)).abs() < 1e-5,
            "动画 head 角度应同步到 head_rotate"
        );

        render.set_animation(false);
        assert!(!render.get_animation());
    }

    /// 各 setter/getter 往返
    #[test]
    fn test_setters_getters() {
        let mut render = BaseSkinRender::new();

        render.set_back_color(Vec4::new(0.2, 0.4, 0.6, 1.0));
        assert_eq!(render.get_back_color(), Vec4::new(0.2, 0.4, 0.6, 1.0));
        assert!(render.switch_back);

        render.set_enable_cape(true);
        assert!(render.get_enable_cape());
        assert!(render.switch_type);
        render.set_enable_top(true);
        assert!(render.get_enable_top());

        let rot = Vec3::new(1.0, 2.0, 3.0);
        render.set_arm_rotate(rot);
        assert_eq!(render.get_arm_rotate(), rot);
        render.set_leg_rotate(rot);
        assert_eq!(render.get_leg_rotate(), rot);
        render.set_head_rotate(rot);
        assert_eq!(render.get_head_rotate(), rot);
    }
}
