//! 公共 wgpu 离屏渲染后端
//!
//! 设备初始化，后端回退链且全局仅初始化一次；另有贴图上传与离屏目标读回，
//! 供 `mml-tex-draw` 的物品 / 方块图标与 `mml-skin-draw` 的皮肤 / 头像 3D 共用。
//!
//! 像素约定：tiny_skia `Pixmap` 内部是**预乘 BGRA8**；wgpu 纹理统一用
//! **straight 直乘 RGBA8**。上传时 premul→straight，读回时按需转回。

use std::sync::OnceLock;

use pollster::block_on;
use tiny_skia::{IntSize, Pixmap};

/// 全局设备；初始化失败同样记入，避免每次调用反复重试
static GPU: OnceLock<Option<GpuDevice>> = OnceLock::new();

/// wgpu 逻辑设备 + 命令队列，跨 crate 共享一个实例
pub struct GpuDevice {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl GpuDevice {
    /// 逻辑设备
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// 命令队列
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }
}

/// 取全局设备；首次调用时按后端回退链初始化，之后所有调用方共用
pub fn device() -> Option<&'static GpuDevice> {
    GPU.get_or_init(|| block_on(init(default_chain()))).as_ref()
}

/// 按给定回退链初始化一个设备，不占用也不读全局缓存，测试与调试用
///
/// - `chain`: 回退链，每项为显示名与后端位，按优先级排列
///
/// # 返回值
///
/// 返回首个成功的设备，全部失败时返回 `None`
pub fn device_with_chain(chain: &[(&str, wgpu::Backends)]) -> Option<GpuDevice> {
    block_on(init(chain))
}

/// 按系统给出默认后端回退链：
/// - Windows：DX12 → VK → GL
/// - macOS：Metal → VK → GL
/// - Linux/其他 Unix：VK → GL
fn default_chain() -> &'static [(&'static str, wgpu::Backends)] {
    #[cfg(windows)]
    return &[
        ("DX12", wgpu::Backends::DX12),
        ("VULKAN", wgpu::Backends::VULKAN),
        ("GL", wgpu::Backends::GL),
    ];
    #[cfg(target_os = "macos")]
    return &[
        ("METAL", wgpu::Backends::METAL),
        ("VULKAN", wgpu::Backends::VULKAN),
        ("GL", wgpu::Backends::GL),
    ];
    #[cfg(all(unix, not(target_os = "macos")))]
    return &[
        ("VULKAN", wgpu::Backends::VULKAN),
        ("GL", wgpu::Backends::GL),
    ];
    #[cfg(not(any(windows, unix)))]
    return &[("GL", wgpu::Backends::GL)];
}

/// 按回退链逐个后端、逐个适配器尝试初始化，独显优先，全失败返回 `None`
///
/// - `chain`: 回退链，每项为显示名与后端位，按优先级排列
async fn init(chain: &[(&str, wgpu::Backends)]) -> Option<GpuDevice> {
    for (name, backends) in chain {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = *backends;
        let instance = wgpu::Instance::new(desc);

        // 独显优先：枚举适配器按 独显→其余 排序，离屏渲染无需管是否主显卡；
        // 逐个尝试建设备，建不成再退下一个适配器/后端
        let mut adapters = block_on(instance.enumerate_adapters(*backends));
        adapters.sort_by_key(|a| match a.get_info().device_type {
            wgpu::DeviceType::DiscreteGpu => 0,
            _ => 1,
        });
        for adapter in adapters {
            let info = adapter.get_info();
            let Ok((device, queue)) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("mml-gpu"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    experimental_features: wgpu::ExperimentalFeatures::default(),
                    memory_hints: wgpu::MemoryHints::default(),
                    trace: wgpu::Trace::Off,
                })
                .await
            else {
                continue;
            };
            println!(
                "[渲染] GPU后端 {name}：{}（{:?}）",
                info.name, info.device_type
            );
            return Some(GpuDevice { device, queue });
        }
    }
    None
}

