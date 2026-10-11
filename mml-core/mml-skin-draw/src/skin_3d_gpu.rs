//! 基于 wgpu 的皮肤渲染后端
//!
//! 把史蒂夫模型的本体、披风与顶层按 [`SkinRenderWgpu`] 的参数离屏渲染成 [`Pixmap`]；
//! 后端回退链与像素约定见 `mml-gpu`，参数与部件矩阵见 [`crate::skin_3d_render`]。
//!
//! 顶点每帧现搭：先按 [`SkinRenderWgpu::get_matrix`] 把各部件的顶点与法线变换到世界坐标，
//! 再拼成一个顶点缓冲，管线侧只剩「投影 * 视图」一个矩阵。

use std::sync::OnceLock;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec2, Vec3, Vec4};
use mml_skin::SkinType;
use tiny_skia::Pixmap;
use wgpu::util::DeviceExt;

use crate::{
    skin_3d_cube,
    skin_3d_cube_model::{CubeModelItemObj, SteveModel, SteveTexture},
    skin_3d_model,
    skin_3d_render::{ErrorType, ModelPartType, SkinRenderType},
    skin_3d_texture,
};

/// 渲染 shader，顶点入口 `vs`、片元入口 `fs`
const WGSL: &str = include_str!("skin_3d_gpu.wgsl");

/// MSAA 采样数，[`SkinRenderType::MSAA`] 用 4x
const MSAA_SAMPLES: u32 = 4;

/// 顶点，世界坐标加整张贴图的归一化 uv 与法线
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vert {
    /// 顶点位置，已乘部件矩阵，世界坐标，单位同 [`skin_3d_cube::VALUE`]
    pos: [f32; 3],
    /// 贴图坐标，按皮肤贴图尺寸归一化到 0..1，原点在贴图左上角
    uv: [f32; 2],
    /// 顶点法线，已转到世界坐标，单位向量
    normal: [f32; 3],
}

/// uniform 布局，只含一个「投影 * 视图」矩阵
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    /// 投影乘视图矩阵，列主序
    mvp: [[f32; 4]; 4],
}

/// 一个部件的顶点来源
struct PartRef<'a> {
    /// 部件类型，决定用哪个部件矩阵
    part: ModelPartType,
    /// 该部件的立方体顶点与索引
    shape: &'a CubeModelItemObj,
    /// 该部件的 UV，长度为顶点数的两倍，已归一化到 0..1
    uv: &'a [f32],
}

/// 把模型的六个部件与对应 UV 配成待拼装列表，本体与顶层共用
fn parts_of<'a>(shape: &'a SteveModel, tex: &'a SteveTexture) -> [PartRef<'a>; 6] {
    [
        PartRef {
            part: ModelPartType::Head,
            shape: &shape.head,
            uv: &tex.head,
        },
        PartRef {
            part: ModelPartType::Body,
            shape: &shape.body,
            uv: &tex.body,
        },
        PartRef {
            part: ModelPartType::LeftArm,
            shape: &shape.left_arm,
            uv: &tex.left_arm,
        },
        PartRef {
            part: ModelPartType::RightArm,
            shape: &shape.right_arm,
            uv: &tex.right_arm,
        },
        PartRef {
            part: ModelPartType::LeftLeg,
            shape: &shape.left_leg,
            uv: &tex.left_leg,
        },
        PartRef {
            part: ModelPartType::RightLeg,
            shape: &shape.right_leg,
            uv: &tex.right_leg,
        },
    ]
}

