//! items/*.json 结构
//!
//! 图标定义：模型节点分派、tint 表与 gui 变换。

use serde::Deserialize;



use super::*;

/// items/*.json 顶层定义
#[derive(Deserialize)]
pub(super) struct ItemDefJson {
    /// 图标模型定义，类型分派根节点
    #[serde(default)]
    pub(super) model: Option<ItemModelJson>,
}

#[derive(Deserialize)]
/// 物品模型定义，按 `type` 分派到不同子结构
pub(super) struct ItemModelJson {
    /// 类型：minecraft:model / composite / condition / select / range_dispatch /
    /// special / empty，缺省视为 model
    #[serde(rename = "type", default)]
    pub(super) kind: Option<String>,
    /// minecraft:model：models/ 下的模型名，如 minecraft:item/apple
    #[serde(default)]
    pub(super) model: Option<ModelRef>,
    /// minecraft:model：tint 表，模型面 tintindex 索引
    #[serde(default)]
    pub(super) tints: Option<Vec<TintJson>>,
    /// minecraft:composite：子模型，各子节点可带 transformation
    #[serde(default)]
    pub(super) models: Option<Vec<ItemModelJson>>,
    /// minecraft:condition：图标取条件不满足的默认态
    #[serde(default)]
    pub(super) on_false: Option<Box<ItemModelJson>>,
    /// 条件满足的分支不渲染图标，仅为完整反序列化保留
    #[serde(default)]
    #[allow(dead_code)]
    pub(super) on_true: Option<Box<ItemModelJson>>,
    /// minecraft:select：取 when 含 "gui" 的 case，否则 fallback
    #[serde(default)]
    pub(super) cases: Option<Vec<CaseJson>>,
    /// select / range_dispatch：兜底，无 entry 阈值命中时才用
    #[serde(default)]
    pub(super) fallback: Option<Box<ItemModelJson>>,
    /// range_dispatch：按阈值分派的条目
    #[serde(default)]
    pub(super) entries: Option<Vec<EntryJson>>,
    /// 节点自带变换，composite 子模型拼合用，ItemTransform 语义
    #[serde(default)]
    pub(super) transformation: Option<TransformDefJson>,
}

/// select 的一个分支
#[derive(Deserialize)]
pub(super) struct CaseJson {
    /// 条件属性取值，含 "gui" 才选中
    #[serde(default)]
    pub(super) when: Option<SelectWhen>,
    /// 命中时使用的模型
    #[serde(default)]
    pub(super) model: Option<ItemModelJson>,
}

/// range_dispatch 的一个分派条目：阈值 + 模型
#[derive(Deserialize)]
pub(super) struct EntryJson {
    /// 阈值命中时使用的模型
    #[serde(default)]
    pub(super) model: Option<ItemModelJson>,
    /// 分派阈值，≤0 的最后一个生效
    #[serde(default)]
    pub(super) threshold: f64,
}

/// select 的 when：字符串或字符串列表，如 "gui" / ["gui", "ground"]
#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum SelectWhen {
    /// 单个条件
    One(String),
    /// 多个条件，命中任一即可
    Many(Vec<String>),
}

/// tint 定义
#[derive(Deserialize)]
pub(super) struct TintJson {
    /// 类型：minecraft:constant / grass / dye / firework 等
    #[serde(rename = "type", default)]
    pub(super) kind: Option<String>,
    /// minecraft:constant 的颜色
    #[serde(default)]
    pub(super) value: Option<i64>,
    /// dye/firework/map_color/potion 等的默认色
    #[serde(default)]
    pub(super) default: Option<i64>,
}

/// 节点 transformation，照 ItemTransform 语义，quaternion 为 [x,y,z,w]
#[derive(Deserialize)]
pub(super) struct TransformDefJson {
    /// 平移，模型空间单位
    #[serde(default)]
    pub(super) translation: Option<Vec<f32>>,
    /// 左旋转四元数 [x,y,z,w]
    #[serde(default)]
    pub(super) left_rotation: Option<Vec<f32>>,
    /// 欧拉角，单位度
    #[serde(default)]
    pub(super) rotation: Option<Vec<f32>>,
    /// 缩放系数
    #[serde(default)]
    pub(super) scale: Option<Vec<f32>>,
    /// 右旋转四元数 [x,y,z,w]
    #[serde(default)]
    pub(super) right_rotation: Option<Vec<f32>>,
}
