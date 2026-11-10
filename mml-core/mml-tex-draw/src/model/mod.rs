//! 26.3 模型 JSON → 渲染 quad 的精确烘焙，照反编译 FaceBakery/CuboidRotation/ItemTransform 规则
//!
//! 顶点 0..1 模型空间，照 FaceBakery.bakeVertex 的 div(16)，gui display 变换由渲染后端在
//! shader 矩阵里叠加；quad 只携带几何/uv/法线/贴图名/tint 色/透明度分组。

use std::collections::HashMap;

use glam::{Mat4, Vec3};
use mml_base::archives::BaseArchive;
use tiny_skia::Pixmap;

mod json;
mod texture;

pub(crate) use self::json::{Resolved, resolve};
pub(crate) use self::texture::*;

/// 一个渲染 quad，坐标在 0..1 模型空间
#[derive(Clone)]
pub struct Quad {
    /// 4 个顶点位置，角序照 FaceInfo，从面外侧看逆时针
    pub pos: [[f32; 3]; 4],
    /// 4 个顶点 uv，0..1，v=0 为贴图顶行
    pub uv: [[f32; 2]; 4],
    /// 光照法线，calculateFacing 量化后的轴向单位向量
    pub normal: [f32; 3],
    /// 4 个顶点色，tint 后 RGBA，0..1
    pub color: [[f32; 4]; 4],
    /// 需要使用的贴图相对路径
    pub tex: String,
    /// uv 矩形内含半透明像素，走 translucent pass
    pub translucent: bool,
    /// 自发光，如火焰：不参与方向光照，全亮渲染
    pub fullbright: bool,
}

/// 一个完整模型的烘焙产物
pub struct BakedModel {
    /// 渲染 quad 列表
    pub quads: Vec<Quad>,
    /// gui_light：true=side 走 ITEMS_3D 光照，false=front 走 ITEMS_FLAT
    pub gui_light_3d: bool,
    /// gui display 变换，照 ItemTransform，缺省即 NO_TRANSFORM
    pub transform: GuiTransform,
}

/// 一次烘焙的产物：模型与它引用到的贴图
///
/// 方块与物品两条烘焙链路共用，`textures` 直接喂给渲染器
pub struct BakedModelTextures {
    /// 烘焙出的模型；其 quad 的 `tex` 是 `textures` 的键
    pub model: BakedModel,
    /// 模型 quad 引用到的贴图，路径 → 解码位图；同一路径跨模型复用同一份解码结果
    pub textures: HashMap<String, Pixmap>,
}

/// gui display 变换参数，照 ItemTransform 语义，translation 已 ×0.0625
#[derive(Clone, Copy)]
pub struct GuiTransform {
    /// 旋转角度，单位度，绕 x/y/z 轴
    pub rotation: [f32; 3],
    /// 平移，模型空间单位，已 ×0.0625 并 clamp ±5
    pub translation: [f32; 3],
    /// 缩放系数，clamp ±4，未写时为 1
    pub scale: [f32; 3],
}

impl Default for GuiTransform {
    /// NO_TRANSFORM：rotation/translation 零、scale 一
    fn default() -> Self {
        Self {
            rotation: [0.0; 3],
            translation: [0.0; 3],
            scale: [1.0; 3],
        }
    }
}

/// FACE_INFO 表里的一格：一个顶点角，三个轴各选盒子的一条边
#[derive(Clone, Copy)]
pub(crate) struct FaceVertex {
    /// x 轴选哪条边：0=MIN_X、3=MAX_X；本表内只出现这两个值
    pub(crate) x: u8,
    /// y 轴选哪条边：1=MIN_Y、4=MAX_Y；本表内只出现这两个值
    pub(crate) y: u8,
    /// z 轴选哪条边：2=MIN_Z、5=MAX_Z；本表内只出现这两个值
    pub(crate) z: u8,
}

