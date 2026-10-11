//! 基于 OpenGL 的皮肤渲染后端
//!
//! 组合 [`BaseSkinRender`] 与 glow 上下文，把史蒂夫模型的本体、顶层与披风
//! 渲染到指定的帧缓冲。

pub mod gl_model;
pub mod gl_shader;

use std::sync::Arc;

use glam::{Vec2, Vec3};
use glow::*;
use mml_skin::SkinType;
use tiny_skia::Pixmap;

use crate::{
    BaseSkinRender, ErrorType, ModelPartType, cube,
    cube_model::CubeModelItemObj,
    model,
    renders::gl_render::gl_model::{ModelVao, VaoItem, VertexOpenGL},
    texture,
};

/// 渲染类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkinRenderType {
    /// 普通渲染，无后处理抗锯齿
    Normal,
    /// FXAA 后处理抗锯齿
    FXAA,
    /// MSAA 多重采样抗锯齿
    MSAA,
}

/// 编译并链接着色器程序，macOS 下注入版本头
///
/// - `gl`: OpenGL 上下文
///
/// # 返回值
///
/// 返回链接好的着色器程序，编译或链接失败时 panic
fn init_shader(gl: &Context) -> Program {
    let mut vertex = String::from(gl_shader::VERTEX_SHADER_SOURCE);
    if cfg!(target_os = "macos") {
        vertex.insert_str(0, gl_shader::MACOS_HEADER);
    }

    let mut fragment = String::from(gl_shader::FRAGMENT_SHADER_SOURCE);
    if cfg!(target_os = "macos") {
        fragment.insert_str(0, gl_shader::MACOS_HEADER);
    }

    unsafe {
        let vertex_shader = gl.create_shader(VERTEX_SHADER).unwrap();

        gl.shader_source(vertex_shader, &vertex);
        gl.compile_shader(vertex_shader);
        if !gl.get_shader_compile_status(vertex_shader) {
            panic!(
                "vertex Shader compile fail: {info}",
                info = gl.get_shader_info_log(vertex_shader)
            );
        }

        let fragment_shader = gl.create_shader(FRAGMENT_SHADER).unwrap();

        gl.shader_source(fragment_shader, &fragment);
        gl.compile_shader(fragment_shader);
        if !gl.get_shader_compile_status(fragment_shader) {
            panic!(
                "fragment Shader compile fail: {info}",
                info = gl.get_shader_info_log(fragment_shader)
            );
        }

        let pg = gl.create_program().unwrap();

        gl.attach_shader(pg, vertex_shader);
        gl.attach_shader(pg, fragment_shader);
        gl.link_program(pg);
        if !gl.get_program_link_status(pg) {
            panic!(
                "Program link fail: {info}",
                info = gl.get_program_info_log(pg)
            );
        }

        gl.detach_shader(pg, vertex_shader);
        gl.detach_shader(pg, fragment_shader);

        gl.delete_shader(vertex_shader);
        gl.delete_shader(fragment_shader);

        pg
    }
}

/// 检查并打印 OpenGL 错误队列中的全部错误
///
/// - `gl`: OpenGL 上下文
fn check_error(gl: &Context) {
    unsafe {
        let mut err = gl.get_error();
        while err != glow::NO_ERROR {
            eprintln!("OpenGL Error: {}", err);
            err = gl.get_error();
        }
    }
}