/// 拼一组部件的顶点与索引
///
/// 各部件顶点先乘自己的部件矩阵转到世界坐标，法线按 `w = 0` 变换，即只吃旋转不吃平移；
/// 索引按已追加的顶点数整体偏移。空部件直接跳过。
fn build_mesh(render: &SkinRenderWgpu, parts: &[PartRef<'_>]) -> (Vec<Vert>, Vec<u16>) {
    let mut verts = Vec::new();
    let mut indices = Vec::new();

    for part in parts {
        if part.shape.model.is_empty() {
            continue;
        }

        let tran = render.get_matrix(part.part);
        let offset = verts.len() as u16;

        for index in 0..part.shape.model.len() / 3 {
            let src = index * 3;
            let uv = index * 2;

            verts.push(Vert {
                pos: (tran * Vec3::from_slice(&part.shape.model[src..src + 3]).extend(1.0))
                    .truncate()
                    .to_array(),
                uv: [part.uv[uv], part.uv[uv + 1]],
                normal: (tran * Vec3::from_slice(&skin_3d_cube::VERTICES[src..src + 3]).extend(0.0))
                    .truncate()
                    .normalize()
                    .to_array(),
            });
        }

        indices.extend(part.shape.point.iter().map(|point| point + offset));
    }

    (verts, indices)
}

/// 一份 GPU 网格：顶点缓冲、索引缓冲与索引数量
struct Mesh {
    /// 顶点缓冲
    verts: wgpu::Buffer,
    /// 索引缓冲，格式 u16
    indices: wgpu::Buffer,
    /// 索引数量，即 `draw_indexed` 的范围
    count: u32,
}

/// 把 CPU 顶点与索引上传成网格，索引为空时返回 `None`，不建空缓冲
fn upload_mesh(device: &wgpu::Device, label: &str, verts: &[Vert], indices: &[u16]) -> Option<Mesh> {
    if indices.is_empty() {
        return None;
    }

    Some(Mesh {
        verts: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(verts),
            usage: wgpu::BufferUsages::VERTEX,
        }),
        indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        }),
        count: indices.len() as u32,
    })
}

/// 一组渲染管线，按采样数区分
struct Layer {
    /// 本体与披风管线，写深度
    base: wgpu::RenderPipeline,
    /// 顶层管线，混合且只测深度不写深度
    overlay: wgpu::RenderPipeline,
}

/// 全部管线、绑定组布局与采样器，进程内只建一次
struct Pipelines {
    /// 单采样管线
    single: Layer,
    /// 4x MSAA 管线
    msaa: Layer,
    /// 绑定组布局：0 号 uniform、1 号采样器、2 号贴图
    bind_layout: wgpu::BindGroupLayout,
    /// 贴图采样器，NEAREST 采样加边缘钳制
    sampler: wgpu::Sampler,
}

/// 管线缓存，设备全局唯一；初始化失败同样记入，避免每次渲染反复重试
static PIPE: OnceLock<Option<Pipelines>> = OnceLock::new();

/// 取全局管线；首次调用时按需创建，无可用 GPU 时返回 `None`
fn pipelines() -> Option<&'static Pipelines> {
    PIPE.get_or_init(|| {
        let gpu = mml_gpu::device()?;
        Some(Pipelines::new(gpu.device()))
    })
    .as_ref()
}

impl Pipelines {
    /// 建 shader、绑定组布局、采样器与单采样 / MSAA 两套管线
    fn new(device: &wgpu::Device) -> Pipelines {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mml-skin-draw"),
            source: wgpu::ShaderSource::Wgsl(WGSL.into()),
        });

        // 绑定组布局与采样器都是三类离屏渲染共用的形状，统一由 mml-gpu 提供
        let bind_layout = mml_gpu::texture_bind_layout(device, wgpu::ShaderStages::VERTEX);

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mml-skin-draw"),
            bind_group_layouts: &[Some(&bind_layout)],
            immediate_size: 0,
        });

        // 两层都做预乘意义上的 over：贴图是 straight 源，目标按预乘读回成 Pixmap，
        // 这个组合把 straight 源正确叠到预乘底上，透明的贴图像素不会写坏底色
        let blend = Some(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        });

        // 背面剔除：部件的相邻共面面不剔除会互相盖住；顶层靠深度测试压在本体上
        let mk = |samples: u32, depth_write: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("mml-skin-draw"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vert>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x3,
                                offset: 0,
                                shader_location: 0,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: 12,
                                shader_location: 1,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x3,
                                offset: 20,
                                shader_location: 2,
                            },
                        ],
                    })],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    // 各面按右手系外侧 CCW 缠绕，但 wgpu 在 y 向下的帧缓冲坐标系里判绕序，
                    // 外侧面读出来是 CW，用 Ccw 会把可见面全剔掉、只剩背面（照不到光）
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(depth_write),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: samples,
                    ..Default::default()
                },
                multiview_mask: None,
                cache: None,
            })
        };

        Pipelines {
            single: Layer {
                base: mk(1, true),
                overlay: mk(1, false),
            },
            msaa: Layer {
                base: mk(MSAA_SAMPLES, true),
                overlay: mk(MSAA_SAMPLES, false),
            },
            bind_layout,
            sampler: mml_gpu::nearest_sampler(device),
        }
    }
}

