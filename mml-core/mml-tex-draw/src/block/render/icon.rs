//! 图标渲染
//!
//! 等轴测 / 平面 / 拼接三种取图方式，输出图标位图。
use std::collections::HashMap;

use crate::block::icons::IconSpec;
use crate::block::obj::BlockFace2d;
use crate::gpu::GpuCtx;
use crate::model::{BakedModel, BakedModelTextures, Quad, bake_model};

use mml_base::archives::BaseArchive;
use mml_sys::path_helper;
use tiny_skia::Pixmap;

use super::*;

/// 渲染并写盘完成的一个方块图标
pub(super) struct IconOutput {
    /// 方块 / 图标 id，带命名空间，如 `minecraft:stone`
    pub(super) id: String,
    /// 输出文件名，`:` 已换成 `_`，如 `minecraft_stone.png`
    pub(super) out_name: String,
    /// 2D 平面贴图的分组（按贴图去重）；一组都没有时为空，
    /// 如头颅这类皮肤贴图不适合裁方块
    pub(super) out_name2d: Option<Vec<BlockFace2d>>,
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
pub(super) fn render_icon(
    gpu: Option<&GpuCtx>,
    archive: &BaseArchive,
    tex_cache: &mut HashMap<String, Pixmap>,
    id_rel: &str,
    spec: &IconSpec,
) -> Option<IconOutput> {
    // 表 ID 带命名空间 minecraft:xx，为 mod 支持预留；输出文件名把 ':' 换成 '_'
    let id = id_rel;
    let out_name = format!("{}.png", id_rel.replace(':', "_"));
    // 2D 平面贴图：只有整块立方体才做 —— 栅栏、栅栏门、台阶、楼梯这类是拼出来的几何，
    // 单拿一张贴图代表不了它，宁可不给 2D。
    // 顺序固定为 顶 / 侧 / 底；第二个值是贴图指纹，用来把上下同图的面并成一组
    let mut flat: [Option<(Vec<u8>, String)>; 3] = [None, None, None];
    let data = match spec {
        IconSpec::Skip => return None,
        IconSpec::Form(_, inner) => return render_icon(gpu, archive, tex_cache, id_rel, inner),
        IconSpec::Model(rel) => {
            // 烘焙失败即跳过：无 elements 的父模板、贴图引用无值
            let BakedModelTextures { model, textures } = bake_model(archive, rel, tex_cache)?;
            if is_full_cube(&model) {
                flat = flat_faces(&model, &textures);
            }
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
            if is_full_cube(&model) {
                flat = flat_faces(&model, &textures);
            }
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
            // 拼合模型（床、门）不做 2D：几何是多个盒子拼出来的
            render_baked(gpu, archive, model, textures, None)?
        }
    };

    path_helper::write_bytes(&crate::get_block_dir()?.join(&out_name), &data).ok()?;

    // 2D 与 3D 分开存盘：两套同时存在，展示时不用重渲染，见 get_block_path_2d_face
    let base = id_rel.replace(':', "_");
    let mut groups: Vec<BlockFace2d> = Vec::new();
    // 贴图指纹 → 已合并的组，上下同图时只写一份文件、界面也只给一个选项
    let mut merged: Vec<(String, usize)> = Vec::new();
    if let Some(dir) = crate::get_block2d_dir() {
        // 顶 / 侧 / 底，文件名后缀与面名一致；侧面沿用无后缀的老名字
        for (index, item) in flat.iter().enumerate() {
            let Some((bytes, key)) = item else {
                continue;
            };
            let face = ["top", "side", "bottom"][index];

            if let Some((_, at)) = merged.iter().find(|(each, _)| each == key)
                && let Some(group) = groups.get_mut(*at)
            {
                group.faces.push(face.to_string());
                continue;
            }

            let name = match index {
                0 => format!("{base}_2d_top.png"),
                2 => format!("{base}_2d_bottom.png"),
                _ => format!("{base}_2d.png"),
            };
            if path_helper::write_bytes(&dir.join(&name), bytes).is_err() {
                continue;
            }
            merged.push((key.clone(), groups.len()));
            groups.push(BlockFace2d {
                faces: vec![face.to_string()],
                file: name,
            });
        }
    }
    let out_name2d = (!groups.is_empty()).then_some(groups);

    Some(IconOutput {
        id: id.to_string(),
        out_name,
        out_name2d,
    })
}

/// 模型是不是一个整块立方体
///
/// 判定看烘焙后的 quad：至少 6 个面，所有顶点都落在 0 / 1 上，且包围盒正好是 1×1×1。
/// 栅栏、栅栏门、台阶、楼梯、玻璃板、火把这些是拼出来的几何（顶点会落在 0.5、0.375 这类位置，
/// 或者包围盒不是整块），单拿一张贴图代表不了，不给 2D。草方块的两层整面模型会通过。
///
/// - `model`: 烘焙后的模型
///
/// # 返回值
///
/// 整块立方体返回 `true`
pub(super) fn is_full_cube(model: &BakedModel) -> bool {
    if model.quads.len() < 6 {
        return false;
    }

    const EPS: f32 = 1e-3;
    let mut min = [1.0f32; 3];
    let mut max = [0.0f32; 3];
    for quad in &model.quads {
        for pos in &quad.pos {
            for axis in 0..3 {
                let value = pos[axis];
                if (value - 0.0).abs() > EPS && (value - 1.0).abs() > EPS {
                    return false;
                }
                min[axis] = min[axis].min(value);
                max[axis] = max[axis].max(value);
            }
        }
    }

    min.iter().all(|each| each.abs() <= EPS) && max.iter().all(|each| (each - 1.0).abs() <= EPS)
}

/// 按面出 2D 平面贴图：顶 / 侧 / 底，取不到的为 `None`
///
/// - `model`: 烘焙后的模型
/// - `textures`: 模型用到的贴图，路径 → 解码位图
///
/// # 返回值
///
/// 依次是顶面 / 侧面 / 底面的 PNG 字节
pub(super) fn flat_faces(
    model: &BakedModel,
    textures: &HashMap<String, Pixmap>,
) -> [Option<(Vec<u8>, String)>; 3] {
    [
        flat_face(model, textures, |normal| normal[1] > 0.5),
        flat_face(model, textures, |normal| normal[1].abs() <= 0.5),
        flat_face(model, textures, |normal| normal[1] < -0.5),
    ]
}

/// 取指定朝向里最有代表性的一张贴图，出 2D 平面贴图
///
/// 按「quad 面积 × 贴图不透明占比」挑代表贴图：只看面积时草方块侧面的 overlay
/// 会和底层侧面打平，带上不透明度才选得到底层那张；群系 / 物品 tint 从 quad 顶点色补上。
/// 贴图按最近邻放大到与 3D 图标同尺寸，像素画不会被插值糊掉。
///
/// - `model`: 烘焙后的模型
/// - `textures`: 模型用到的贴图，路径 → 解码位图
/// - `pick`: 该面的法线筛选条件
///
/// # 返回值
///
/// 返回 PNG 字节；该朝向没有可用 quad 或贴图缺失时返回 `None`
pub(super) fn flat_face(
    model: &BakedModel,
    textures: &HashMap<String, Pixmap>,
    pick: impl Fn(&[f32; 3]) -> bool,
) -> Option<(Vec<u8>, String)> {
    let mut best: Option<(f32, &Quad)> = None;
    for quad in model.quads.iter().filter(|quad| pick(&quad.normal)) {
        let Some(tex) = textures.get(&quad.tex) else {
            continue;
        };
        let frame = first_frame(tex);
        let score = quad_area(&quad.pos) * opaque_ratio(&frame);
        if best.is_none_or(|(top, _)| score > top) {
            best = Some((score, quad));
        }
    }

    let quad = best?.1;
    let frame = first_frame(textures.get(&quad.tex)?);
    // 与 3D 路径同一个换算：tiny-skia 的 Pixmap 数据是预乘 RGBA，只反除 alpha，不换通道
    let mut rgba = to_straight_rgba(frame.data());

    // 群系 / 物品染色记在顶点色里，贴图本身不带色，这里补上一层
    let tint = quad.color[0];
    if tint != [1.0, 1.0, 1.0, 1.0] {
        for px in rgba.chunks_exact_mut(4) {
            for (i, channel) in px.iter_mut().enumerate().take(3) {
                *channel = (f32::from(*channel) * tint[i]).round().clamp(0.0, 255.0) as u8;
            }
        }
    }

    // 贴图指纹：同路径 + 同染色出来的图完全一样，用来把上下同图的面并组
    let key = format!("{}|{tint:?}", quad.tex);
    let size = BLOCK_SIZE as u32;
    Some((
        encode_png(
            size,
            &scale_nearest(&rgba, frame.width(), frame.height(), size),
        )?,
        key,
    ))
}

/// 动画贴图取首帧，静态贴图原样克隆
pub(super) fn first_frame(tex: &Pixmap) -> Pixmap {
    if tex.height() > tex.width() {
        extract_frame(tex, 0).unwrap_or_else(|| tex.clone())
    } else {
        tex.clone()
    }
}

/// 贴图里不透明像素的占比，0..1
pub(super) fn opaque_ratio(tex: &Pixmap) -> f32 {
    let total = tex.width() as usize * tex.height() as usize;
    if total == 0 {
        return 0.0;
    }
    let visible = tex.data().chunks_exact(4).filter(|px| px[3] > 0).count();
    visible as f32 / total as f32
}

/// 四边形面积，用来挑"最有代表性"的那张贴图
///
/// - `pos`: 4 个顶点位置，角序从面外侧看逆时针
///
/// # 返回值
///
/// 返回面积，单位模型空间平方
pub(super) fn quad_area(pos: &[[f32; 3]; 4]) -> f32 {
    let sub = |a: [f32; 3], b: [f32; 3]| [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let cross = |u: [f32; 3], v: [f32; 3]| {
        let c = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt()
    };
    // 两个三角形拼成四边形
    cross(sub(pos[0], pos[1]), sub(pos[0], pos[2])) / 2.0
        + cross(sub(pos[0], pos[2]), sub(pos[0], pos[3])) / 2.0
}

/// 最近邻缩放到 size×size
///
/// - `src`: 源像素，RGBA8 紧邻
/// - `sw` / `sh`: 源宽高，单位像素
/// - `size`: 目标边长，单位像素
///
/// # 返回值
///
/// 返回 size²×4 字节的 RGBA 像素
pub(super) fn scale_nearest(src: &[u8], sw: u32, sh: u32, size: u32) -> Vec<u8> {
    let mut out = vec![0u8; size as usize * size as usize * 4];
    if sw == 0 || sh == 0 {
        return out;
    }
    for y in 0..size {
        let sy = (y * sh / size).min(sh - 1);
        for x in 0..size {
            let sx = (x * sw / size).min(sw - 1);
            let si = ((sy as usize * sw as usize + sx as usize) * 4) as usize;
            let di = ((y as usize * size as usize + x as usize) * 4) as usize;
            out[di..di + 4].copy_from_slice(&src[si..si + 4]);
        }
    }
    out
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
pub(super) fn transform_composite_part(model: &mut BakedModel, tr: [f32; 3], yaw_deg: f32) {
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
pub(super) fn render_once(
    gpu: Option<&GpuCtx>,
    model: &BakedModel,
    textures: &HashMap<String, Pixmap>,
    glint: Option<&Pixmap>,
) -> Option<Vec<u8>> {
    gpu.and_then(|ctx| ctx.render(model, textures, glint, BLOCK_SIZE as u32))
}

/// 一个渲染完成的方块图标
pub(super) struct RenderedIcon {
    /// 方块 / 图标 id
    pub(super) id: String,
    /// 渲染输出的文件名
    pub(super) out_name: String,
    /// 2D 平面贴图的分组，一个都没有时为空
    pub(super) out_name2d: Option<Vec<BlockFace2d>>,
    /// 所属分类，来自 `BLOCK_ICONS`
    pub(super) cat: String,
}