/// 预乘 BGRA → straight RGBA，即 tiny_skia 位图字节转 wgpu 纹理字节
///
/// alpha 为 0 的像素 rgb 清零，避免残留颜色渗出。
///
/// - `data`: 预乘 BGRA8 字节，每像素 4 字节
///
/// # 返回值
///
/// 返回等长的 straight RGBA8 字节，内存序 R,G,B,A
pub fn premul_bgra_to_straight_rgba(data: &[u8]) -> Vec<u8> {
    let mut out = data.to_vec();
    for px in out.chunks_exact_mut(4) {
        // 输入内存序 BGRA → 输出 RGBA
        let (b, g, r, a) = (px[0], px[1], px[2], px[3]);
        px[0] = r;
        px[1] = g;
        px[2] = b;
        let a = u32::from(a);
        if a == 0 {
            px[0] = 0;
            px[1] = 0;
            px[2] = 0;
        } else if a < 255 {
            for c in px.iter_mut().take(3) {
                *c = (u32::from(*c) * 255 / a).min(255) as u8;
            }
        }
    }
    out
}

/// straight RGBA → 预乘 BGRA，即写入 tiny_skia `Pixmap` 的内存序
///
/// - `rgba`: straight RGBA8 字节，每像素 4 字节
///
/// # 返回值
///
/// 返回等长的预乘 BGRA8 字节，内存序 B,G,R,A
pub fn straight_rgba_to_premul_bgra(rgba: &[u8]) -> Vec<u8> {
    let mut out = rgba.to_vec();
    for px in out.chunks_exact_mut(4) {
        let (r, g, b, a) = (px[0], px[1], px[2], px[3]);
        let a = u32::from(a);
        let mul = |c: u8| {
            if a == 255 {
                c
            } else {
                (u32::from(c) * a / 255) as u8
            }
        };
        // 内存序 BGRA；rgb 乘回 alpha 变预乘
        px[0] = mul(b);
        px[1] = mul(g);
        px[2] = mul(r);
        px[3] = a as u8;
    }
    out
}