/// 把背景色换算成预乘清屏色，目标按预乘读回，alpha 小于 1 时颜色分量要一起缩放
///
/// - `color`: 背景色，RGBA 四个分量，取值范围 0.0..1.0
fn clear_color(color: Vec4) -> wgpu::Color {
    wgpu::Color {
        r: f64::from(color.x * color.w),
        g: f64::from(color.y * color.w),
        b: f64::from(color.z * color.w),
        a: f64::from(color.w),
    }
}

/// wgpu 皮肤渲染器
///
/// 持有全部渲染参数与渲染档位：管线是进程级缓存，贴图与顶点每次渲染现搭。
/// 参数与部件矩阵的方法在 [`crate::skin_3d_render`]。
pub struct SkinRenderWgpu {
    /// 是否渲染披风
    pub enable_cape: bool,
    /// 是否渲染第二层，即顶层
    pub enable_top: bool,

    /// 背景色，RGBA 四个分量，取值范围 0.0..1.0
    pub back_color: Vec4,
    /// 皮肤类型，决定模型与贴图布局
    pub skin_type: SkinType,

    /// 皮肤贴图
    pub skin_tex: Option<Pixmap>,
    /// 披风贴图
    pub cape: Option<Pixmap>,

    /// 是否已加载披风
    pub have_cape: bool,
    /// 是否已加载皮肤
    pub have_skin: bool,

    /// 模型视距，即缩放距离，越大越远
    pub distance: f32,
    /// 模型平移位置
    pub xy: Vec2,
    /// 模型自转矩阵，由 [`SkinRenderWgpu::rotate`] 累积
    pub last: Mat4,

    /// 手臂旋转角度，单位度
    pub arm_rotate: Vec3,
    /// 腿部旋转角度，单位度
    pub leg_rotate: Vec3,
    /// 头部旋转角度，单位度
    pub head_rotate: Vec3,

    /// 画布宽度，单位像素
    pub width: i32,
    /// 画布高度，单位像素
    pub height: i32,

    /// 渲染档位
    pub render_type: SkinRenderType,
}

impl SkinRenderWgpu {
    /// 创建渲染器，默认画布 800x600、黑色背景、单采样
    ///
    /// 不碰 GPU：设备与管线在 [`SkinRenderWgpu::render`] 里按需取，取不到才报 `NoGpu`。
    pub fn new() -> Self {
        Self {
            enable_cape: false,
            enable_top: false,
            back_color: Vec4::new(0.0, 0.0, 0.0, 1.0),
            skin_type: SkinType::Unknown,
            skin_tex: None,
            cape: None,
            have_cape: false,
            have_skin: false,
            distance: 1.0,
            xy: Vec2::new(0.0, 0.0),
            last: Mat4::default(),
            arm_rotate: Vec3::new(0.0, 0.0, 0.0),
            leg_rotate: Vec3::new(0.0, 0.0, 0.0),
            head_rotate: Vec3::new(0.0, 0.0, 0.0),
            width: 800,
            height: 600,
            render_type: SkinRenderType::Normal,
        }
    }

