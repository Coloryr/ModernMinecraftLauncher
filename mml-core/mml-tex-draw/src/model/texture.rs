//! 贴图解析与着色
//!
//! 贴图引用展开、按路径取染色色、自发光判定。
use std::collections::HashMap;

use glam::Vec3;
use mml_base::archives::BaseArchive;
use tiny_skia::Pixmap;

use crate::block::{
    AnimMeta, BIRCH_TINT, FOLIAGE_TINT, GRASS_TINT, SPRUCE_TINT, TextureRefObj, decode_png,
    read_anim_meta,
};

use super::json::ElementJson;
use super::*;

fn resolve_ref(textures: &HashMap<String, TextureRefObj>, r: &str) -> Option<String> {
    let mut r = r;
    for _ in 0..8 {
        let rest = r.strip_prefix('#').unwrap_or(r);
        match textures.get(rest) {
            Some(v) => r = v.value()?,
            None if r.starts_with('#') => return None,
            None => return Some(r.to_string()),
        }
    }
    None
}

/// 贴图引用值 -> jar内路径
///
/// - `value`: 贴图引用值，取 `block/stone` 或 `minecraft:block/stone`
///
/// # 返回值
///
/// 返回 jar 内路径 `assets/minecraft/textures/…​.png`；非 minecraft 命名空间时返回 `None`
pub(crate) fn texture_path(value: &str) -> Option<String> {
    let (ns, path) = value.split_once(':').unwrap_or(("minecraft", value));
    if ns != "minecraft" {
        return None;
    }
    Some(format!("assets/minecraft/textures/{path}.png"))
}

/// 加载贴图，带缓存，返回 RGBA 位图
///
/// - `archive`: 客户端 jar 归档
/// - `cache`: 跨模型复用的贴图缓存，路径 → 解码位图
/// - `path`: jar 内贴图路径
///
/// # 返回值
///
/// 返回解码后的位图，读取或 PNG 解码失败时返回 `None`
pub(crate) fn load_texture(
    archive: &BaseArchive,
    cache: &mut HashMap<String, Pixmap>,
    path: &str,
) -> Option<Pixmap> {
    if let Some(tex) = cache.get(path) {
        return Some(tex.clone());
    }
    let bytes = archive.read(path).ok()?;
    let tex = decode_png(&bytes)?;
    cache.insert(path.to_string(), tex.clone());
    Some(tex)
}

/// 解码后的位图，straight-alpha RGBA
pub(crate) struct BitmapRgba {
    /// 像素数据，straight-alpha RGBA
    pub(crate) data: Vec<u8>,
    /// 每行字节数
    pub(crate) stride: usize,
    /// 宽，单位像素
    pub(crate) width: usize,
    /// 高，单位像素；动画贴图是各帧堆叠后的总高
    pub(crate) height: usize,
}

/// 读取位图为 straight-alpha RGBA
///
/// - `tex`: 解码后的位图
///
/// # 返回值
///
/// 见 [`BitmapRgba`]
pub(crate) fn bitmap_rgba(tex: &Pixmap) -> Option<BitmapRgba> {
    let (w, h) = (tex.width() as usize, tex.height() as usize);

    Some(BitmapRgba {
        data: crate::block::to_straight_rgba(tex.data()),
        stride: w * 4,
        width: w,
        height: h,
    })
}

