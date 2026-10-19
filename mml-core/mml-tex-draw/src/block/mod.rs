//! 方块渲染：图标表、数据、渲染管线
//!
//! # 子模块
//!
//! | 模块 | 用途 |
//! |------|------|
//! | [`icons`] | 创造模式图标表：方块 ID、分类、渲染规格 |
//! | [`obj`] | 渲染结果数据结构 BlocksObj 与 ItemsObj |
//! | [`skin`] | 皮肤方块注册 |
pub mod icons;
pub mod obj;
pub mod skin;

use std::{
    cell::RefCell,
    collections::HashMap,
    io::Cursor,
    sync::{
        LazyLock,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
};

use crate::block::icons::{BLOCK_ICONS, IconSpec};
use crate::gpu::GpuCtx;
use crate::model::{BakedModel, BakedModelTextures, Quad, bake_model};

use mml_base::{archives::BaseArchive, serialize_tools};
use mml_game::gui_hook::ProgressGui;
use mml_names::i18_items::error_type::{CoreResult, ErrorType};
use mml_sys::path_helper;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tiny_skia::Pixmap;

/// 输出图片边长，单位像素
const BLOCK_SIZE: i32 = 256;

/// 游戏帧率，单位刻每秒；APNG 帧延迟按 刻数 / FRAME_RATE 换算成秒
const FRAME_RATE: u16 = 20;

/// 平原群系染色色，与 wiki 物品图标一致：草与树叶等灰度贴图按 tintindex 取它
pub(crate) const GRASS_TINT: [u8; 3] = [145, 189, 89]; // #91BD59
/// 树叶染色色，取平原群系色
pub(crate) const FOLIAGE_TINT: [u8; 3] = [119, 171, 47]; // #77AB2F
/// 白桦树叶染色色，游戏内不随群系变化
pub(crate) const BIRCH_TINT: [u8; 3] = [128, 167, 85]; // #80A755
/// 云杉树叶染色色，游戏内不随群系变化
pub(crate) const SPRUCE_TINT: [u8; 3] = [97, 153, 97]; // #619961

/// 解码贴图数据为预乘 RGBA8 位图
///
/// - `data`: PNG 文件字节
///
/// # 返回值
///
/// 返回解码后的位图，解码失败时返回 `None`
pub fn decode_png(data: &[u8]) -> Option<Pixmap> {
    Pixmap::decode_png(data).ok()
}

/// 预乘 RGBA 转 straight alpha，边界处供 GL 与 PNG 用
///
/// 按 a 归一化后就近取整，半值取偶；与 skia 的取值仅个别半值差 1
///
/// - `data`: 预乘 RGBA 像素
///
/// # 返回值
///
/// 返回 straight-alpha RGBA 像素
pub(crate) fn to_straight_rgba(data: &[u8]) -> Vec<u8> {
    let mut out = data.to_vec();

    for px in out.chunks_exact_mut(4) {
        let a = px[3] as u32;
        if a == 0 {
            px[0] = 0;
            px[1] = 0;
            px[2] = 0;
            continue;
        }
        if a == 255 {
            continue;
        }

        for c in px.iter_mut().take(3) {
            *c = (f32::from(*c) * 255.0 / a as f32).round_ties_even() as u8;
        }
    }

    out
}

/// 从竖排动画贴图中取出一帧
///
/// - `tex`: 竖排帧条带，帧为正方形，帧高等于宽
/// - `index`: 帧号，从 0 起
///
/// # 返回值
///
/// 返回该帧位图，帧越界时返回 `None`
fn extract_frame(tex: &Pixmap, index: usize) -> Option<Pixmap> {
    let size = tex.width();
    let top = index as u32 * size;
    let stride = size as usize * 4;

    let mut frame = Pixmap::new(size, size)?;

    for y in 0..size as usize {
        let src = (top as usize + y) * stride;
        let dst = y * stride;
        frame.data_mut()[dst..dst + stride].copy_from_slice(&tex.data()[src..src + stride]);
    }

    Some(frame)
}

/// 时间线上的一段：播放第几帧、持续多久
struct AnimEntry {
    /// 源帧序号，索引 `AnimTimeline::src`；构建期已按帧数取模，不会越界
    frame: u32,
    /// 该段持续刻数，至少 1
    ticks: u32,
}

/// 动画时间线：源帧 + 展开后的帧序列，按刻惰性取帧
///
/// 取帧时才做克隆与插值混合，避免物化整条逐刻帧序列，frametime 大时可达上千帧位图
struct AnimTimeline {
    /// 源帧位图，按条带顺序切出
    src: Vec<Pixmap>,
    /// 展开后的帧序列，按播放顺序排列
    entries: Vec<AnimEntry>,
    /// 帧之间是否平滑插值
    interpolate: bool,
}

impl AnimTimeline {
    /// 由原始帧条带 + mcmeta 元数据构建时间线
    ///
    /// - `tex`: 竖排帧条带
    /// - `meta`: 动画配置，来自 mcmeta
    ///
    /// # 返回值
    ///
    /// 返回时间线，条带切帧失败时返回 `None`
    fn build(tex: &Pixmap, meta: &AnimMeta) -> Option<Self> {
        let count = (tex.height() / tex.width()).max(1) as usize;
        let src: Vec<Pixmap> = (0..count)
            .map(|i| extract_frame(tex, i))
            .collect::<Option<Vec<_>>>()?;
        let frametime = meta.frametime.max(1);
        // frames 列表优先，帧号越界按帧数取模；缺省按 0..n 顺序、每帧 frametime 刻
        let entries = meta
            .frames
            .as_ref()
            .map(|list| {
                list.iter()
                    .map(|f| AnimEntry {
                        frame: f.index % src.len() as u32,
                        ticks: f.time.max(1),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| {
                (0..src.len() as u32)
                    .map(|i| AnimEntry {
                        frame: i,
                        ticks: frametime,
                    })
                    .collect()
            });
        Some(Self {
            src,
            entries,
            interpolate: meta.interpolate,
        })
    }

    /// 整条时间线的总时长，单位刻
    fn total_ticks(&self) -> u32 {
        self.entries.iter().map(|entry| entry.ticks).sum()
    }

    /// 取逐刻时间线上第 tick 刻的帧；interpolate 时向序列下一帧线性过渡
    ///
    /// - `tick`: 游戏刻，调用方需先按 `total_ticks` 取模
    ///
    /// # 返回值
    ///
    /// 返回该刻的帧位图，时间线为空时返回 `None`
    fn frame_at(&self, tick: u32) -> Option<Pixmap> {
        let mut acc = 0u32;
        for (i, entry) in self.entries.iter().enumerate() {
            if tick < acc + entry.ticks {
                let frame = &self.src[entry.frame as usize];
                if !self.interpolate {
                    return Some(frame.clone());
                }
                let next = self.entries[(i + 1) % self.entries.len()].frame as usize;
                let f = (tick - acc) as f32 / entry.ticks as f32;
                return blend_bitmap(frame, &self.src[next], f);
            }
            acc += entry.ticks;
        }
        None
    }
}

/// 按比例线性插值混合两张 RGBA 贴图
///
/// - `a`: 起始帧，f=0 时的结果
/// - `b`: 目标帧，f=1 时的结果
/// - `f`: 混合比例，取值 0..1
///
/// # 返回值
///
/// 返回混合后的位图，尺寸不一致或分配失败时返回 `None`
fn blend_bitmap(a: &Pixmap, b: &Pixmap, f: f32) -> Option<Pixmap> {
    if a.width() != b.width() || a.height() != b.height() {
        return None;
    }

    let mut out = Pixmap::new(a.width(), a.height())?;

    for ((o, x), y) in out
        .data_mut()
        .chunks_exact_mut(4)
        .zip(a.data().chunks_exact(4))
        .zip(b.data().chunks_exact(4))
    {
        for c in 0..4 {
            o[c] = (f32::from(x[c]) * (1.0 - f) + f32::from(y[c]) * f).round() as u8;
        }
    }

    Some(out)
}

/// 把 RGBA 像素编码为 size×size 的 PNG，alpha 直通
///
/// - `size`: 输出边长，单位像素
/// - `rgba`: RGBA 像素，长度等于 size²×4
///
/// # 返回值
///
/// 返回 PNG 字节，编码失败时返回 `None`
fn encode_png(size: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut encoder = png::Encoder::new(&mut cursor, size, size);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
        writer.finish().ok()?;
    }
    Some(cursor.into_inner())
}

/// APNG 的一帧：像素与它占用的显示时长
struct ApngFrame {
    /// 该帧的 RGBA 像素，长度等于边长²×4
    rgba: Vec<u8>,
    /// 该帧持续刻数，含采样步长跨过的刻数；渲染结果与前一帧相同时并入前一帧
    ticks: u16,
}

/// 把多帧合并成 APNG，每帧附带显示时长，单位刻
///
/// - `size`: 输出边长，单位像素
/// - `frames`: 帧列表，按显示顺序排列
///
/// # 返回值
///
/// 返回 APNG 字节，编码失败时返回 `None`
fn encode_apng(size: u32, frames: Vec<ApngFrame>) -> Option<Vec<u8>> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut encoder = png::Encoder::new(&mut cursor, size, size);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        if let Err(e) = encoder.set_animated(frames.len() as u32, 0) {
            log_apng_fail("set_animated", &e);
            return None;
        }
        let mut writer = match encoder.write_header() {
            Ok(writer) => writer,
            Err(e) => {
                log_apng_fail("write_header", &e);
                return None;
            }
        };
        for frame in &frames {
            // 帧显示时长 = 持续刻数 / FRAME_RATE 秒，采样跨过的刻数一并计入
            if let Err(e) = writer.set_frame_delay(frame.ticks, FRAME_RATE) {
                log_apng_fail("set_frame_delay", &e);
                return None;
            }
            if let Err(e) = writer.write_image_data(&frame.rgba) {
                log_apng_fail("write_image_data", &e);
                return None;
            }
        }
        if let Err(e) = writer.finish() {
            log_apng_fail("finish", &e);
            return None;
        }
    }
    Some(cursor.into_inner())
}

/// 记一条 APNG 编码失败日志
///
/// - `stage`: 失败的 png 编码步骤名，如 `set_animated`
/// - `error`: 底层编码错误
fn log_apng_fail(stage: &str, error: &dyn std::fmt::Display) {
    mml_log::error_type(ErrorType::ApngEncodeFail {
        stage: stage.to_string(),
        error: error.to_string(),
    });
}

/// textures 表的值：老版为字符串，新版可为对象，如 glass 的 force_translucent
/// 公开供手动测试调试单个方块用
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum TextureRefObj {
    /// 老版：直接是贴图引用值
    Plain(String),
    /// 新版：对象形式，只取 sprite 字段
    Object { sprite: String },
}

impl TextureRefObj {
    /// 取引用值字符串
    ///
    /// # 返回值
    ///
    /// 返回贴图引用值，如 `block/stone`；两种形态都总有值
    pub(crate) fn value(&self) -> Option<&str> {
        match self {
            Self::Plain(value) => Some(value),
            Self::Object { sprite } => Some(sprite),
        }
    }
}

/// 分阶段耗时统计，纳秒累计；设置 MML_RENDER_PROFILE=1 时在渲染结束后打印
/// 用于定位并发渲染的性能瓶颈，正常路径只有两次 fetch_add
static PROFILE: LazyLock<RenderProfile> = LazyLock::new(|| RenderProfile {
    enabled: std::env::var("MML_RENDER_PROFILE").is_ok(),
    ..Default::default()
});

/// 渲染各阶段耗时累计，仅在 MML_RENDER_PROFILE 设置时输出
#[derive(Default)]
struct RenderProfile {
    /// 是否启用统计，来自环境变量 MML_RENDER_PROFILE
    enabled: bool,
    /// 模型与贴图解析阶段的累计耗时，单位纳秒
    resolve: AtomicU64,
    /// GPU 绘制阶段的累计耗时，单位纳秒
    draw: AtomicU64,
    /// 出图写盘阶段的累计耗时，单位纳秒
    write: AtomicU64,
}

impl RenderProfile {
    /// 打印各阶段耗时统计，未启用时直接返回
    fn print(&self, total: std::time::Duration) {
        if !self.enabled {
            return;
        }
        let f = |nanos: u64| nanos as f64 / 1e9;
        println!(
            "[性能] 总计{:.1}s | resolve {:.1}s | draw {:.1}s | write {:.1}s | 其他 {:.1}s",
            total.as_secs_f64(),
            f(self.resolve.load(Ordering::Relaxed)),
            f(self.draw.load(Ordering::Relaxed)),
            f(self.write.load(Ordering::Relaxed)),
            total.as_secs_f64()
                - f(self.resolve.load(Ordering::Relaxed))
                - f(self.draw.load(Ordering::Relaxed))
                - f(self.write.load(Ordering::Relaxed)),
        );
    }
}

/// 追加头颅立方体的 6 个面到 `quads`，照 SkullModel 的 head 部件
///
/// 模型空间 y 轴向下，8×8×8 立方体，x/z ∈ ±4、y ∈ -8..0；帽子层同尺寸外扩 0.25 像素、
/// 贴图偏移 (32,0)。摆放照 SkullBlockRenderer 绕 X 轴 180° 翻转后居中，头落在方块内
/// 4..12 / 0..8 / 4..12 像素处；纯旋转不改变绕序，无需重排 winding
///
/// uv 矩形取自各面在贴图上的展开位置：右(0,8) 前(8,8) 左(16,8) 后(24,8) 顶(8,0) 底(16,0)；
/// 每面 4 个顶点按索引取 [0]右上 [1]左上 [2]左下 [3]右下
///
/// - `quads`: 输出 quad 列表，就地追加
/// - `tex`: 头颅贴图，皮肤头部展开布局
/// - `path`: 贴图相对路径，写入 quad
/// - `tex_off`: 该层贴图在图上的偏移，单位像素，基础层 (0,0)、帽子层 (32,0)
/// - `grow`: 外扩距离，单位像素，基础层为 0
fn push_head_cube(quads: &mut Vec<Quad>, tex: &Pixmap, path: &str, tex_off: [f32; 2], grow: f32) {
    let (tw, th) = (tex.width() as f32, tex.height() as f32);
    let min = [-4.0 - grow, -8.0 - grow, -4.0 - grow];
    let max = [4.0 + grow, 0.0 + grow, 4.0 + grow];

    // 立方体三边都是 8：uv 展开的各列与行位置 u1..u4、v1..v2
    let (u, v) = (tex_off[0], tex_off[1]);
    let (u1, u2, u3, u4) = (u + 8.0, u + 16.0, u + 24.0, u + 32.0);
    let (v1, v2) = (v + 8.0, v + 16.0);

    // 顶点：t* 在 z=min 面、l* 在 z=max 面，照 ModelPart.Cube 的命名
    let t0 = [min[0], min[1], min[2]];
    let t1 = [max[0], min[1], min[2]];
    let t2 = [max[0], max[1], min[2]];
    let t3 = [min[0], max[1], min[2]];
    let l0 = [min[0], min[1], max[2]];
    let l1 = [max[0], min[1], max[2]];
    let l2 = [max[0], max[1], max[2]];
    let l3 = [min[0], max[1], max[2]];

    // (顶点, 模型空间法线, uv矩形[left,top,right,bottom])，面顺序同 ModelPart.Cube
    let faces: [([[f32; 3]; 4], [f32; 3], [f32; 4]); 6] = [
        ([l1, l0, t0, t1], [0.0, -1.0, 0.0], [u1, v, u2, v1]), // 底
        ([t2, t3, l3, l2], [0.0, 1.0, 0.0], [u2, v1, u3, v]),  // 顶（矩形上下颠倒）
        ([t0, l0, l3, t3], [-1.0, 0.0, 0.0], [u, v1, u1, v2]), // 西
        ([t1, t0, t3, t2], [0.0, 0.0, -1.0], [u1, v1, u2, v2]), // 北
        ([l1, t1, t2, l2], [1.0, 0.0, 0.0], [u2, v1, u3, v2]), // 东
        ([l0, l1, l2, l3], [0.0, 0.0, 1.0], [u3, v1, u4, v2]), // 南
    ];

    for (verts, normal, rect) in faces {
        let (left, top, right, bottom) = (rect[0], rect[1], rect[2], rect[3]);
        // 顶点 i 的 uv 角照 Polygon 构造：[0]→(right,top) [1]→(left,top) [2]→(left,bottom) [3]→(right,bottom)
        let corners = [[right, top], [left, top], [left, bottom], [right, bottom]];
        let pos = verts.map(|p| [p[0] / 16.0 + 0.5, -p[1] / 16.0, -p[2] / 16.0 + 0.5]);
        let uv = corners.map(|c| [c[0] / tw, c[1] / th]);
        let uv_rect = [
            left.min(right) / tw,
            top.min(bottom) / th,
            left.max(right) / tw,
            top.max(bottom) / th,
        ];
        quads.push(Quad {
            pos,
            uv,
            normal: [normal[0], -normal[1], -normal[2]],
            // 头颅贴图无 tint：皮肤颜色已烘在贴图里
            color: [[1.0; 4]; 4],
            tex: path.to_string(),
            translucent: crate::model::rect_translucent(tex, None, &uv_rect),
            fullbright: false,
        });
    }
}

/// 烘焙一个头颅为 quad 列表：贴图、立方体几何与图标 gui 变换
///
/// gui 变换取 item/template_skull 的 display.gui：旋转 30/45/0、平移 0/3/0、缩放 1，
/// 与创造栏视角一致；模型空间已居中，无需再适配画布
///
/// - `archive`: 客户端 jar 归档，读取头颅贴图
/// - `tex_rel`: jar 内贴图相对路径，如 `entity/zombie/zombie`
/// - `hat`: 是否带帽子层；玩家头与僵尸头为 true，贴图须为 64×64
/// - `tex_cache`: 跨模型复用的贴图缓存，路径 → 解码位图
///
/// # 返回值
///
/// 见 [`BakedModelTextures`]；贴图读取失败时返回 `None`
fn bake_head(
    archive: &BaseArchive,
    tex_rel: &str,
    hat: bool,
    tex_cache: &mut HashMap<String, Pixmap>,
) -> Option<BakedModelTextures> {
    let path = crate::model::texture_path(tex_rel)?;
    let tex = crate::model::load_texture(archive, tex_cache, &path)?;

    // 帽子层取自 humanoidHeadLayer，其贴图是 64×64；mobHeadLayer 的 64×32 也"放得下"
    // texOffs(32,0)，但取到的是头部侧面，故按贴图高度兜底断言表的 hat 标记
    debug_assert!(
        !hat || tex.height() >= 64,
        "{tex_rel} 标了帽子层，但贴图 {}×{} 不是 humanoidHeadLayer 的 64×64",
        tex.width(),
        tex.height()
    );

    let mut textures = HashMap::new();
    textures.insert(path.clone(), tex.clone());
    Some(BakedModelTextures {
        model: bake_head_model(tex, hat, &path),
        textures,
    })
}

/// 头颅烘焙核心：从已加载的贴图直接出模型，外部皮肤文件的图标渲染也走这里
///
/// - `tex`: 头颅贴图
/// - `hat`: 是否带帽子层
/// - `path`: 贴图相对路径，写入 quad
///
/// # 返回值
///
/// 返回烘焙产物，基础层 6 面，带帽子层时再加 6 面
fn bake_head_model(tex: Pixmap, hat: bool, path: &str) -> BakedModel {
    let mut quads = Vec::new();
    push_head_cube(&mut quads, &tex, path, [0.0, 0.0], 0.0);
    if hat {
        push_head_cube(&mut quads, &tex, path, [32.0, 0.0], 0.25);
    }

    BakedModel {
        quads,
        // 头颅是 3D 模型，走 ITEMS_3D 侧向光照，同游戏
        gui_light_3d: true,
        transform: crate::model::GuiTransform {
            rotation: [30.0, 45.0, 0.0],
            // display 的平移单位是1/16方块，此处已按 GuiTransform 约定换算
            translation: [0.0, 3.0 / 16.0, 0.0],
            scale: [1.0; 3],
        },
    }
}

/// 渲染并写盘完成的一个方块图标
struct IconOutput {
    /// 方块 / 图标 id，带命名空间，如 `minecraft:stone`
    id: String,
    /// 输出文件名，`:` 已换成 `_`，如 `minecraft_stone.png`
    out_name: String,
}

/// 渲染一个方块图标，对应 block_icons 表的一行
///
/// 各规格的渲染方式：
/// - Model：等轴测渲染 3D 模型
/// - IsoModel：无 display 的模型强制按标准 gui 旋转等轴测渲染
/// - Composite：多模型按 transformation 拼合，如床与门
/// - Form：特殊形态，按内层规格渲染；ID 非真实方块名，注册由 render_blocks 跳过
/// - Head：头颅立方体，手工几何，见 `bake_head`
/// - Skip：实体渲染，如箱子、旗帜、潜影盒，跳过
///
/// - `gpu`: GPU 上下文，`None` 时由 `render_baked` 内部回退
/// - `archive`: 客户端 jar 归档，只读
/// - `tex_cache`: 跨模型复用的贴图缓存，路径 → 解码位图
/// - `id_rel`: 表条目 ID，带命名空间，如 `minecraft:stone`
/// - `spec`: 渲染规格
///
/// # 返回值
///
/// 见 [`IconOutput`]；失败或规格为 Skip 时返回 `None`
fn render_icon(
    gpu: Option<&GpuCtx>,
    archive: &BaseArchive,
    tex_cache: &mut HashMap<String, Pixmap>,
    id_rel: &str,
    spec: &IconSpec,
) -> Option<IconOutput> {
    // 表 ID 带命名空间 minecraft:xx，为 mod 支持预留；输出文件名把 ':' 换成 '_'
    let id = id_rel;
    let out_name = format!("{}.png", id_rel.replace(':', "_"));
    let data = match spec {
        IconSpec::Skip => return None,
        IconSpec::Form(_, inner) => return render_icon(gpu, archive, tex_cache, id_rel, inner),
        IconSpec::Model(rel) => {
            // 烘焙失败即跳过：无 elements 的父模板、贴图引用无值
            let BakedModelTextures { model, textures } = bake_model(archive, rel, tex_cache)?;
            render_baked(gpu, archive, model, textures, None)?
        }
        IconSpec::IsoModel(rel) => {
            // 模型无 display 变换，游戏创造栏对这些走平面贴图；按需求强制标准方块 gui 旋转
            // 即 block/block 的 gui display，出等轴测 3D 观感
            let BakedModelTextures {
                mut model,
                textures,
            } = bake_model(archive, rel, tex_cache)?;
            model.transform = crate::model::GuiTransform {
                rotation: [30.0, 225.0, 0.0],
                translation: [0.0; 3],
                scale: [0.625; 3],
            };
            render_baked(gpu, archive, model, textures, None)?
        }
        IconSpec::Head(tex_rel, hat) => {
            let BakedModelTextures { model, textures } =
                bake_head(archive, tex_rel, *hat, tex_cache)?;
            render_baked(gpu, archive, model, textures, None)?
        }
        IconSpec::Composite(parts) => {
            // 各 part 独立烘焙，quad 按 transformation 先绕 Y 轴旋转再平移后拼合
            // 空间为 0..1，1.0 = 1 方块；gui 变换与光照取首个 part，床与门各 part 相同
            let Some((first, rest)) = parts.split_first() else {
                return None;
            };
            let BakedModelTextures {
                mut model,
                mut textures,
            } = bake_model(archive, first.0, tex_cache)?;
            transform_composite_part(&mut model, first.1, first.2);
            for (rel, tr, yaw) in rest {
                let BakedModelTextures {
                    model: mut part,
                    textures: texs,
                } = bake_model(archive, rel, tex_cache)?;
                textures.extend(texs);
                transform_composite_part(&mut part, *tr, *yaw);
                model.quads.extend(part.quads);
            }
            // 无 display 变换的拼合模型如门，强制标准方块 gui 旋转出等轴测 3D 观感
            // 有 display 的床保持原变换
            if model.transform.rotation == [0.0, 0.0, 0.0] {
                model.transform.rotation = [30.0, 225.0, 0.0];
            }
            // 多格拼合模型超出单格 gui 变换范围，按自身投影包围盒适配画布
            fit_composite(&mut model);
            render_baked(gpu, archive, model, textures, None)?
        }
    };

    path_helper::write_bytes(&crate::get_block_dir()?.join(&out_name), &data).ok()?;
    Some(IconOutput {
        id: id.to_string(),
        out_name,
    })
}

/// 把动画贴图条带换成首帧
///
/// quad 的 uv 是单帧空间 16×16，带 mcmeta 的贴图却是竖排帧条带，如 campfire_fire 为
/// 16×128，直接渲染会把所有帧压在一张面上；图标管线 `render_baked` 内部会调用，
/// 其它直接调渲染器的调用方同样要调用
///
/// - `textures`: 模型用到的贴图，路径 → 解码位图，就地替换竖排条带为首帧
pub fn use_first_frame(textures: &mut HashMap<String, Pixmap>) {
    for path in textures.keys().cloned().collect::<Vec<_>>() {
        let Some(strip) = textures.get(&path) else {
            continue;
        };
        if strip.height() > strip.width() {
            let first = extract_frame(strip, 0).unwrap_or_else(|| strip.clone());
            textures.insert(path.clone(), first);
        }
    }
}

/// 渲染烘焙模型出图：贴图带动画时按时间线逐帧渲染合成 APNG，静态模型单帧出图
///
/// 动画贴图按刻等距采样，渲染结果相同的相邻帧合并延迟；`glint` 为 Some 时叠加附魔光效
///
/// - `gpu`: GPU 上下文，`None` 时返回 `None`，单帧渲染仅 GPU 一条路径
/// - `archive`: 客户端 jar 归档，读取动画 mcmeta
/// - `model`: 烘焙后的模型
/// - `textures`: 模型用到的贴图，路径 → 解码位图
/// - `glint`: 附魔光效贴图，无则 `None`
///
/// # 返回值
///
/// 返回 PNG 或 APNG 字节，渲染或编码失败时返回 `None`
pub(crate) fn render_baked(
    gpu: Option<&GpuCtx>,
    archive: &BaseArchive,
    model: BakedModel,
    mut textures: HashMap<String, Pixmap>,
    glint: Option<&Pixmap>,
) -> Option<Vec<u8>> {
    // 动画贴图：时间线必须用**原始条带**建，所以要在换首帧之前
    let mut timelines: Vec<(String, AnimTimeline)> = Vec::new();
    for path in textures.keys().cloned().collect::<Vec<_>>() {
        let strip = textures.get(&path)?;
        if strip.height() > strip.width() {
            let meta = read_anim_meta(archive, &path);
            timelines.push((path.clone(), AnimTimeline::build(strip, &meta)?));
        }
    }

    // 渲染上传用首帧，quad 的 uv 是单帧空间
    use_first_frame(&mut textures);
    if timelines.is_empty() {
        return encode_png(
            BLOCK_SIZE as u32,
            &render_once(gpu, &model, &textures, glint)?,
        );
    }

    // 最多渲染 MAX_APNG_FRAMES 帧，对逐刻时间线等距采样，
    // 采样步长代表的持续刻数并入 APNG 帧延迟；渲染结果相同的相邻帧合并延迟
    let totals: Vec<u32> = timelines.iter().map(|(_, t)| t.total_ticks()).collect();
    let total = totals.iter().copied().max()?;
    const MAX_APNG_FRAMES: usize = 64;
    let stride = total.div_ceil(MAX_APNG_FRAMES as u32);

    let mut frames: Vec<ApngFrame> = Vec::with_capacity((total / stride + 1) as usize);
    let mut k: u32 = 0;
    while k < total {
        for (j, (path, tl)) in timelines.iter().enumerate() {
            let frame = tl.frame_at(k % totals[j])?;
            textures.insert(path.clone(), frame);
        }
        let img = render_once(gpu, &model, &textures, None)?;
        let ticks = stride.min(total - k) as u16;
        match frames.last_mut() {
            // 渲染结果与上一帧相同：并入其显示时长，不重复存帧
            Some(last) if last.rgba == img => last.ticks += ticks,
            _ => frames.push(ApngFrame { rgba: img, ticks }),
        }
        k += stride;
    }
    encode_apng(BLOCK_SIZE as u32, frames)
}

/// composite part 变换：绕 Y 轴旋转让门板朝向镜头，再平移；法线同角旋转
///
/// - `model`: 待变换的烘焙产物，就地修改顶点与法线
/// - `tr`: 平移，0..1 模型空间，1.0 = 1 方块
/// - `yaw_deg`: 绕 Y 轴旋转角，单位度
fn transform_composite_part(model: &mut BakedModel, tr: [f32; 3], yaw_deg: f32) {
    let (s, c) = yaw_deg.to_radians().sin_cos();
    for quad in &mut model.quads {
        for p in &mut quad.pos {
            let (x, z) = (p[0], p[2]);
            p[0] = x * c + z * s + tr[0];
            p[1] += tr[1];
            p[2] = -x * s + z * c + tr[2];
        }
        let (nx, nz) = (quad.normal[0], quad.normal[2]);
        quad.normal[0] = nx * c + nz * s;
        quad.normal[2] = -nx * s + nz * c;
    }
}

/// 多格拼合模型如床适配画布：按合并后 quad 的投影包围盒等比缩放居中
///
/// 单格模型不改 gui 变换，游戏图标按完整方块尺度渲染，半砖等矮模型不放大
///
/// - `model`: 待适配的烘焙产物，就地修改 gui 变换
pub(crate) fn fit_composite(model: &mut BakedModel) {
    const MARGIN: f32 = 2.0;
    let slot = BLOCK_SIZE as f32;
    let inner = slot - MARGIN * 2.0;

    // gui 变换只取旋转，即平移与缩放置零后的线性部分
    let rot = crate::gpu::item_transform_matrix(&crate::model::GuiTransform {
        rotation: model.transform.rotation,
        translation: [0.0; 3],
        scale: [1.0; 3],
    });

    // 投影全部角点：rot 含 T(-0.5)，输出即相对画布中心的归一化坐标，y 向下
    let mut min_q = glam::Vec3::splat(f32::INFINITY);
    let mut max_q = glam::Vec3::splat(f32::NEG_INFINITY);
    for quad in &model.quads {
        for v in &quad.pos {
            let p = rot.transform_point3(glam::Vec3::from(*v));
            min_q = min_q.min(p);
            max_q = max_q.max(p);
        }
    }

    // 等比缩放至长边贴合画布，平移使包围盒中心落在画布中心
    let k = (inner / slot) / (max_q.x - min_q.x).max(max_q.y - min_q.y);
    model.transform.scale = [k, k, k];
    model.transform.translation = [
        -k * (min_q.x + max_q.x) / 2.0,
        -k * (min_q.y + max_q.y) / 2.0,
        0.0,
    ];
}

/// 单帧渲染，仅 GPU
///
/// 不提供 CPU 软件光栅化回退：那条路径用画家算法排序，遇到互相穿插的几何会错前后关系，
/// 如篝火的火焰斜插在原木之间，结果与 GPU 明显不一致，宁可在无 GPU 时直接报错
///
/// - `gpu`: GPU 上下文，`None` 时直接失败
/// - `model`: 烘焙后的模型
/// - `textures`: 模型用到的贴图，路径 → 解码位图
/// - `glint`: 附魔光效贴图，无则 `None`
///
/// # 返回值
///
/// 返回 straight-alpha RGBA 像素，无 GPU 或渲染失败时返回 `None`
fn render_once(
    gpu: Option<&GpuCtx>,
    model: &BakedModel,
    textures: &HashMap<String, Pixmap>,
    glint: Option<&Pixmap>,
) -> Option<Vec<u8>> {
    gpu.and_then(|ctx| ctx.render(model, textures, glint, BLOCK_SIZE as u32))
}

/// 一个渲染完成的方块图标
struct RenderedIcon {
    /// 方块 / 图标 id
    id: String,
    /// 渲染输出的文件名
    out_name: String,
    /// 所属分类，来自 `BLOCK_ICONS`
    cat: String,
}

/// 按创造模式图标表渲染全部方块
///
/// 渲染清单与分类来自 `block_icons::BLOCK_ICONS`，是生成脚本从 26.2 客户端 jar 提取、
/// 26.3 增量更新后手工维护的固定表；下载流程见 lib 的 `load_blocks`，这里只做解包渲染。
/// 渲染按 CPU 核数经 rayon 并发，各线程持有只读 jar 句柄与贴图缓存，完成后单线程汇总
///
/// - `archive`: 客户端 jar 归档，只读
/// - `gui`: 进度回调，按已处理的表条目数上报，传 `None` 不上报
///
/// # 返回值
///
/// 输出目录未初始化或 GPU 不可用时返回相应错误，成功返回 `Ok(())`
pub fn render_blocks(archive: &BaseArchive, gui: ProgressGui) -> CoreResult<()> {
    let render_start = std::time::Instant::now();
    // 输出目录未初始化时报错；各图标写盘前也各自取一次
    crate::get_block_dir().ok_or(ErrorType::DownloadFileFail)?;

    // GPU 上下文按平台条件编译选择后端，DX12→VK→GL，创建一次跨线程复用
    // 图标渲染只有这一条路径，全不可用时直接报错
    let Some(gpu) = GpuCtx::try_new() else {
        return Err(ErrorType::GpuNotAvailable);
    };

    // 提取语言文件并构建 ID->语言键 映射
    let names = crate::extract_langs(archive);

    let total = BLOCK_ICONS.len();
    let done = AtomicUsize::new(0);
    // 每1%左右上报一次，避免高频回调刷爆 GUI
    let step = (total / 100).max(1);

    // 每个工作线程的独立资源，随线程存活跨 job 复用：jar 只读句柄用 thread_local 保证
    // 每线程只开一次，map_init 的 init 在工作被偷取时会按 job 重跑；贴图缓存避免反复解码
    thread_local! {
        static LOCAL_ARCHIVE: RefCell<Option<Option<BaseArchive>>> = const { RefCell::new(None) };
        static TEX_CACHE: RefCell<HashMap<String, Pixmap>> = RefCell::new(HashMap::new());
    }

    // 本轮取消令牌：取消后跳过剩余条目，进度随之停住；已写出的 PNG 留着
    // 下次全量渲染覆盖，与渲染失败时的残留一致
    let cancel = crate::cancel_token();

    // 并发渲染
    let rendered: Vec<Option<RenderedIcon>> = BLOCK_ICONS
        .par_iter()
        .map(|(id_rel, cat, spec)| {
            if cancel.is_cancelled() {
                return None;
            }
            LOCAL_ARCHIVE.with(|local_archive| {
                TEX_CACHE.with(|tex_cache| {
                    let mut tex_cache = tex_cache.borrow_mut();
                    let mut local_archive = local_archive.borrow_mut();
                    let archive = local_archive
                        .get_or_insert_with(|| BaseArchive::open_readonly(archive.path()).ok())
                        .as_ref()
                        .unwrap_or(archive);

                    // 单个表条目的处理，进度计数对跳过的条目也要生效
                    let item_start = std::time::Instant::now();
                    let out = render_icon(Some(&gpu), archive, &mut tex_cache, id_rel, spec);

                    // 定位异常慢的单方块，如超大动画贴图
                    if PROFILE.enabled {
                        let secs = item_start.elapsed().as_secs_f64();
                        if secs > 1.0 {
                            println!("[性能] 慢方块 {id_rel}：{secs:.1}s");
                        }
                    }

                    // 处理完一个计一个，含跳过的；每跨过 step 阈值上报一次
                    let now = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if now % step == 0
                        && let Some(gui) = &gui
                    {
                        gui.set_progress_now(now, Some(total));
                    }

                    // Form 形态只出图不注册：ID 非真实方块名，不进 blocks() 列表
                    out.filter(|_| !matches!(spec, IconSpec::Form(..)))
                        .map(|icon| RenderedIcon {
                            id: icon.id,
                            out_name: icon.out_name,
                            cat: cat.to_string(),
                        })
                })
            })
        })
        .collect();

    PROFILE.print(render_start.elapsed());

    // 取消：本轮作废。版本号由调用方在返回 Ok 后才写入，这里提前返回
    // 不会把半成品标成「已渲染」
    if cancel.is_cancelled() {
        return Err(ErrorType::TaskCancel);
    }

    // 完成时补一次 100%，避免进度停在最后一个 step 前
    if let Some(gui) = &gui {
        gui.set_progress_now(total, Some(total));
    }

    let mut blocks = crate::blocks_write();
    for icon in rendered.into_iter().flatten() {
        blocks.tex.insert(icon.id.clone(), icon.out_name);
        blocks.cat.insert(icon.id.clone(), icon.cat);
        if let Some(lang_key) = names.block.get(&icon.id) {
            blocks.name.insert(icon.id, lang_key.clone());
        }
    }

    Ok(())
}

/// 动画贴图配置
#[derive(Clone)]
pub struct AnimMeta {
    /// 帧间隔，单位游戏刻，1 刻 = 50ms
    pub frametime: u32,
    /// 帧之间是否平滑插值
    pub interpolate: bool,
    /// mcmeta 的 frames 自定义帧序列，缺省按 0..n 顺序展开
    pub frames: Option<Vec<AnimFrame>>,
}

/// 帧序列的一项：帧号与该帧持续刻数，缺省时长为 frametime
#[derive(Clone, Copy)]
pub struct AnimFrame {
    /// 帧号，从 0 起，越界按帧数取模
    pub index: u32,
    /// 该帧持续刻数，单位刻
    pub time: u32,
}

/// mcmeta 文件顶层，JSON 镜像
#[derive(Serialize, Deserialize, Default)]
struct TextureMetaObj {
    /// animation 节点，非动画贴图没有
    animation: Option<TextureAnimationObj>,
}

/// mcmeta 的 animation 节点，JSON 镜像
#[derive(Serialize, Deserialize, Default)]
struct TextureAnimationObj {
    /// 帧间隔，单位刻
    frametime: Option<u32>,
    /// 帧之间是否平滑插值
    interpolate: Option<bool>,
    /// 自定义帧序列：数字项为帧号，对象项为{index, time}
    frames: Option<Vec<serde_json::Value>>,
}

/// 读取动画贴图配置：帧间隔、是否插值、自定义帧序列
///
/// - `archive`: 客户端 jar 归档，读取 `.mcmeta`
/// - `tex_path`: jar 内贴图路径
///
/// # 返回值
///
/// 返回动画配置；无 mcmeta 或解析失败时返回缺省配置，frametime 为 1 且不插值
pub fn read_anim_meta(archive: &BaseArchive, tex_path: &str) -> AnimMeta {
    let fallback = AnimMeta {
        frametime: 1,
        interpolate: false,
        frames: None,
    };
    let Ok(data) = archive.read(&format!("{tex_path}.mcmeta")) else {
        return fallback;
    };
    let Ok(buf) = String::from_utf8(data) else {
        return fallback;
    };
    let Ok(meta) = serialize_tools::json_from_str::<TextureMetaObj>(&buf) else {
        return fallback;
    };
    let Some(animation) = meta.animation else {
        return fallback;
    };
    let frametime = animation.frametime.filter(|t| *t > 0).unwrap_or(1);
    let frames = animation.frames.map(|list| {
        list.into_iter()
            .filter_map(|f| match f {
                serde_json::Value::Number(n) => n.as_u64().map(|i| AnimFrame {
                    index: i as u32,
                    time: frametime,
                }),
                serde_json::Value::Object(o) => {
                    let index = o.get("index").and_then(|v| v.as_u64())? as u32;
                    let time = o
                        .get("time")
                        .and_then(|v| v.as_u64())
                        .filter(|t| *t > 0)
                        .unwrap_or(frametime as u64) as u32;
                    Some(AnimFrame { index, time })
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    });
    AnimMeta {
        frametime,
        interpolate: animation.interpolate.unwrap_or(false),
        frames: frames.filter(|f| !f.is_empty()),
    }
}

/// 当前已渲染的游戏版本
pub(crate) fn mml_tex_draw_id() -> String {
    crate::blocks_read().id.clone()
}

#[cfg(test)]
mod icon_table_tests {
    use super::*;
    use std::collections::HashSet;

    use crate::block::icons::{SpecialForm, special_form};

    /// 固定表完整性：方块 ID 唯一、分类非空、各规格内容非空
    #[test]
    fn icon_table_consistent() {
        assert!(BLOCK_ICONS.len() > 800, "表应覆盖全部创造栏方块");
        let mut seen = HashSet::new();
        for (id, cat, spec) in BLOCK_ICONS {
            assert!(seen.insert(*id), "重复方块ID：{id}");
            assert!(!cat.is_empty(), "{id} 分类为空");
            match spec {
                IconSpec::Model(rel) | IconSpec::IsoModel(rel) => {
                    assert!(!rel.is_empty(), "{id} 模型名为空")
                }
                IconSpec::Composite(parts) => assert!(!parts.is_empty(), "{id} composite无part"),
                // 头颅：贴图路径非空；带帽子层的贴图必须是 humanoidHeadLayer 的 64×64
                IconSpec::Head(tex, _) => assert!(!tex.is_empty(), "{id} 头颅贴图为空"),
                // Form形态：ID必须以形态后缀结尾，内层规格不能是Skip/Form
                IconSpec::Form(form, inner) => {
                    assert!(
                        id.ends_with(&format!("_{}", form.suffix())),
                        "{id} 形态后缀与SpecialForm不符"
                    );
                    assert!(
                        matches!(
                            inner,
                            IconSpec::Model(_) | IconSpec::IsoModel(_) | IconSpec::Composite(_)
                        ),
                        "{id} Form内层规格非法"
                    );
                }
                IconSpec::Skip => {}
            }
        }
    }

    /// 抽查关键行：渲染规格与分类应与游戏创造栏一致
    #[test]
    fn icon_table_spot_checks() {
        let table: HashMap<&str, (&str, &IconSpec)> = BLOCK_ICONS
            .iter()
            .map(|(id, cat, spec)| (*id, (*cat, spec)))
            .collect();
        assert!(matches!(
            table["minecraft:stone"].1,
            IconSpec::Model("block/stone")
        ));
        assert_eq!(table["minecraft:stone"].0, "buildingBlocks");
        assert!(matches!(
            table["minecraft:white_bed"].1,
            IconSpec::Composite(_)
        ));
        // special 实体渲染跳过
        assert!(matches!(table["minecraft:chest"].1, IconSpec::Skip));
        // 头颅走手工立方体几何：骷髅头单层、玩家头带帽子层
        assert!(matches!(
            table["minecraft:skeleton_skull"].1,
            IconSpec::Head(_, false)
        ));
        assert!(matches!(
            table["minecraft:player_head"].1,
            IconSpec::Head(_, true)
        ));
        // 原木在游戏的建筑方块栏
        assert_eq!(table["minecraft:oak_log"].0, "buildingBlocks");
        assert_eq!(table["minecraft:grass_block"].0, "natural");
    }

    /// 特殊形态：基础ID应能查到形态，且对应"基础ID_后缀"图标确实在表里
    #[test]
    fn special_form_lookup() {
        assert_eq!(special_form("minecraft:furnace"), Some(SpecialForm::Lit));
        assert_eq!(
            special_form("minecraft:redstone_lamp"),
            Some(SpecialForm::Lit)
        );
        assert_eq!(
            special_form("minecraft:oak_button"),
            Some(SpecialForm::Pressed)
        );
        assert_eq!(
            special_form("minecraft:oak_pressure_plate"),
            Some(SpecialForm::Down)
        );
        assert_eq!(special_form("minecraft:oak_door"), Some(SpecialForm::Open));
        assert_eq!(
            special_form("minecraft:piston"),
            Some(SpecialForm::Extended)
        );
        assert_eq!(special_form("minecraft:vault"), Some(SpecialForm::Active));
        assert_eq!(special_form("minecraft:campfire"), Some(SpecialForm::Off));
        // 无特殊形态的方块
        assert_eq!(special_form("minecraft:stone"), None);
        assert_eq!(special_form("minecraft:oak_stairs"), None);

        // 每个查到的形态都应有对应后缀图标，且图标ID后缀与形态匹配
        for (id, cat, _) in BLOCK_ICONS {
            if let Some(form) = special_form(id) {
                let alt = format!("{id}_{}", form.suffix());
                let hit = BLOCK_ICONS
                    .iter()
                    .any(|(name, _, spec)| *name == alt && matches!(spec, IconSpec::Form(..)));
                assert!(hit, "形态图标缺失：{alt}（{cat}）");
            }
        }
    }
}

#[cfg(test)]
mod head_render_tests {
    use super::*;

    /// 头颅渲染冒烟：各头颅出图到 tests/out/head_*.png，需 GPU 与本地客户端 jar
    ///
    /// 运行：cargo test -p mml-tex-draw --lib head_render -- --ignored --nocapture
    /// 环境变量 MML_TEST_JAR 指定客户端 jar，缺省用 tests/out/tools/client-26.3.jar
    #[test]
    #[ignore]
    fn head_render_smoke() {
        let jar = std::env::var("MML_TEST_JAR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/out/tools/client-26.3.jar")
            });
        assert!(jar.exists(), "客户端jar不存在：{}", jar.display());

        let archive = BaseArchive::open(&jar).unwrap();
        let gpu = GpuCtx::try_new().expect("无可用GPU后端");

        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/out");
        std::fs::create_dir_all(&dir).unwrap();

        // 各头颅：单层 64×32 贴图与带帽子层 64×64 贴图各取一个
        for (id, tex, hat) in [
            ("skeleton_skull", "entity/skeleton/skeleton", false),
            ("creeper_head", "entity/creeper/creeper", false),
            ("player_head", "entity/player/wide/steve", true),
            ("zombie_head", "entity/zombie/zombie", true),
        ] {
            let mut cache = HashMap::new();
            let BakedModelTextures { model, textures } = bake_head(&archive, tex, hat, &mut cache)
                .unwrap_or_else(|| panic!("{id} 烘焙失败"));
            assert_eq!(model.quads.len(), if hat { 12 } else { 6 }, "{id} 面数不符");

            // 带帽子层的贴图必须是 humanoidHeadLayer 的 64×64，mobHeadLayer 是 64×32
            let tex_size = textures
                .values()
                .map(|p| (p.width(), p.height()))
                .next()
                .unwrap();
            assert_eq!(
                tex_size,
                if hat { (64, 64) } else { (64, 32) },
                "{id} 贴图尺寸不符"
            );
            let rgba = gpu
                .render(&model, &textures, None, 256)
                .expect("GPU渲染失败");

            let path = dir.join(format!("head_{id}.png"));
            let file = std::fs::File::create(&path).unwrap();
            let mut encoder = png::Encoder::new(file, 256, 256);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&rgba)
                .unwrap();
            println!("输出：{}", path.display());
        }
    }

    /// 用本地皮肤文件渲染头颅图标；slim 与 wide 只差手臂，头部区域一致，均按带帽子层烘焙
    ///
    /// 运行：cargo test -p mml-tex-draw --lib head_render_skin_file -- --ignored --nocapture
    #[test]
    #[ignore]
    fn head_render_skin_file() {
        let skin = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../mml-skin-draw/tests/skin_slim.png");
        assert!(skin.exists(), "皮肤文件不存在：{}", skin.display());
        let bytes = std::fs::read(&skin).unwrap();
        let tex = decode_png(&bytes).expect("皮肤解码失败");

        let gpu = GpuCtx::try_new().expect("无可用GPU后端");

        let model = bake_head_model(tex.clone(), true, "skin");
        assert_eq!(model.quads.len(), 12, "面数不符");
        let textures = HashMap::from([("skin".to_string(), tex)]);
        let rgba = gpu
            .render(&model, &textures, None, 256)
            .expect("GPU渲染失败");

        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/out");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("head_skin_slim.png");
        let file = std::fs::File::create(&path).unwrap();
        let mut encoder = png::Encoder::new(file, 256, 256);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&rgba)
            .unwrap();
        println!("输出：{}", path.display());
    }
}