/// 把位图上传为 OpenGL 纹理，NEAREST 采样加边缘钳制
///
/// - `gl`: OpenGL 上下文
/// - `image`: 要上传的位图，RGBA8 内存序
/// - `texture`: 目标纹理对象
fn load_tex(gl: &glow::Context, image: &Pixmap, texture: Texture) {
    unsafe {
        gl.active_texture(TEXTURE0);
        gl.bind_texture(TEXTURE_2D, Some(texture));

        gl.tex_parameter_i32(TEXTURE_2D, TEXTURE_MIN_FILTER, NEAREST as i32);
        gl.tex_parameter_i32(TEXTURE_2D, TEXTURE_MAG_FILTER, NEAREST as i32);
        gl.tex_parameter_i32(TEXTURE_2D, TEXTURE_WRAP_S, CLAMP_TO_BORDER as i32);
        gl.tex_parameter_i32(TEXTURE_2D, TEXTURE_WRAP_T, CLAMP_TO_BORDER as i32);
    }

    // 位图固定是 RGBA8，直接按 RGBA 上传，原先 GLES 下的 BGRA 转换分支不再需要
    unsafe {
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA8 as i32,
            image.width() as i32,
            image.height() as i32,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            PixelUnpackData::Slice(Some(image.data())),
        );
    }

    unsafe {
        gl.bind_texture(glow::TEXTURE_2D, None);
    }
}

/// 把模型数据的顶点、UV、法线与索引填充到 VAO 缓冲
///
/// - `gl`: OpenGL 上下文
/// - `vao`: 目标部件的缓冲对象集合
/// - `model`: 模型顶点与索引数据
/// - `uv`: 归一化 UV 坐标数组
/// - `pg`: 着色器程序，用于获取 attrib 位置
fn put_vao_item(gl: &Context, vao: &VaoItem, model: &CubeModelItemObj, uv: &Vec<f32>, pg: Program) {
    unsafe {
        gl.use_program(Some(pg));

        gl.bind_vertex_array(Some(vao.vertex_array_object));

        let postion = gl.get_attrib_location(pg, "a_position").unwrap();
        let tex = gl.get_attrib_location(pg, "a_texCoord").unwrap();
        let normal = gl.get_attrib_location(pg, "a_normal").unwrap();

        gl.disable_vertex_attrib_array(postion);
        gl.disable_vertex_attrib_array(tex);
        gl.disable_vertex_attrib_array(normal);

        let size = model.model.len() / 3;

        let mut points: Vec<VertexOpenGL> = Vec::new();

        for index in 0..size {
            let src = index * 3;
            let src1 = index * 2;

            points[index] = VertexOpenGL {
                pos: Vec3::new(model.model[src], model.model[src + 1], model.model[src + 2]),
                uv: Vec2::new(uv[src1], uv[src1 + 1]),
                normal: Vec3::new(
                    cube::VERTICES[src],
                    cube::VERTICES[src + 1],
                    cube::VERTICES[src + 2],
                ),
            }
        }

        gl.bind_buffer(ARRAY_BUFFER, Some(vao.vertex_buffer_object));
        let data: &[u8] = core::slice::from_raw_parts(
            points.as_ptr() as *const u8,
            points.len() * core::mem::size_of::<VertexOpenGL>(),
        );
        gl.buffer_data_u8_slice(ARRAY_BUFFER, data, STATIC_DRAW);

        gl.bind_buffer(ARRAY_BUFFER, Some(vao.index_buffer_object));
        let data: &[u8] = core::slice::from_raw_parts(
            model.point.as_ptr() as *const u8,
            model.point.len() * core::mem::size_of::<VertexOpenGL>(),
        );
        gl.buffer_data_u8_slice(ARRAY_BUFFER, data, STATIC_DRAW);

        gl.vertex_attrib_pointer_f32(
            postion,
            3,
            FLOAT,
            false,
            (core::mem::size_of::<f32>() * 8) as i32,
            0,
        );
        gl.vertex_attrib_pointer_f32(
            tex,
            3,
            FLOAT,
            false,
            (core::mem::size_of::<f32>() * 8) as i32,
            (core::mem::size_of::<f32>() * 3) as i32,
        );
        gl.vertex_attrib_pointer_f32(
            normal,
            3,
            FLOAT,
            false,
            (core::mem::size_of::<f32>() * 8) as i32,
            (core::mem::size_of::<f32>() * 5) as i32,
        );

        gl.enable_vertex_attrib_array(postion);
        gl.enable_vertex_attrib_array(tex);
        gl.enable_vertex_attrib_array(normal);

        gl.bind_vertex_array(None);
    }
}

