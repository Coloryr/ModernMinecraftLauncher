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
    GPU.get_or_init(|| block_on(init())).as_ref()
}

/// 按系统选择后端回退链逐个尝试初始化，独显优先，全失败返回 `None`：
/// - Windows：DX12 → VK → GL
/// - macOS：Metal → VK → GL
/// - Linux/其他 Unix：VK → GL
async fn init() -> Option<GpuDevice> {
    #[cfg(windows)]
    let chain: &[(&str, wgpu::Backends)] = &[
        ("DX12", wgpu::Backends::DX12),
        ("VULKAN", wgpu::Backends::VULKAN),
        ("GL", wgpu::Backends::GL),
    ];
    #[cfg(target_os = "macos")]
    let chain: &[(&str, wgpu::Backends)] = &[
        ("METAL", wgpu::Backends::METAL),
        ("VULKAN", wgpu::Backends::VULKAN),
        ("GL", wgpu::Backends::GL),
    ];
    #[cfg(all(unix, not(target_os = "macos")))]
    let chain: &[(&str, wgpu::Backends)] = &[
        ("VULKAN", wgpu::Backends::VULKAN),
        ("GL", wgpu::Backends::GL),
    ];
    #[cfg(not(any(windows, unix)))]
    let chain: &[(&str, wgpu::Backends)] = &[("GL", wgpu::Backends::GL)];

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

/// 离屏渲染目标，颜色与深度附件成对创建
pub struct OffscreenTarget {
    /// 颜色附件，格式 Rgba8Unorm，用途 RENDER_ATTACHMENT | COPY_SRC
    pub color: wgpu::Texture,
    /// 深度附件，格式 Depth32Float，仅用于深度测试
    pub depth: wgpu::Texture,
}

/// 创建指定尺寸的离屏渲染目标
///
/// - `device`: 全局 wgpu 逻辑设备
/// - `width`: 目标宽度，单位像素
/// - `height`: 目标高度，单位像素
///
/// # 返回值
///
/// 返回成对的颜色与深度附件
pub fn offscreen_target(device: &wgpu::Device, width: u32, height: u32) -> OffscreenTarget {
    let mk = |format, usage, label| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    };
    OffscreenTarget {
        color: mk(
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            "mml-gpu target",
        ),
        depth: mk(
            wgpu::TextureFormat::Depth32Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
            "mml-gpu depth",
        ),
    }
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
