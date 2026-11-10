//! 模型引用解析
//!
//! 把 items/*.json 的模型节点展开成子模型列表，并算出 tint 与变换矩阵。

use glam::{Mat4, Quat, Vec3};
use serde::Deserialize;

use crate::{
    block::GRASS_TINT,
    model::{
        FACE_INFO, calculate_facing,
        face_index_of, face_normal, recalculate_winding,
        select_extent,
    },
};


use super::*;

/// 一个可渲染子模型，composite 展平后的项
pub(super) struct ChildModel {
    /// models/ 下的相对名，如 item/apple
    pub(super) model: String,
    /// tint 表，tintindex 索引
    pub(super) tints: Vec<[u8; 3]>,
    /// 节点自带 transformation，烘焙期直接作用于顶点
    pub(super) transform: Option<Mat4>,
}

/// 解析 model 节点为可渲染子模型列表；special/empty/未知类型返回 None，即跳过该物品
///
/// - `node`: items/*.json 的 model 节点，递归展开 composite 分派
///
/// # 返回值
///
/// 返回展平后的子模型列表，类型不可渲染或引用缺失时返回 `None`
pub(super) fn resolve_item_models(node: &ItemModelJson) -> Option<Vec<ChildModel>> {
    match node.kind.as_deref() {
        None | Some("minecraft:model") => {
            let model = match node.model.as_ref()? {
                ModelRef::Name(name) => strip_ns(name),
                // special 的对象形式不会走到这里，kind 已分派
                ModelRef::Object(_) => return None,
            };
            Some(vec![ChildModel {
                model,
                tints: node
                    .tints
                    .as_ref()
                    .map(|list| list.iter().map(tint_color).collect())
                    .unwrap_or_default(),
                transform: node.transformation.as_ref().map(transform_matrix),
            }])
        }
        Some("minecraft:composite") => {
            let mut out = Vec::new();
            for child in node.models.as_ref()? {
                out.extend(resolve_item_models(child)?);
            }
            Some(out)
        }
        // 图标取条件不满足的默认态，如指南针非 lodestone 绑定、bundle 未满
        Some("minecraft:condition") => resolve_item_models(node.on_false.as_ref()?),
        Some("minecraft:select") => {
            let case = node.cases.as_ref()?.iter().find(|c| match &c.when {
                Some(SelectWhen::One(s)) => s == "gui",
                Some(SelectWhen::Many(list)) => list.iter().any(|s| s == "gui"),
                None => false,
            });
            match case {
                Some(c) => resolve_item_models(c.model.as_ref()?),
                None => resolve_item_models(node.fallback.as_ref()?),
            }
        }
        // range_dispatch：静态图标属性值恒为0，取最后一个阈值≤0的entry；
        // 指南针/时钟等的entries从threshold 0.0起，fallback缺省
        Some("minecraft:range_dispatch") => {
            let chosen = node
                .entries
                .as_ref()?
                .iter()
                .filter(|e| e.threshold <= 0.0)
                .last()
                .and_then(|e| e.model.as_ref())
                .or_else(|| node.fallback.as_deref())?;
            resolve_item_models(chosen)
        }
        // special 实体渲染 / empty / 未知类型跳过
        _ => None,
    }
}

/// 去掉 "minecraft:" 命名空间前缀
///
/// - `name`: 模型名，可带命名空间前缀
///
/// # 返回值
///
/// 返回去前缀后的相对名
pub(super) fn strip_ns(name: &str) -> String {
    name.strip_prefix("minecraft:").unwrap_or(name).to_string()
}

/// minecraft:model 引用的模型名。special 节点的 model 是对象，
/// 如 {"type":"minecraft:trident"}，仅作占位让整文件能反序列化；
/// special 在解析期已被跳过，用不到该字段
#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum ModelRef {
    /// models/ 下的相对名，可带命名空间前缀
    Name(String),
    /// 对象形式，special 的 model.type 等
    #[allow(dead_code)]
    Object(serde_json::Map<String, serde_json::Value>),
}

/// tint 节点 → RGB：grass 类型按平原群系常量色，其余取 ARGB int
///
/// - `t`: tint 定义
///
/// # 返回值
///
/// 返回 RGB 染色，0..255
pub(super) fn tint_color(t: &TintJson) -> [u8; 3] {
    match t.kind.as_deref() {
        Some("minecraft:grass") => GRASS_TINT,
        _ => argb_rgb(t.value.or(t.default).unwrap_or(-1)),
    }
}