    /// 按当前参数离屏渲染一帧并读回位图
    ///
    /// 画布尺寸取 [`SkinRenderWgpu::width`] 与 `height`，背景按 `back_color` 清屏；
    /// 绘制顺序为本体、披风，最后是开混合且不写深度的顶层。
    ///
    /// # 返回值
    ///
    /// 返回预乘 BGRA 的位图，内存序与 [`Pixmap`] 一致；没有皮肤、皮肤类型未知、
    /// 画布尺寸非正或读回失败时返回对应的 [`ErrorType`]
    pub fn render(&self) -> Result<Pixmap, ErrorType> {
        let gpu = mml_gpu::device().ok_or(ErrorType::NoGpu)?;
        let pipes = pipelines().ok_or(ErrorType::NoGpu)?;
        let (device, queue) = (gpu.device(), gpu.queue());

        // 没有皮肤或类型未知一律不画
        if !self.have_skin || self.skin_type == SkinType::Unknown {
            return Err(ErrorType::NoSkin);
        }
        if self.width <= 0 || self.height <= 0 {
            return Err(ErrorType::InvalidSize);
        }

        let (width, height) = (self.width as u32, self.height as u32);
        let skin = self.skin_tex.as_ref().ok_or(ErrorType::NoSkin)?;
        let skin_tex =
            mml_gpu::upload_texture(device, queue, skin).ok_or(ErrorType::RenderError)?;
        let cape_tex = self
            .cape
            .as_ref()
            .and_then(|cape| mml_gpu::upload_texture(device, queue, cape));

        // 部件矩阵已经烘进顶点，uniform 里是「投影 * 视图 * 模型」，
        // 模型矩阵带着自转、平移与视距
        let mvp = self.get_matrix(ModelPartType::Proj)
            * self.get_matrix(ModelPartType::View)
            * self.get_matrix(ModelPartType::Model);
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mml-skin-draw uniform"),
            contents: bytemuck::bytes_of(&Uniforms {
                mvp: mvp.to_cols_array_2d(),
            }),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let skin_view = skin_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_skin = mml_gpu::texture_bind_group(
            device,
            &pipes.bind_layout,
            &uniform,
            &pipes.sampler,
            &skin_view,
        );
        let bind_cape = cape_tex.as_ref().map(|cape| {
            let view = cape.create_view(&wgpu::TextureViewDescriptor::default());
            mml_gpu::texture_bind_group(device, &pipes.bind_layout, &uniform, &pipes.sampler, &view)
        });

        // 顶点：本体六件、按开关的披风、按开关的顶层，全部按部件矩阵先转成世界坐标
        let shape = skin_3d_model::get_steve(self.skin_type);
        let tex = skin_3d_texture::get_steve_texture(self.skin_type);
        let (verts, indices) = build_mesh(self, &parts_of(&shape, &tex));
        let body = upload_mesh(device, "mml-skin-draw body", &verts, &indices);

        let cape = if self.have_cape && self.enable_cape {
            let part = PartRef {
                part: ModelPartType::Cape,
                shape: &shape.cape,
                uv: &tex.cape,
            };
            let (verts, indices) = build_mesh(self, &[part]);
            upload_mesh(device, "mml-skin-draw cape", &verts, &indices)
        } else {
            None
        };

        let top = if self.enable_top {
            let shape = skin_3d_model::get_steve_top(self.skin_type);
            let tex = skin_3d_texture::get_steve_texture_top(self.skin_type);
            let (verts, indices) = build_mesh(self, &parts_of(&shape, &tex));
            upload_mesh(device, "mml-skin-draw top", &verts, &indices)
        } else {
            None
        };

        let samples = match self.render_type {
            SkinRenderType::Normal => 1,
            SkinRenderType::MSAA => MSAA_SAMPLES,
        };
        let layer = if samples > 1 {
            &pipes.msaa
        } else {
            &pipes.single
        };

        // 颜色 / 深度附件与 MSAA 解析目标都由 mml-gpu 建；读回取 resolve，单采样时就是 color
        let target = mml_gpu::offscreen_target_samples(device, width, height, samples);
        let color_view = target
            .color
            .create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view = target
            .depth
            .create_view(&wgpu::TextureViewDescriptor::default());
        let resolve_view = target
            .resolve
            .as_ref()
            .map(|texture| texture.create_view(&wgpu::TextureViewDescriptor::default()));

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mml-skin-draw"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mml-skin-draw"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    depth_slice: None,
                    resolve_target: resolve_view.as_ref(),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color(self.back_color)),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            pass.set_bind_group(0, &bind_skin, &[]);
            pass.set_pipeline(&layer.base);

            if let Some(mesh) = &body {
                pass.set_vertex_buffer(0, mesh.verts.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint16);
                pass.draw_indexed(0..mesh.count, 0, 0..1);
            }

            if let (Some(mesh), Some(bind)) = (&cape, &bind_cape) {
                pass.set_bind_group(0, bind, &[]);
                pass.set_vertex_buffer(0, mesh.verts.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint16);
                pass.draw_indexed(0..mesh.count, 0, 0..1);
            }

            if let Some(mesh) = &top {
                pass.set_pipeline(&layer.overlay);
                pass.set_bind_group(0, &bind_skin, &[]);
                pass.set_vertex_buffer(0, mesh.verts.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint16);
                pass.draw_indexed(0..mesh.count, 0, 0..1);
            }
        }
        queue.submit(Some(encoder.finish()));

        // 有 MSAA 时读解析目标，否则直接读颜色附件
        let source = target.resolve.as_ref().unwrap_or(&target.color);
        let premul =
            mml_gpu::readback(device, queue, source, width, height).ok_or(ErrorType::RenderError)?;

        mml_gpu::pixmap_from_premul_rgba(&premul, width, height).ok_or(ErrorType::RenderError)
    }
}

#[cfg(test)]
mod tests {
    use tiny_skia::IntSize;

    use super::*;

