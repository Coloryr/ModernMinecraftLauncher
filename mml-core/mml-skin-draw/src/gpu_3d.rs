//! 3D 渲染公共管线，wgpu 离屏，头 / 皮肤 3D 共用的顶点格式、管线与渲染流程
//!
//! 设备来自全局 [`mml_gpu::device`]，后端回退链见其文档；管线进程内只建一次。
//! 像素约定：贴图上传 premul BGRA → straight RGBA，读回 premul RGBA → premul
//! BGRA，即 `Pixmap` 内存序，仅交换 R/B；全程预乘语义与 tiny_skia 一致。

use std::sync::OnceLock;

use bytemuck::{Pod, Zeroable};
use glam::Mat4;
use tiny_skia::Pixmap;
use wgpu::util::DeviceExt;

use crate::skin_draw::downsample;

/// 渲染 shader，含底层与顶层两个片元入口
const WGSL: &str = include_str!("gpu_3d.wgsl");

/// 顶点，模型坐标加整张贴图的归一化 uv
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct Vert {
    /// 顶点位置，模型坐标，单位皮肤像素
    pub pos: [f32; 3],
    /// 贴图坐标，已按整张贴图尺寸归一化到 0..1
    pub uv: [f32; 2],
}

/// 立方体一个面的角：相对部件中心的模型坐标偏移加块内 uv
///
/// 顶点顺序与 uv 布局的来龙去脉见 [`crate::skin_3d_draw::face_corners`]。
#[derive(Clone, Copy)]
pub(crate) struct FaceCorner {
    /// 相对部件中心的模型坐标偏移，单位皮肤像素
    pub pos: [f32; 3],
    /// 块内 uv，0..1 比例，v=0 在贴图块顶部
    pub uv: [f32; 2],
}

/// 把一个面的四个角展开为 6 顶点，按三角形 0,1,2 与 0,2,3 排列
///
/// - `corners`: 面四角，见 [`FaceCorner`]
/// - `rect`: 贴图块，按 tx / ty / tw / th 排列，单位像素
/// - `tex_w` / `tex_h`: 贴图宽高，单位像素，HD 皮肤按实际尺寸归一化
///
/// # 返回值
///
/// 返回 6 个顶点，uv 已按贴图尺寸归一化
pub(crate) fn face_verts(
    corners: [FaceCorner; 4],
    rect: [i32; 4],
    tex_w: f32,
    tex_h: f32,
) -> [Vert; 6] {
    let (tx, ty, tw, th) = (
        rect[0] as f32,
        rect[1] as f32,
        rect[2] as f32,
        rect[3] as f32,
    );
    let mut out = [Vert {
        pos: [0.0; 3],
        uv: [0.0; 2],
    }; 6];
    for (i, &ci) in [0usize, 1, 2, 0, 2, 3].iter().enumerate() {
        let c = corners[ci];
        out[i] = Vert {
            pos: c.pos,
            uv: [(tx + c.uv[0] * tw) / tex_w, (ty + c.uv[1] * th) / tex_h],
        };
    }
    out
}

/// 构造像素空间到 NDC 的正交投影矩阵
///
/// - `width`: 目标宽度，单位像素
/// - `height`: 目标高度，单位像素
///
/// # 返回值
///
/// 返回正交投影矩阵，输入为 0..width / 0..height 且 y 向下的像素坐标
///
/// z 像素值越大离观察者越近，深度反向映射，近处深度小，配合 LessEqual 与清 1.0。
pub(crate) fn ortho(width: u32, height: u32) -> Mat4 {
    let (w, h) = (width as f32, height as f32);
    Mat4::from_cols_array(&[
        2.0 / w,
        0.0,
        0.0,
        0.0,
        0.0,
        -2.0 / h,
        0.0,
        0.0,
        0.0,
        0.0,
        -1.0 / 1000.0,
        0.0,
        -1.0,
        1.0,
        0.5,
        1.0,
    ])
}

/// 底层不透明 / cutout 与顶层半透明两条管线及其绑定资源
struct Pipelines {
    /// 底层管线，写深度并剔除背面
    base: wgpu::RenderPipeline,
    /// 顶层 overlay 管线，半透明混合且只读深度
    overlay: wgpu::RenderPipeline,
    /// 绑定组布局：0 号 uniform、1 号采样器、2 号贴图
    bind_layout: wgpu::BindGroupLayout,
    /// 贴图采样器，NEAREST 采样加边缘钳制
    sampler: wgpu::Sampler,
}