/// ARGB int → [r, g, b]，输入可为负
///
/// - `v`: ARGB 色值，i64，可为负，如 -1 = 白
///
/// # 返回值
///
/// 返回 RGB 分量，0..255
pub(super) fn argb_rgb(v: i64) -> [u8; 3] {
    let u = v as u32;
    [(u >> 16) as u8, (u >> 8) as u8, u as u8]
}

/// 节点 transformation → 矩阵，照 ItemTransform 语义：T·leftRot·R(euler度)·S·rightRot
///
/// - `t`: transformation 定义
///
/// # 返回值
///
/// 返回组合后的变换矩阵
pub(super) fn transform_matrix(t: &TransformDefJson) -> Mat4 {
    let take = |v: &Option<Vec<f32>>, i: usize, d: f32| {
        v.as_ref().and_then(|v| v.get(i).copied()).unwrap_or(d)
    };
    let quat = |q: &Option<Vec<f32>>| {
        Quat::from_xyzw(
            take(q, 0, 0.0),
            take(q, 1, 0.0),
            take(q, 2, 0.0),
            take(q, 3, 1.0),
        )
    };
    Mat4::from_translation(Vec3::new(
        take(&t.translation, 0, 0.0),
        take(&t.translation, 1, 0.0),
        take(&t.translation, 2, 0.0),
    )) * Mat4::from_quat(quat(&t.left_rotation))
        * Mat4::from_rotation_x(take(&t.rotation, 0, 0.0).to_radians())
        * Mat4::from_rotation_y(take(&t.rotation, 1, 0.0).to_radians())
        * Mat4::from_rotation_z(take(&t.rotation, 2, 0.0).to_radians())
        * Mat4::from_scale(Vec3::new(
            take(&t.scale, 0, 1.0),
            take(&t.scale, 1, 1.0),
            take(&t.scale, 2, 1.0),
        ))
        * Mat4::from_quat(quat(&t.right_rotation))
}

/// ===== bake：extrude 挤出，照反编译 ItemModelGenerator；以及物品模型烘焙 =====

/// 一个面的顶点数据，烘进 `Quad` 前仍是模型空间
pub(super) struct FaceData {
    /// 4 个顶点位置，模型空间 0..1
    pub(super) pos: [[f32; 3]; 4],
    /// 4 个顶点 uv，0..1，与 `pos` 同序
    pub(super) uv: [[f32; 2]; 4],
    /// 面法线，轴向单位向量；朝向无法量化时取 (0,1,0)
    pub(super) normal: [f32; 3],
}

/// bakeQuad 核心，无元素旋转，照 FaceBakery.bakeQuad：FaceInfo 角选点/16 → uv 角映射
/// 即 Quadrant.R0，再做 calculateFacing 量化 + 规范角序重排。
/// from/to 可为退化或翻转盒，extrude 裙边即是，照反编译原样传入即可
///
/// - `from`: 盒子最小角，0..16 坐标
/// - `to`: 盒子最大角，0..16 坐标
/// - `facing`: 面索引，0=down..5=east
/// - `uvs`: uv 矩形 `[u0,v0,u1,v1]`，0..16 像素
///
/// # 返回值
///
/// 见 [`FaceData`]；位置与 uv 都已归一到 0..1 模型空间
pub(super) fn bake_face(from: &[f32; 3], to: &[f32; 3], facing: usize, uvs: [f32; 4]) -> FaceData {
    let corner = |i: usize| -> [f32; 3] {
        let info = FACE_INFO[facing][i];
        [
            select_extent(info.x, from, to) / 16.0,
            select_extent(info.y, from, to) / 16.0,
            select_extent(info.z, from, to) / 16.0,
        ]
    };
    let mut pos: [[f32; 3]; 4] = std::array::from_fn(corner);
    let mut uv: [[f32; 2]; 4] = std::array::from_fn(|i| {
        let idx = i % 4; // Quadrant.R0
        let u = if idx == 0 || idx == 1 { uvs[0] } else { uvs[2] };
        let v = if idx == 0 || idx == 3 { uvs[1] } else { uvs[3] };
        [u / 16.0, v / 16.0]
    });
    // 最终朝向与角序由 calculateFacing 量化归一，与游戏一致
    let f = calculate_facing(&pos).and_then(|dir| face_index_of(Vec3::from(dir)));
    if let Some(f) = f {
        recalculate_winding(&mut pos, &mut uv, f);
    }
    let normal = f
        .map(|f| face_normal(f).to_array())
        .unwrap_or([0.0, 1.0, 0.0]);
    FaceData { pos, uv, normal }
}