/// 渲染类型枚举
///
/// 与 [`SkinRenderType`] 内容相同，供后处理分支代码使用；当前相关逻辑被注释停用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkinRenderGLType {
    /// 普通渲染
    Normal,
    /// MSAA 多重采样抗锯齿
    MSAA,
    /// FXAA 后处理抗锯齿
    FXAA,
}

impl Default for SkinRenderGLType {
    fn default() -> Self {
        Self::Normal
    }
}

/// OpenGL 皮肤渲染器
pub struct SkinRenderOpenGL {
    /// 公共渲染状态，含交互、动画与贴图来源
    pub base: BaseSkinRender,

    /// OpenGL 上下文
    gl: Arc<Context>,

    /// OpenGL 适配信息，含渲染器、版本与 GLSL 版本
    pub info: String,

    // 渲染状态
    /// 当前渲染类型
    render_type: SkinRenderType,

    /// 上次渲染使用的画布宽度
    render_width: i32,
    /// 上次渲染使用的画布高度
    render_height: i32,

    // 着色器程序
    /// 皮肤渲染着色器程序
    pg: Program,

    // 纹理
    /// 皮肤贴图
    texture_skin: Texture,
    /// 披风贴图
    texture_cape: Texture,

    // 普通模型的 VAO
    /// 本体模型的缓冲
    normal_vao: ModelVao,
    // 顶层模型的 VAO
    /// 顶层模型的缓冲
    top_vao: ModelVao,

    // 模型数据
    /// 单个部件的索引数量，即绘制时的元素个数
    steve_model_draw_order_count: i32,
}

impl SkinRenderOpenGL {
    /// 创建 OpenGL 渲染器，编译着色器、创建纹理与缓冲并启用背面剔除
    ///
    /// - `gl`: OpenGL 上下文
    pub fn new(gl: Arc<glow::Context>) -> Self {
        unsafe {
            let pg = init_shader(&gl);
            let skin = gl.create_texture().unwrap();
            let cape = gl.create_texture().unwrap();
            let model = ModelVao::new(&gl);
            let top = ModelVao::new(&gl);

            let info = format!(
                "Renderer: {}\nOpenGL Version: {}\nGLSL Version: {}",
                gl.get_parameter_string(glow::RENDERER),
                gl.get_parameter_string(glow::VERSION),
                gl.get_parameter_string(glow::SHADING_LANGUAGE_VERSION)
            );

            gl.clear_color(0.0, 0.0, 0.0, 1.0);
            gl.enable(CULL_FACE);
            gl.cull_face(BACK);

            Self {
                base: BaseSkinRender::new(),
                gl,
                info,
                pg,
                render_width: 0,
                render_height: 0,
                texture_skin: skin,
                texture_cape: cape,
                normal_vao: model,
                top_vao: top,
                steve_model_draw_order_count: 0,
                render_type: SkinRenderType::Normal,
            }
        }
    }

    /// 设置渲染类型，下次渲染时生效
    ///
    /// - `value`: 渲染类型
    pub fn set_render_type(&mut self, value: SkinRenderType) {
        self.render_type = value;
        self.base.switch_type = true;
    }

    /// 获取当前渲染类型
    pub fn get_render_type(&self) -> SkinRenderType {
        self.render_type
    }

    /// 绘制披风，未加载披风或未启用披风渲染时跳过
    fn draw_cape(&self) {
        if self.base.have_cape && self.base.enable_cape {
            unsafe {
                self.gl.bind_texture(TEXTURE_2D, Some(self.texture_cape));
                let model_loc = self.gl.get_uniform_location(self.pg, "self");
                if let Some(loc) = model_loc {
                    let mat = self.base.get_matrix(ModelPartType::Cape);
                    self.gl
                        .uniform_matrix_4_f32_slice(Some(&loc), false, mat.as_ref());
                    self.gl
                        .bind_vertex_array(Some(self.normal_vao.cape.vertex_array_object));
                    self.gl.draw_elements(
                        TRIANGLES,
                        self.steve_model_draw_order_count,
                        UNSIGNED_SHORT,
                        0,
                    );
                }
                self.gl.bind_texture(TEXTURE_2D, None);
            }
        }
    }