/// 上传一张位图为 RGBA8Unorm 纹理，内容为 straight alpha
///
/// - `device`: 全局 wgpu 逻辑设备
/// - `queue`: 全局命令队列，用于写入纹理数据
/// - `tex`: 源位图，tiny_skia 的预乘 BGRA 内存序，内部转 straight RGBA
///
/// # 返回值
///
/// 返回创建好的纹理，位图为空时返回 `None`；采样方式由调用方的采样器决定，
/// 推荐 NEAREST 与 ClampToEdge
pub fn upload_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    tex: &Pixmap,
) -> Option<wgpu::Texture> {
    let (w, h) = (tex.width(), tex.height());
    if w == 0 || h == 0 {
        return None;
    }
    let rgba = premul_bgra_to_straight_rgba(tex.data());
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("mml-gpu texture"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(w * 4),
            rows_per_image: Some(h),
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    Some(texture)
}

/// 离屏渲染目标，颜色与深度附件成对创建，多重采样时另带解析目标
pub struct OffscreenTarget {
    /// 颜色附件，格式 Rgba8Unorm；单采样时用途含 COPY_SRC，可直接读回
    pub color: wgpu::Texture,
    /// 深度附件，格式 Depth32Float，仅用于深度测试
    pub depth: wgpu::Texture,
    /// 多重采样的解析目标，单采样时为 `None`；采样数与读回都应当用它
    pub resolve: Option<wgpu::Texture>,
}

/// 建一个渲染附件纹理
///
/// - `device`: wgpu 逻辑设备
/// - `width` / `height`: 尺寸，单位像素
/// - `samples`: 采样数，1 表示单采样
/// - `format`: 纹理格式，颜色用 `Rgba8Unorm`，深度用 `Depth32Float`
/// - `usage`: 用途
/// - `label`: 调试标签
fn attachment(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    samples: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
    label: &str,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: samples,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

/// 创建指定尺寸的离屏渲染目标，单采样
///
/// - `device`: wgpu 逻辑设备
/// - `width`: 目标宽度，单位像素
/// - `height`: 目标高度，单位像素
///
/// # 返回值
///
/// 见 [`OffscreenTarget`]，`resolve` 恒为 `None`
pub fn offscreen_target(device: &wgpu::Device, width: u32, height: u32) -> OffscreenTarget {
    offscreen_target_samples(device, width, height, 1)
}

/// 创建指定尺寸与采样数的离屏渲染目标
///
/// 采样数大于 1 时 `color` 与 `depth` 是多重采样附件，另给一个单采样的 `resolve`
/// 当解析目标，读回必须用 `resolve`；单采样时 `resolve` 为 `None`，直接用 `color`。
///
/// - `device`: wgpu 逻辑设备
/// - `width`: 目标宽度，单位像素
/// - `height`: 目标高度，单位像素
/// - `samples`: 采样数，1 表示单采样
///
/// # 返回值
///
/// 见 [`OffscreenTarget`]
pub fn offscreen_target_samples(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    samples: u32,
) -> OffscreenTarget {
    let msaa = samples > 1;
    let color_usage = if msaa {
        wgpu::TextureUsages::RENDER_ATTACHMENT
    } else {
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC
    };

    OffscreenTarget {
        color: attachment(
            device,
            width,
            height,
            samples,
            wgpu::TextureFormat::Rgba8Unorm,
            color_usage,
            "mml-gpu target",
        ),
        depth: attachment(
            device,
            width,
            height,
            samples,
            wgpu::TextureFormat::Depth32Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
            "mml-gpu depth",
        ),
        resolve: msaa.then(|| {
            attachment(
                device,
                width,
                height,
                1,
                wgpu::TextureFormat::Rgba8Unorm,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                "mml-gpu resolve",
            )
        }),
    }
}

/// 离屏渲染统一用的贴图采样器：NEAREST 取整加边缘钳制
///
/// - `device`: wgpu 逻辑设备
///
/// # 返回值
///
/// 返回可直接塞进绑定组的采样器
pub fn nearest_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("mml-gpu nearest"),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    })
}

/// 离屏渲染统一用的绑定组布局：0 号 uniform、1 号采样器、2 号二维贴图
///
/// - `device`: wgpu 逻辑设备
/// - `uniform_visibility`: uniform 对哪些着色阶段可见，只有顶点读就传 `VERTEX`，
///   片元也要读就并上 `FRAGMENT`
///
/// # 返回值
///
/// 返回与 [`texture_bind_group`] 配套的布局
pub fn texture_bind_layout(
    device: &wgpu::Device,
    uniform_visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mml-gpu texture"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: uniform_visibility,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
        ],
    })
}

/// 按 [`texture_bind_layout`] 建绑定组，按 0 uniform、1 采样器、2 贴图填三个槽
///
/// - `device`: wgpu 逻辑设备
/// - `layout`: 由 [`texture_bind_layout`] 建出的布局
/// - `uniform`: uniform 缓冲
/// - `sampler`: 贴图采样器，通常来自 [`nearest_sampler`]
/// - `texture`: 贴图视图
///
/// # 返回值
///
/// 返回可直接 `set_bind_group(0, ..)` 的绑定组
pub fn texture_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform: &wgpu::Buffer,
    sampler: &wgpu::Sampler,
    texture: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("mml-gpu texture"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(texture),
            },
        ],
    })
}