/// 按 NativeImage.computeTransparency 扫 uv 矩形：仅当含 1..254 之间的 alpha
/// 才算 translucent，alpha==0 只是镂空，仍走cutout管线；
/// x0=floor(u0·w) y0=floor(v0·h) x1=ceil(u1·w) y1=ceil(v1·h)，动画贴图所有唯一帧合并判定
///
/// - `tex`: 解码后的位图
/// - `anim`: 动画元数据，动画贴图按方形帧竖向堆叠判定
/// - `uv`: uv 矩形，0..1，按单帧归一化
///
/// # 返回值
///
/// uv 矩形内含半透明像素返回 true，否则 false
pub(crate) fn rect_translucent(tex: &Pixmap, anim: Option<&AnimMeta>, uv: &[f32; 4]) -> bool {
    let Some(BitmapRgba {
        data: base,
        stride,
        width: w,
        height: h_total,
    }) = bitmap_rgba(tex)
    else {
        return false;
    };
    if w == 0 || h_total == 0 {
        return false;
    }
    // 动画贴图按方形帧竖向堆叠，帧高=宽；uv矩形以单帧归一化
    let frame_h = if anim.is_some() { w } else { h_total };
    let frame_count = (h_total / frame_h).max(1);
    let frames: Vec<u32> = match anim {
        Some(meta) => {
            let mut seen = Vec::new();
            let list = meta
                .frames
                .as_ref()
                .map(|list| list.iter().map(|f| f.index).collect::<Vec<_>>());
            for i in list.unwrap_or_else(|| (0..frame_count as u32).collect()) {
                let idx = i % frame_count as u32;
                if !seen.contains(&idx) {
                    seen.push(idx);
                }
            }
            if seen.is_empty() { vec![0] } else { seen }
        }
        None => vec![0],
    };

    let x0 = (uv[0] * w as f32).floor() as usize;
    let y0 = (uv[1] * frame_h as f32).floor() as usize;
    let x1 = ((uv[2] * w as f32).ceil() as usize).min(w);
    let y1 = ((uv[3] * frame_h as f32).ceil() as usize).min(frame_h);
    if x0 >= x1 || y0 >= y1 {
        return false;
    }
    for &frame in &frames {
        let fy = frame as usize * frame_h;
        for y in y0..y1 {
            let row = fy + y;
            if row >= h_total {
                break;
            }
            for x in x0..x1 {
                let a = base[row * stride + x * 4 + 3];
                // 与游戏一致：a==0为镂空，走cutout；仅1..254算半透明
                if a > 0 && a < 255 {
                    return true;
                }
            }
        }
    }
    false
}

/// tintindex -> 染色：灰度贴图按群系色，与游戏平原群系常量色一致
///
/// - `path`: 贴图相对路径，按路径关键词选群系色
///
/// # 返回值
///
/// 返回 RGB 染色，0..255
fn tint_for_path(path: &str) -> [u8; 3] {
    if path.contains("birch_leaves") {
        BIRCH_TINT
    } else if path.contains("spruce_leaves") {
        SPRUCE_TINT
    } else if path.contains("leaves") {
        FOLIAGE_TINT
    } else {
        GRASS_TINT
    }
}

/// ===== bake：照 FaceBakery.bakeQuad 逐条对应 =====

/// 取盒子边界分量，把 extent 枚举映射到 from/to 的分量
///
/// - `extent`: 坐标选择器，0=MIN_X 1=MIN_Y 2=MIN_Z 3=MAX_X 4=MAX_Y 5=MAX_Z
/// - `from`: 盒子最小角
/// - `to`: 盒子最大角
///
/// # 返回值
///
/// 返回对应的分量值
pub(crate) fn select_extent(extent: u8, from: &[f32], to: &[f32]) -> f32 {
    match extent {
        0 => from[0],
        1 => from[1],
        2 => from[2],
        3 => to[0],
        4 => to[1],
        _ => to[2],
    }
}

/// 面索引 -> 法线轴，与 FACE_INDEX 一致：down=0..east=5
///
/// - `i`: 面索引，0=down..5=east
///
/// # 返回值
///
/// 返回该面的外向法线单位向量
pub(crate) fn face_normal(i: usize) -> Vec3 {
    match i {
        0 => Vec3::NEG_Y,
        1 => Vec3::Y,
        2 => Vec3::NEG_Z,
        3 => Vec3::Z,
        4 => Vec3::NEG_X,
        _ => Vec3::X,
    }
}

/// 法线 -> 面索引，取点积最接近1的轴
///
/// - `dir`: 轴向单位向量
///
/// # 返回值
///
/// 返回匹配的面索引，0=down..5=east；非轴向向量返回 `None`
pub(crate) fn face_index_of(dir: Vec3) -> Option<usize> {
    (0..6).find(|&i| dir.dot(face_normal(i)) > 0.999)
}