    /// 绘制本体模型的身体、头部与四肢，各自应用部件矩阵
    fn draw_skin(&mut self) {
        unsafe {
            self.gl.bind_texture(TEXTURE_2D, Some(self.texture_skin));

            if let Some(loc) = self.gl.get_uniform_location(self.pg, "self") {
                let mat = self.base.get_matrix(ModelPartType::Body);
                self.gl
                    .uniform_matrix_4_f32_slice(Some(&loc), false, mat.as_ref());
                self.gl
                    .bind_vertex_array(Some(self.normal_vao.body.vertex_array_object));
                self.gl.draw_elements(
                    TRIANGLES,
                    self.steve_model_draw_order_count,
                    UNSIGNED_SHORT,
                    0,
                );

                let mat = self.base.get_matrix(ModelPartType::Head);
                self.gl
                    .uniform_matrix_4_f32_slice(Some(&loc), false, mat.as_ref());
                self.gl
                    .bind_vertex_array(Some(self.normal_vao.head.vertex_array_object));
                self.gl.draw_elements(
                    TRIANGLES,
                    self.steve_model_draw_order_count,
                    UNSIGNED_SHORT,
                    0,
                );

                let mat = self.base.get_matrix(ModelPartType::LeftArm);
                self.gl
                    .uniform_matrix_4_f32_slice(Some(&loc), false, mat.as_ref());
                self.gl
                    .bind_vertex_array(Some(self.normal_vao.left_arm.vertex_array_object));
                self.gl.draw_elements(
                    TRIANGLES,
                    self.steve_model_draw_order_count,
                    UNSIGNED_SHORT,
                    0,
                );

                let mat = self.base.get_matrix(ModelPartType::RightArm);
                self.gl
                    .uniform_matrix_4_f32_slice(Some(&loc), false, mat.as_ref());
                self.gl
                    .bind_vertex_array(Some(self.normal_vao.right_arm.vertex_array_object));
                self.gl.draw_elements(
                    TRIANGLES,
                    self.steve_model_draw_order_count,
                    UNSIGNED_SHORT,
                    0,
                );

                let mat = self.base.get_matrix(ModelPartType::LeftLeg);
                self.gl
                    .uniform_matrix_4_f32_slice(Some(&loc), false, mat.as_ref());
                self.gl
                    .bind_vertex_array(Some(self.normal_vao.left_leg.vertex_array_object));
                self.gl.draw_elements(
                    TRIANGLES,
                    self.steve_model_draw_order_count,
                    UNSIGNED_SHORT,
                    0,
                );

                let mat = self.base.get_matrix(ModelPartType::RightLeg);
                self.gl
                    .uniform_matrix_4_f32_slice(Some(&loc), false, mat.as_ref());
                self.gl
                    .bind_vertex_array(Some(self.normal_vao.right_leg.vertex_array_object));
                self.gl.draw_elements(
                    TRIANGLES,
                    self.steve_model_draw_order_count,
                    UNSIGNED_SHORT,
                    0,
                );
            }

            self.gl.bind_vertex_array(None);
            self.gl.bind_texture(TEXTURE_2D, None);
        }
    }