/// 管线缓存，设备全局唯一，失败也记入避免反复重试
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
    /// 创建两条管线、绑定组布局与采样器
    fn new(device: &wgpu::Device) -> Pipelines {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mml-skin-3d"),
            source: wgpu::ShaderSource::Wgsl(WGSL.into()),
        });

        // 绑定组布局与采样器是三类离屏渲染共用的形状，统一由 mml-gpu 提供
        let bind_layout = mml_gpu::texture_bind_layout(device, wgpu::ShaderStages::VERTEX);

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mml-skin-3d"),
            bind_group_layouts: &[Some(&bind_layout)],
            immediate_size: 0,
        });

        // 两条管线都写深度：overlay 部件互相穿插，手臂 overlay 内侧面会伸进
        // 身体 overlay，只读深度会退化成按绘制顺序取胜；底层透明镂空
        // 不写深度，overlay 仍能透过镂空显示
        let depth_write = wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        };
        let depth_ro = depth_write.clone();
        let target = wgpu::ColorTargetState {
            format: wgpu::TextureFormat::Rgba8Unorm,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        };

        // 底层剔除背面：部件间存在共面面，头底与身顶、腿顶与身底、臂内与身侧，
        // 不剔除会 z-fighting，背面盖住正面。
        // 顶层不剔除：帽子远侧要从透明纹素后面透出来，与 CPU 参考的无剔除效果一致；
        // 顶层没有共面面，不会 z-fighting
        let mk = |blend: Option<wgpu::BlendState>,
                  depth: wgpu::DepthStencilState,
                  cull: Option<wgpu::Face>,
                  entry: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("mml-skin-3d"),
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
                        ],
                    })],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        blend,
                        ..target.clone()
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    // 面绕序为外侧 CCW，NDC 中 y 向下时屏幕绕序仍为 CCW
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: cull,
                    ..Default::default()
                },
                depth_stencil: Some(depth),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };

        // 顶层：straight-alpha 混合，color 用 SrcAlpha/1-SrcAlpha、alpha 用 One/1-One，
        // 透明底上的累积结果即 premul 语义，与读回后的 Pixmap 一致
        let overlay_blend = Some(wgpu::BlendState {
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

        Pipelines {
            base: mk(None, depth_write, Some(wgpu::Face::Back), "fs_base"),
            overlay: mk(overlay_blend, depth_ro, None, "fs_overlay"),
            bind_layout,
            sampler: mml_gpu::nearest_sampler(device),
        }
    }
}

/// 渲染一组立方体面，超采样后读回再降采样
///
/// - `texture`: 皮肤贴图
/// - `base` / `overlay`: 底层 / 顶层顶点，每面 6 顶点，由 [`face_verts`] 产出
/// - `tran`: 模型变换矩阵，含旋转、缩放与平移到像素空间
/// - `width` / `height`: 输出尺寸，单位像素
/// - `supersample`: 超采样倍数
///
/// # 返回值
///
/// 返回预乘 BGRA 的 `Pixmap`，无可用 GPU 或贴图为空时返回 `None`
pub(crate) fn render_3d(
    texture: &Pixmap,
    base: &[Vert],
    overlay: &[Vert],
    tran: Mat4,
    width: u32,
    height: u32,
    supersample: u32,
) -> Option<Pixmap> {
    let gpu = mml_gpu::device()?;
    let pipes = pipelines()?;
    let (device, queue) = (gpu.device(), gpu.queue());
    let (w, h) = (width * supersample, height * supersample);
    // 正交投影按最终尺寸算：变换矩阵的平移 / 缩放以最终像素为单位，
    // 超采样目标渲染出来正好放大 supersample 倍，降采样后回到最终坐标
    let mvp = ortho(width, height) * tran;

    // 贴图 + uniform + 绑定组
    let tex = mml_gpu::upload_texture(device, queue, texture)?;
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("mml-skin-3d uniform"),
        contents: bytemuck::bytes_of(&Mvp {
            mvp: mvp.to_cols_array_2d(),
        }),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let bind = mml_gpu::texture_bind_group(
        device,
        &pipes.bind_layout,
        &uniform_buf,
        &pipes.sampler,
        &view,
    );

    // 空顶点列表不建缓冲：零长缓冲连 slice 都会 panic，调用方如披风就没有顶层
    let vbase = (!base.is_empty()).then(|| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mml-skin-3d base"),
            contents: bytemuck::cast_slice(base),
            usage: wgpu::BufferUsages::VERTEX,
        })
    });
    let voverlay = (!overlay.is_empty()).then(|| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mml-skin-3d overlay"),
            contents: bytemuck::cast_slice(overlay),
            usage: wgpu::BufferUsages::VERTEX,
        })
    });

    // 离屏渲染：先底层并写深度，后顶层做混合且只读深度
    let target = mml_gpu::offscreen_target(device, w, h);
    let color_view = target
        .color
        .create_view(&wgpu::TextureViewDescriptor::default());
    let depth_view = target
        .depth
        .create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mml-skin-3d"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mml-skin-3d"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &color_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
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
        pass.set_bind_group(0, &bind, &[]);
        if let Some(buf) = &vbase {
            pass.set_pipeline(&pipes.base);
            pass.set_vertex_buffer(0, buf.slice(..));
            pass.draw(0..base.len() as u32, 0..1);
        }
        if let Some(buf) = &voverlay {
            pass.set_pipeline(&pipes.overlay);
            pass.set_vertex_buffer(0, buf.slice(..));
            pass.draw(0..overlay.len() as u32, 0..1);
        }
    }
    queue.submit(Some(encoder.finish()));

    // 读回 premul RGBA，转成 premul BGRA 的 Pixmap，再降采样
    let premul = mml_gpu::readback(device, queue, &target.color, w, h)?;
    let img = mml_gpu::pixmap_from_premul_rgba(&premul, w, h)?;
    downsample(&img, supersample)
}

/// uniform 布局，只含一个 MVP 矩阵
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Mvp {
    /// 模型视图投影矩阵，列主序
    mvp: [[f32; 4]; 4],
}