    /// 造一张纯色不透明皮肤
    fn skin_bitmap(color: [u8; 4]) -> Pixmap {
        let mut data = vec![0u8; 64 * 64 * 4];
        for px in data.chunks_exact_mut(4) {
            px.copy_from_slice(&color);
        }

        Pixmap::from_vec(data, IntSize::from_wh(64, 64).unwrap()).unwrap()
    }

    /// 取像素的直乘 RGBA
    fn pixel(image: &Pixmap, x: u32, y: u32) -> [u8; 4] {
        let px = image.pixel(x, y).unwrap().demultiply();

        [px.red(), px.green(), px.blue(), px.alpha()]
    }

    /// 造一个渲染器；无可用 GPU 时返回 `None`，用例随之跳过
    fn renderer() -> Option<SkinRenderWgpu> {
        mml_gpu::device()?;

        Some(SkinRenderWgpu::new())
    }

    /// 装载一张纯色新版宽臂皮肤并设好画布
    fn prepare(render: &mut SkinRenderWgpu, size: i32, color: [u8; 4]) {
        render.width = size;
        render.height = size;
        render.skin_type = SkinType::New;
        render.set_skin_tex(&skin_bitmap(color)).unwrap();
    }

    /// 统计画面里非背景色的像素数，背景为不透明黑
    fn drawn_pixels(image: &Pixmap) -> usize {
        (0..image.height())
            .flat_map(|y| (0..image.width()).map(move |x| (x, y)))
            .filter(|(x, y)| pixel(image, *x, *y) != [0, 0, 0, 255])
            .count()
    }

    /// 加载皮肤后应把模型画在背景上：画面中心出现非背景像素，四角仍是背景
    #[test]
    fn render_draws_model_over_background() {
        let Some(mut render) = renderer() else {
            return;
        };
        prepare(&mut render, 128, [80, 160, 80, 255]);

        let image = render.render().expect("有皮肤且尺寸为正时应渲染成功");
        assert_eq!((image.width(), image.height()), (128, 128));

        let drawn = drawn_pixels(&image);
        assert!(drawn > 1000, "模型应覆盖画面中心，实际非背景像素 {drawn}");

        // 四角保持背景色，说明模型没有铺满整张画布
        for (x, y) in [(0, 0), (127, 0), (0, 127), (127, 127)] {
            assert_eq!(pixel(&image, x, y), [0, 0, 0, 255], "({x},{y}) 应是背景");
        }
    }

    /// MSAA 档位也应渲染出模型
    #[test]
    fn render_msaa_draws_model() {
        let Some(mut render) = renderer() else {
            return;
        };
        prepare(&mut render, 64, [200, 60, 60, 255]);
        render.render_type = SkinRenderType::MSAA;

        let image = render.render().expect("MSAA 档位应渲染成功");
        assert_eq!((image.width(), image.height()), (64, 64));

        let drawn = drawn_pixels(&image);
        assert!(drawn > 200, "MSAA 下模型应可见，实际非背景像素 {drawn}");
    }

    /// 自转矩阵必须真的进到画面里：转 90 度后应与正面视图不同
    #[test]
    fn render_applies_rotation() {
        let Some(mut render) = renderer() else {
            return;
        };
        prepare(&mut render, 128, [80, 160, 80, 255]);

        let front = render.render().expect("正面应渲染成功");

        render.rotate(0.0, 90.0);
        let side = render.render().expect("转 90 度后应渲染成功");

        assert_ne!(front.data(), side.data(), "转动后画面必须变化");
    }

    /// 没有皮肤时按 NoSkin 报错
    #[test]
    fn render_without_skin_is_rejected() {
        let Some(render) = renderer() else {
            return;
        };

        assert_eq!(render.render(), Err(ErrorType::NoSkin));
    }

    /// 皮肤类型未知时按 NoSkin 报错，且每次渲染都给同样的答案
    #[test]
    fn render_unknown_skin_is_rejected() {
        let Some(mut render) = renderer() else {
            return;
        };

        // set_skin_tex 只查宽度，类型未知由渲染阶段判
        render
            .set_skin_tex(&skin_bitmap([10, 20, 30, 255]))
            .unwrap();

        assert_eq!(render.render(), Err(ErrorType::NoSkin));
        assert_eq!(render.render(), Err(ErrorType::NoSkin));
    }

    /// 画布尺寸非正时按 InvalidSize 报错
    #[test]
    fn render_zero_size_is_rejected() {
        let Some(mut render) = renderer() else {
            return;
        };
        prepare(&mut render, 0, [0, 0, 255, 255]);

        assert_eq!(render.render(), Err(ErrorType::InvalidSize));
    }
}