    /// 绘制顶层模型，半透明混合并关闭深度写入，避免与本体穿插闪烁
    fn draw_skin_top(&mut self) {
        unsafe {
            self.gl.bind_texture(TEXTURE_2D, Some(self.texture_skin));

            if let Some(loc) = self.gl.get_uniform_location(self.pg, "self") {
                let mat = self.base.get_matrix(ModelPartType::Body);
                self.gl
                    .uniform_matrix_4_f32_slice(Some(&loc), false, mat.as_ref());
                self.gl
                    .bind_vertex_array(Some(self.top_vao.body.vertex_array_object));
                self.gl.draw_elements(
                    TRIANGLES,
                    self.steve_model_draw_order_count,
                    UNSIGNED_SHORT,
                    0,
                );

                let mat = self.base.get_matrix(ModelPartType::Head);
                self.gl
                    .uniform_matrix_4_f32_slice(Some(&loc), false, mat.as_ref());
                self.gl
                    .bind_vertex_array(Some(self.top_vao.head.vertex_array_object));
                self.gl.draw_elements(
                    TRIANGLES,
                    self.steve_model_draw_order_count,
                    UNSIGNED_SHORT,
                    0,
                );

                let mat = self.base.get_matrix(ModelPartType::LeftArm);
                self.gl
                    .uniform_matrix_4_f32_slice(Some(&loc), false, mat.as_ref());
                self.gl
                    .bind_vertex_array(Some(self.top_vao.left_arm.vertex_array_object));
                self.gl.draw_elements(
                    TRIANGLES,
                    self.steve_model_draw_order_count,
                    UNSIGNED_SHORT,
                    0,
                );

                let mat = self.base.get_matrix(ModelPartType::RightArm);
                self.gl
                    .uniform_matrix_4_f32_slice(Some(&loc), false, mat.as_ref());
                self.gl
                    .bind_vertex_array(Some(self.top_vao.right_arm.vertex_array_object));
                self.gl.draw_elements(
                    TRIANGLES,
                    self.steve_model_draw_order_count,
                    UNSIGNED_SHORT,
                    0,
                );

                let mat = self.base.get_matrix(ModelPartType::LeftLeg);
                self.gl
                    .uniform_matrix_4_f32_slice(Some(&loc), false, mat.as_ref());
                self.gl
                    .bind_vertex_array(Some(self.top_vao.left_leg.vertex_array_object));
                self.gl.draw_elements(
                    TRIANGLES,
                    self.steve_model_draw_order_count,
                    UNSIGNED_SHORT,
                    0,
                );

                let mat = self.base.get_matrix(ModelPartType::RightLeg);
                self.gl
                    .uniform_matrix_4_f32_slice(Some(&loc), false, mat.as_ref());
                self.gl
                    .bind_vertex_array(Some(self.top_vao.right_leg.vertex_array_object));
                self.gl.draw_elements(
                    TRIANGLES,
                    self.steve_model_draw_order_count,
                    UNSIGNED_SHORT,
                    0,
                );
            }

            self.gl.bind_texture(TEXTURE_2D, None);
        }
    }

    /// 按当前皮肤类型重新生成模型与 UV 数据并填充各部件缓冲
    fn load_model(&mut self) {
        let normal = model::get_steve(self.base.skin_type);
        let top = model::get_steve_top(self.base.skin_type);
        let tex = texture::get_steve_texture(self.base.skin_type);
        let textop = texture::get_steve_texture_top(self.base.skin_type);

        self.steve_model_draw_order_count = normal.head.point.len() as i32;

        put_vao_item(
            &self.gl,
            &self.normal_vao.head,
            &normal.head,
            &tex.head,
            self.pg,
        );
        put_vao_item(
            &self.gl,
            &self.normal_vao.body,
            &normal.body,
            &tex.body,
            self.pg,
        );
        put_vao_item(
            &self.gl,
            &self.normal_vao.left_arm,
            &normal.left_arm,
            &tex.left_arm,
            self.pg,
        );
        put_vao_item(
            &self.gl,
            &self.normal_vao.right_arm,
            &normal.right_arm,
            &tex.right_arm,
            self.pg,
        );
        put_vao_item(
            &self.gl,
            &self.normal_vao.left_leg,
            &normal.left_leg,
            &tex.left_leg,
            self.pg,
        );
        put_vao_item(
            &self.gl,
            &self.normal_vao.right_leg,
            &normal.right_leg,
            &tex.right_leg,
            self.pg,
        );

        put_vao_item(
            &self.gl,
            &self.normal_vao.cape,
            &normal.cape,
            &tex.cape,
            self.pg,
        );

        put_vao_item(
            &self.gl,
            &self.top_vao.head,
            &top.head,
            &textop.head,
            self.pg,
        );
        put_vao_item(
            &self.gl,
            &self.top_vao.body,
            &top.body,
            &textop.body,
            self.pg,
        );
        put_vao_item(
            &self.gl,
            &self.top_vao.left_arm,
            &top.left_arm,
            &textop.left_arm,
            self.pg,
        );
        put_vao_item(
            &self.gl,
            &self.top_vao.right_arm,
            &top.right_arm,
            &textop.right_arm,
            self.pg,
        );
        put_vao_item(
            &self.gl,
            &self.top_vao.left_leg,
            &top.left_leg,
            &textop.left_leg,
            self.pg,
        );
        put_vao_item(
            &self.gl,
            &self.top_vao.right_leg,
            &top.right_leg,
            &textop.right_leg,
            self.pg,
        );
    }