/// calculateFacing：由前三顶点叉积求几何法线，量化到点积最大的正向坐标轴；
/// 退化面返回 None，游戏回退 Direction.UP，等价零法线
///
/// - `pos`: 4 个顶点位置，0..1 模型空间
///
/// # 返回值
///
/// 返回量化的轴向单位向量，退化面返回 `None`
pub(crate) fn calculate_facing(pos: &[[f32; 3]; 4]) -> Option<[f32; 3]> {
    let p0 = Vec3::from(pos[0]);
    let p1 = Vec3::from(pos[1]);
    let p2 = Vec3::from(pos[2]);
    let normal = (p1 - p0).cross(p2 - p0);
    if !normal.is_finite() {
        return None;
    }
    let mut best: Option<(f32, [f32; 3])> = None;
    for candidate in [
        Vec3::NEG_X,
        Vec3::NEG_Y,
        Vec3::NEG_Z,
        Vec3::X,
        Vec3::Y,
        Vec3::Z,
    ] {
        let product = normal.dot(candidate);
        if product >= 0.0 && product > best.map_or(0.0, |(c, _): (f32, [f32; 3])| c) {
            best = Some((product, candidate.to_array()));
        }
    }
    best.map(|(_, dir)| dir)
}

/// recalculateWinding：把顶点顺序换成该朝向 FaceInfo 的规范角序，uv随之同步交换
///
/// - `pos`: 顶点位置，就地重排
/// - `uv`: 顶点 uv，随 pos 同步交换
/// - `facing`: 目标面索引，0=down..5=east
pub(crate) fn recalculate_winding(pos: &mut [[f32; 3]; 4], uv: &mut [[f32; 2]; 4], facing: usize) {
    let mut min = [999.0f32; 3];
    let mut max = [-999.0f32; 3];
    for p in pos.iter() {
        for c in 0..3 {
            min[c] = min[c].min(p[c]);
            max[c] = max[c].max(p[c]);
        }
    }
    for vertex in 0..4 {
        let info = FACE_INFO[facing][vertex];
        let want: [f32; 3] = [
            select_extent(info.x, &min, &max),
            select_extent(info.y, &min, &max),
            select_extent(info.z, &min, &max),
        ];
        let found = (vertex..4).find(|&i| (0..3).all(|c| (pos[i][c] - want[c]).abs() < 1e-5));
        if let Some(i) = found {
            if i != vertex {
                pos.swap(i, vertex);
                uv.swap(i, vertex);
            }
        }
    }
}