#[cfg(test)]
mod sprite_tests {
    use super::*;

    /// 构造竖排动画条带，n 帧 16x16，每帧灰度渐变便于区分
    fn anim_strip(frames: usize) -> Pixmap {
        let mut data = vec![0u8; 16 * 16 * frames * 4];
        for (i, px_chunk) in data.chunks_exact_mut(4).enumerate() {
            // 每帧整体一种不透明颜色，帧间可区分，去重不会合并
            let frame = (i / (16 * 16)) as u8;
            px_chunk.copy_from_slice(&[frame * 60, 128, 64, 255]);
        }

        Pixmap::from_vec(
            data,
            tiny_skia::IntSize::from_wh(16, 16 * frames as u32).unwrap(),
        )
        .unwrap()
    }

    /// 动画条带时间线逐帧取图与 APNG 编码，render_baked 的核心环节
    #[test]
    fn sprite_apng_smoke() {
        let strip = anim_strip(4);
        let meta = AnimMeta {
            frametime: 20,
            interpolate: true,
            frames: None,
        };
        // 逐步定位失败环节
        let tl = AnimTimeline::build(&strip, &meta);
        assert!(tl.is_some(), "timeline构建失败");
        let tl = tl.unwrap();
        assert_eq!(tl.total_ticks(), 80, "total_ticks应=80");
        let mut frames = Vec::new();
        for k in (0..tl.total_ticks()).step_by(20) {
            let bm = tl.frame_at(k).expect("frame_at应成功");
            let crate::model::BitmapRgba {
                data: px,
                width: w,
                height: h,
                ..
            } = crate::model::bitmap_rgba(&bm).expect("位图读出应成功");
            assert_eq!((w, h), (16, 16));
            frames.push(ApngFrame {
                rgba: px,
                ticks: 20,
            });
        }
        let data = encode_apng(16, frames);
        assert!(data.is_some(), "APNG应成功");
        // 16×16纯色帧压缩后很小，仅验证编码产物有效
        assert!(data.unwrap().len() > 100, "APNG应有实际内容");
    }
}