impl FaceVertex {
    /// 按 x/y/z 选择器造一格，书写顺序照 FaceInfo 的 VertexInfo
    const fn new(x: u8, y: u8, z: u8) -> Self {
        Self { x, y, z }
    }
}

/// FaceInfo：六个面的 4 个顶点角，坐标选择器 0=MIN_X 1=MIN_Y 2=MIN_Z 3=MAX_X 4=MAX_Y 5=MAX_Z；
/// 照抄 net.minecraft.client.renderer.FaceInfo 的六个枚举常量
pub(crate) const FACE_INFO: [[FaceVertex; 4]; 6] = [
    // down: (MIN_X,MIN_Y,MAX_Z) (MIN_X,MIN_Y,MIN_Z) (MAX_X,MIN_Y,MIN_Z) (MAX_X,MIN_Y,MAX_Z)
    [
        FaceVertex::new(0, 1, 5),
        FaceVertex::new(0, 1, 2),
        FaceVertex::new(3, 1, 2),
        FaceVertex::new(3, 1, 5),
    ],
    // up: (MIN_X,MAX_Y,MIN_Z) (MIN_X,MAX_Y,MAX_Z) (MAX_X,MAX_Y,MAX_Z) (MAX_X,MAX_Y,MIN_Z)
    [
        FaceVertex::new(0, 4, 2),
        FaceVertex::new(0, 4, 5),
        FaceVertex::new(3, 4, 5),
        FaceVertex::new(3, 4, 2),
    ],
    // north: (MAX_X,MAX_Y,MIN_Z) (MAX_X,MIN_Y,MIN_Z) (MIN_X,MIN_Y,MIN_Z) (MIN_X,MAX_Y,MIN_Z)
    [
        FaceVertex::new(3, 4, 2),
        FaceVertex::new(3, 1, 2),
        FaceVertex::new(0, 1, 2),
        FaceVertex::new(0, 4, 2),
    ],
    // south: (MIN_X,MAX_Y,MAX_Z) (MIN_X,MIN_Y,MAX_Z) (MAX_X,MIN_Y,MAX_Z) (MAX_X,MAX_Y,MAX_Z)
    [
        FaceVertex::new(0, 4, 5),
        FaceVertex::new(0, 1, 5),
        FaceVertex::new(3, 1, 5),
        FaceVertex::new(3, 4, 5),
    ],
    // west: (MIN_X,MAX_Y,MIN_Z) (MIN_X,MIN_Y,MIN_Z) (MIN_X,MIN_Y,MAX_Z) (MIN_X,MAX_Y,MAX_Z)
    [
        FaceVertex::new(0, 4, 2),
        FaceVertex::new(0, 1, 2),
        FaceVertex::new(0, 1, 5),
        FaceVertex::new(0, 4, 5),
    ],
    // east: (MAX_X,MAX_Y,MAX_Z) (MAX_X,MIN_Y,MAX_Z) (MAX_X,MIN_Y,MIN_Z) (MAX_X,MAX_Y,MIN_Z)
    [
        FaceVertex::new(3, 4, 5),
        FaceVertex::new(3, 1, 5),
        FaceVertex::new(3, 1, 2),
        FaceVertex::new(3, 4, 2),
    ],
];

/// FACE_INDEX 表里的一行：模型 JSON 的面名与它对应的面下标
#[derive(Clone, Copy)]
struct FaceIndexEntry {
    /// 模型 JSON `faces` 的键，取值 `down` / `up` / `north` / `south` / `west` / `east`
    name: &'static str,
    /// 该面的下标，与 [`FACE_INFO`] 和 [`face_normal`] 同一套，0=down..5=east
    index: usize,
}

impl FaceIndexEntry {
    /// 按面名与面下标造一行
    const fn new(name: &'static str, index: usize) -> Self {
        Self { name, index }
    }
}