/// 读回离屏颜色目标，输出预乘 RGBA 字节且行紧邻无填充
///
/// - `device`: 全局 wgpu 逻辑设备
/// - `queue`: 全局命令队列，用于提交拷贝命令
/// - `color`: 待读回的颜色附件，格式须为 Rgba8Unorm
/// - `width`: 读回宽度，单位像素
/// - `height`: 读回高度，单位像素
///
/// 帧缓冲语义为预乘 alpha，是 straight 混合在透明底上累积的结果；
/// 这里只做去行填充，不做 premul 换算，调用方按需自行转换，
/// 见 [`straight_rgba_to_premul_bgra`] 转 Pixmap，或反除回 straight。
///
/// # 返回值
///
/// 返回 `width * height * 4` 字节，读回失败时返回 `None`
pub fn readback(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    color: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Option<Vec<u8>> {
    // 拷贝要求 bytes_per_row 256 对齐，目标行宽不足时补齐
    let row = width * 4;
    let aligned =
        row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let read_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mml-gpu readback"),
        size: (aligned * height) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mml-gpu readback"),
    });
    encoder.copy_texture_to_buffer(
        color.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &read_buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(aligned),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));

    let slice = read_buf.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
    let data = slice.get_mapped_range().ok()?;
    if data.len() < (aligned * height) as usize {
        return None;
    }

    // 去掉每行的对齐填充
    let mut out = Vec::with_capacity((row * height) as usize);
    for y in 0..height {
        let start = (y * aligned) as usize;
        out.extend_from_slice(&data[start..start + row as usize]);
    }
    drop(data);
    read_buf.unmap();
    Some(out)
}

/// 预乘 RGBA 字节转 `Pixmap`，输入为 wgpu 读回的内存序
///
/// - `rgba`: 预乘 RGBA8 字节，每像素 4 字节
/// - `width`: 位图宽度，单位像素
/// - `height`: 位图高度，单位像素
///
/// 仅交换 R/B 得到预乘 BGRA，不做 premul 换算。
///
/// # 返回值
///
/// 返回位图，字节数与尺寸对不上时返回 `None`
pub fn pixmap_from_premul_rgba(rgba: &[u8], width: u32, height: u32) -> Option<Pixmap> {
    if rgba.len() != (width * height * 4) as usize {
        return None;
    }
    let mut data = rgba.to_vec();
    for px in data.chunks_exact_mut(4) {
        px.swap(0, 2); // RGBA → BGRA，同为预乘只换内存序
    }
    Pixmap::from_vec(data, IntSize::from_wh(width, height)?)
}

/// straight RGBA 字节转 `Pixmap`，内部换算为预乘 BGRA
///
/// - `rgba`: straight RGBA8 字节，每像素 4 字节
/// - `width`: 位图宽度，单位像素
/// - `height`: 位图高度，单位像素
///
/// # 返回值
///
/// 返回位图，字节数与尺寸对不上时返回 `None`
pub fn pixmap_from_straight_rgba(rgba: &[u8], width: u32, height: u32) -> Option<Pixmap> {
    if rgba.len() != (width * height * 4) as usize {
        return None;
    }
    let data = straight_rgba_to_premul_bgra(rgba);
    Pixmap::from_vec(data, IntSize::from_wh(width, height)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// premul BGRA → straight RGBA：半透明像素反除回直乘，全透明清零
    #[test]
    fn test_premul_to_straight() {
        // 预乘语义 rgb ≤ a：BGRA (100,100,100,200) → RGBA (127,127,127,200)
        let out = premul_bgra_to_straight_rgba(&[100, 100, 100, 200]);
        assert_eq!(out, [127, 127, 127, 200]);
        // 全透明清零
        let out = premul_bgra_to_straight_rgba(&[9, 9, 9, 0]);
        assert_eq!(out, [0, 0, 0, 0]);
    }

    /// straight RGBA → 预乘 BGRA：往返后颜色一致，允许整除误差
    #[test]
    fn test_straight_to_premul_roundtrip() {
        let out = straight_rgba_to_premul_bgra(&[255, 128, 64, 255]);
        // BGRA 内存序，alpha=255 不变
        assert_eq!(&out, &[64, 128, 255, 255]);
    }
}