/// 烘焙一个元素的全部面 -> quad 列表
///
/// - `element`: 模型元素，坐标 0..16
/// - `textures`: 父链合并后的贴图引用表
/// - `archive`: 客户端 jar 归档，读取贴图与动画元数据
/// - `tex_cache`: 跨模型复用的贴图缓存，路径 → 解码位图
/// - `item_tints`: None=方块路径，tintindex按贴图路径取群系常量色；
///   Some=物品路径，tintindex索引items/*.json的tint表，越界或无tintindex为白色，与游戏一致
///
/// # 返回值
///
/// 返回该元素全部可烘焙面，贴图解析失败的面跳过
pub(crate) fn bake_element(
    element: &ElementJson,
    textures: &HashMap<String, TextureRefObj>,
    archive: &BaseArchive,
    tex_cache: &mut HashMap<String, Pixmap>,
    item_tints: Option<&[[u8; 3]]>,
) -> Vec<Quad> {
    let mut quads = Vec::new();
    let Some(faces) = &element.faces else {
        return quads;
    };
    for (name, face) in faces {
        let Some(facing) = FACE_INDEX
            .iter()
            .find(|f| f.name == name.as_str())
            .map(|f| f.index)
        else {
            continue;
        };
        // uv：显式或 defaultFaceUV
        let uvs: [f32; 4] = match &face.uv {
            Some(v) if v.len() == 4 => [v[0], v[1], v[2], v[3]],
            _ => default_face_uv(facing, &element.from, &element.to),
        };

        // 元素旋转：origin为0..16坐标，矩阵内已/16
        let elem_rot = element.rotation.as_ref().map(|r| {
            let axis = match r.axis.as_deref() {
                Some("x") => 0usize,
                Some("z") => 2usize,
                _ => 1usize,
            };
            element_rotation_matrix(
                [r.origin[0] / 16.0, r.origin[1] / 16.0, r.origin[2] / 16.0],
                axis,
                r.angle.unwrap_or(0.0),
                r.rescale.unwrap_or(false),
            )
        });

        // 四顶点位置与uv，bakeVertex：角选点/16 -> 元素旋转
        let corner = |i: usize| -> [f32; 3] {
            let info = FACE_INFO[facing][i];
            [
                select_extent(info.x, &element.from, &element.to) / 16.0,
                select_extent(info.y, &element.from, &element.to) / 16.0,
                select_extent(info.z, &element.from, &element.to) / 16.0,
            ]
        };
        let mut pos: [[f32; 3]; 4] = std::array::from_fn(corner);
        let mut uv: [[f32; 2]; 4] = std::array::from_fn(|i| {
            // CuboidFace.getU/getV：角i对应UVs下标 `(i + rotation/90) % 4`
            let idx = ((i as u32 + face.rotation.unwrap_or(0) / 90) % 4) as usize;
            let u = if idx == 0 || idx == 1 { uvs[0] } else { uvs[2] };
            let v = if idx == 0 || idx == 3 { uvs[1] } else { uvs[3] };
            [u / 16.0, v / 16.0]
        });
        if let Some(m) = &elem_rot {
            for p in &mut pos {
                let v = m.transform_point3(Vec3::from(*p));
                *p = v.to_array();
            }
        }

        // 法线量化 + 规范角序：无元素旋转时才做winding重排
        let facing_final = calculate_facing(&pos).and_then(|dir| face_index_of(Vec3::from(dir)));
        if elem_rot.is_none() {
            if let Some(f) = facing_final {
                recalculate_winding(&mut pos, &mut uv, f);
            }
        }

        // 贴图与透明度
        let Some(value) = face
            .texture
            .as_deref()
            .and_then(|r| resolve_ref(textures, r))
        else {
            continue;
        };
        let Some(path) = texture_path(&value) else {
            continue;
        };
        let Some(tex) = load_texture(archive, tex_cache, &path) else {
            continue;
        };
        let anim = read_anim_meta(archive, &path);

        // tint：方块路径按贴图路径取群系常量色，物品路径索引items/*.json的tint表
        let tint = match item_tints {
            None => face.tintindex.map(|_| tint_for_path(&path)),
            Some(list) => face.tintindex.and_then(|i| list.get(i as usize).copied()),
        }
        .unwrap_or([255, 255, 255]);
        let color = [
            tint[0] as f32 / 255.0,
            tint[1] as f32 / 255.0,
            tint[2] as f32 / 255.0,
            1.0,
        ];

        quads.push(Quad {
            pos,
            uv,
            normal: facing_final
                .map(|f| face_normal(f).to_array())
                .unwrap_or([0.0, 1.0, 0.0]),
            color: [color; 4],
            tex: path.clone(),
            translucent: rect_translucent(
                &tex,
                Some(&anim),
                &[uvs[0] / 16.0, uvs[1] / 16.0, uvs[2] / 16.0, uvs[3] / 16.0],
            ),
            // 火焰贴图自发光，篝火火焰在游戏里不受方向光变暗
            fullbright: is_fullbright_tex(&path),
        });
    }
    quads
}

/// 自发光贴图：篝火/灵魂篝火的火焰，渲染时不做方向光衰减
///
/// - `path`: 贴图相对路径，带 .png 后缀
fn is_fullbright_tex(path: &str) -> bool {
    // soul_campfire_fire 也包含 campfire_fire
    path.contains("campfire_fire")
}