/// 面名 → FACE_INFO 下标，与游戏 Direction 枚举序一致
const FACE_INDEX: [FaceIndexEntry; 6] = [
    FaceIndexEntry::new("down", 0),
    FaceIndexEntry::new("up", 1),
    FaceIndexEntry::new("north", 2),
    FaceIndexEntry::new("south", 3),
    FaceIndexEntry::new("west", 4),
    FaceIndexEntry::new("east", 5),
];

/// defaultFaceUV：照 FaceBakery.defaultFaceUV，面无显式 uv 时从盒子边界投影
///
/// - `facing`: 面索引，0=down..5=east
/// - `from`: 盒子最小角，0..16 坐标
/// - `to`: 盒子最大角，0..16 坐标
///
/// # 返回值
///
/// 返回 `[u0, v0, u1, v1]`，0..16 像素坐标
fn default_face_uv(facing: usize, from: &[f32], to: &[f32]) -> [f32; 4] {
    match facing {
        0 => [from[0], 16.0 - to[2], to[0], 16.0 - from[2]], // down
        1 => [from[0], from[2], to[0], to[2]],               // up
        2 => [16.0 - to[0], 16.0 - to[1], 16.0 - from[0], 16.0 - from[1]], // north
        3 => [from[0], 16.0 - to[1], to[0], 16.0 - from[1]], // south
        4 => [from[2], 16.0 - to[1], to[2], 16.0 - from[1]], // west
        _ => [16.0 - to[2], 16.0 - to[1], 16.0 - from[2], 16.0 - from[1]], // east
    }
}

/// 元素 rotation 的变换矩阵，照 CuboidRotation：绕 origin 旋转，origin 已 /16；
/// rescale 时每轴缩放 1/max|R·axis|，先缩放后旋转：M = R·S
///
/// - `origin`: 旋转原点，模型空间，已 /16
/// - `axis`: 旋转轴，0=x 1=y 2=z
/// - `angle_deg`: 旋转角度，单位度
/// - `rescale`: 是否按游戏规则做 rescale 缩放
///
/// # 返回值
///
/// 返回绕原点旋转的变换矩阵
fn element_rotation_matrix(origin: [f32; 3], axis: usize, angle_deg: f32, rescale: bool) -> Mat4 {
    let angle = angle_deg.to_radians();
    let rot = match axis {
        0 => Mat4::from_rotation_x(angle),
        1 => Mat4::from_rotation_y(angle),
        _ => Mat4::from_rotation_z(angle),
    };
    let mut rot = rot;
    if rescale {
        let mut s = [1.0f32; 3];
        for (i, item) in s.iter_mut().enumerate() {
            let unit = match i {
                0 => Vec3::X,
                1 => Vec3::Y,
                _ => Vec3::Z,
            };
            let t = rot.transform_vector3(unit);
            *item = 1.0 / t.abs().max_element();
        }
        rot = rot * Mat4::from_scale(Vec3::from_slice(&s));
    }
    let origin = Vec3::from(origin);
    Mat4::from_translation(origin) * rot * Mat4::from_translation(-origin)
}

/// ===== 模型 JSON 结构，自带完整 display 解析，26.3 语义 =====

/// 模型 JSON 顶层结构

pub fn bake_model(
    archive: &BaseArchive,
    rel: &str,
    tex_cache: &mut HashMap<String, Pixmap>,
) -> Option<BakedModelTextures> {
    let resolved = resolve(archive, rel)?;
    let mut quads = Vec::new();
    for element in &resolved.elements {
        quads.extend(bake_element(
            element,
            &resolved.textures,
            archive,
            tex_cache,
            None,
        ));
    }
    if quads.is_empty() {
        return None;
    }
    let mut used_textures = HashMap::new();
    for quad in &quads {
        if !used_textures.contains_key(&quad.tex) {
            let tex = load_texture(archive, tex_cache, &quad.tex)?;
            used_textures.insert(quad.tex.clone(), tex);
        }
    }
    Some(BakedModelTextures {
        model: BakedModel {
            quads,
            gui_light_3d: resolved.gui_light_3d,
            transform: resolved.transform,
        },
        textures: used_textures,
    })
}