    /// 开始渲染
    ///
    /// - `fb`: 目标帧缓冲，传 `None` 表示绑定默认帧缓冲
    ///
    /// 按标志位刷新贴图 / 模型后，清屏并把本体、披风、顶层依次绘制到
    /// 指定帧缓冲。未加载皮肤或画布尺寸为 0 时跳过绘制。
    pub fn open_gl_render(&mut self, fb: Option<Framebuffer>) {
        if self.base.switch_skin {
            self.load_skin();
        }
        if self.base.switch_model {
            self.load_model();
        }

        if !self.base.have_skin {
            return;
        }

        if self.base.width == 0 || self.base.height == 0 {
            return;
        }

        if self.base.width != self.render_width || self.base.height != self.render_height {
            self.render_width = self.base.width;
            self.render_height = self.base.height;
        }

        if self.render_width == 0 || self.render_height == 0 {
            return;
        }

        unsafe {
            // match self.base.render_type {
            //     SkinRenderType::MSAA => {
            //         self.gl
            //             .bind_framebuffer(glow::FRAMEBUFFER, Some(self.msaa_frame_buffer));
            //     }
            //     SkinRenderType::FXAA => {
            //         self.gl
            //             .bind_framebuffer(glow::FRAMEBUFFER, Some(self.fxaa_frame_buffer));
            //     }
            //     _ => {
            //         self.gl.bind_framebuffer(glow::FRAMEBUFFER, fb);
            //     }
            // }

            self.gl.bind_framebuffer(glow::FRAMEBUFFER, fb);

            self.gl
                .viewport(0, 0, self.render_width, self.render_height);

            // if self.base.render_type == SkinRenderType::FXAA {
            //     self.gl.clear_color(1.0, 1.0, 1.0, 1.0);
            // } else {
            //     self.gl.clear_color(
            //         self.base.back_color.x,
            //         self.base.back_color.y,
            //         self.base.back_color.z,
            //         self.base.back_color.w,
            //     );
            // }

            self.gl.clear_color(
                self.base.back_color.x,
                self.base.back_color.y,
                self.base.back_color.z,
                self.base.back_color.w,
            );

            self.gl.clear_depth(1.0);
            self.gl.clear(COLOR_BUFFER_BIT | DEPTH_BUFFER_BIT);

            self.gl.enable(CULL_FACE);
            self.gl.enable(DEPTH_TEST);
            self.gl.active_texture(TEXTURE0);

            self.gl.use_program(Some(self.pg));

            let matr = self.base.get_matrix(ModelPartType::Proj);
            if let Some(loc) = self.gl.get_uniform_location(self.pg, "projection") {
                self.gl
                    .uniform_matrix_4_f32_slice(Some(&loc), false, matr.as_ref());
            }

            let matr = self.base.get_matrix(ModelPartType::View);
            if let Some(loc) = self.gl.get_uniform_location(self.pg, "view") {
                self.gl
                    .uniform_matrix_4_f32_slice(Some(&loc), false, matr.as_ref());
            }

            let matr = self.base.get_matrix(ModelPartType::Model);
            if let Some(loc) = self.gl.get_uniform_location(self.pg, "model") {
                self.gl
                    .uniform_matrix_4_f32_slice(Some(&loc), false, matr.as_ref());
            }

            self.gl.depth_mask(true);
            self.gl.disable(BLEND);

            self.draw_skin();
            self.draw_cape();

            if self.base.enable_top {
                self.gl.depth_mask(false);
                self.gl.enable(BLEND);
                self.gl.enable(SAMPLE_ALPHA_TO_COVERAGE);
                self.gl.blend_func(SRC_ALPHA, ONE_MINUS_SRC_ALPHA);

                self.draw_skin_top();

                self.gl.depth_mask(true);
                self.gl.disable(BLEND);
            }

            // // MSAA 后处理
            // if self.base.render_type == SkinRenderGLType::MSAA {
            //     self.gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, fb);
            //     self.gl
            //         .bind_framebuffer(glow::READ_FRAMEBUFFER, Some(self.msaa_frame_buffer));
            //     self.gl.blit_framebuffer(
            //         0,
            //         0,
            //         self.width,
            //         self.height,
            //         0,
            //         0,
            //         self.width,
            //         self.height,
            //         glow::COLOR_BUFFER_BIT,
            //         glow::NEAREST,
            //     );
            //     self.gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            // }
            // // FXAA 后处理
            // else if self.base.render_type == SkinRenderGLType::FXAA {
            //     self.gl.enable(glow::BLEND);
            //     self.gl.disable(glow::DEPTH_TEST);
            //     self.gl.bind_framebuffer(glow::FRAMEBUFFER, fb);
            //     self.gl.viewport(0, 0, self.width, self.height);
            //     self.gl.clear(glow::COLOR_BUFFER_BIT);
            //     self.gl.use_program(Some(self.pg_fxaa));
            //     self.gl.uniform_2_f32(
            //         self.fxaa_step,
            //         1.0 / self.width as f32,
            //         1.0 / self.height as f32,
            //     );
            //     self.gl.active_texture(glow::TEXTURE0);
            //     self.gl
            //         .bind_texture(glow::TEXTURE_2D, Some(self.fxaa_texture));
            //     self.gl.bind_vertex_array(Some(self.fxaa_vao));
            //     self.gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
            //     self.gl.bind_vertex_array(None);
            //     self.gl.enable(glow::DEPTH_TEST);
            //     self.gl.bind_texture(glow::TEXTURE_2D, None);
            //     self.gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            // }

            self.gl.bind_buffer(ARRAY_BUFFER, None);
            self.gl.bind_buffer(ELEMENT_ARRAY_BUFFER, None);
            self.gl.bind_vertex_array(None);
            self.gl.use_program(None);

            check_error(&self.gl);
        }
    }

    /// 上传皮肤 / 披风贴图，贴图无效或皮肤类型未知时触发错误回调
    fn load_skin(&mut self) {
        let base = &mut self.base;

        if base.skin_tex.is_none() {
            base.on_error(ErrorType::InvalidSkin);
            return;
        }

        if base.skin_type == SkinType::Unknown {
            base.on_error(ErrorType::InvalidSkin);
            return;
        }

        let skin_tex = base.skin_tex.as_mut().unwrap();
        load_tex(&self.gl, skin_tex, self.texture_skin);

        if let Some(cape_tex) = base.cape.as_mut() {
            load_tex(&self.gl, cape_tex, self.texture_cape);
        }

        base.switch_skin = false;
        base.switch_model = true;
    }
}

impl Drop for SkinRenderOpenGL {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_texture(self.texture_skin);
            self.gl.delete_texture(self.texture_cape);

            self.normal_vao.delete(&self.gl);
            self.top_vao.delete(&self.gl);

            self.gl.delete_program(self.pg);
        }
    }
}
