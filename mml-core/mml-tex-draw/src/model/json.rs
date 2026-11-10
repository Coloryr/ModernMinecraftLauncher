//! 模型 json 结构
//!
//! 与游戏内的模型 / 方块状态 json 字段一一对应，只取烘焙要用的部分。
use std::collections::HashMap;

use mml_base::{archives::BaseArchive, serialize_tools};
use serde::Deserialize;

use crate::block::TextureRefObj;

use super::*;

/// ===== 模型 JSON 结构，自带完整 display 解析，26.3 语义 =====

/// 模型 JSON 顶层结构
#[derive(Deserialize)]
struct ModelJson {
    /// 父模型路径，沿链向上合并
    #[serde(default)]
    parent: Option<String>,
    /// 贴图引用表，"#side" → 路径或引用
    #[serde(default)]
    textures: HashMap<String, TextureRefObj>,
    /// 元素集，父模板链上第一个带定义的生效
    #[serde(default)]
    elements: Option<Vec<ElementJson>>,
    /// 各视角 display 变换，只用 gui
    #[serde(default)]
    display: Option<DisplayJson>,
    /// gui_light：side 为默认、双灯3D，front 为单面平光
    #[serde(default)]
    gui_light: Option<String>,
}

/// display 定义，只用 gui 视角
#[derive(Deserialize)]
struct DisplayJson {
    /// gui 视角变换，即物品栏视角
    #[serde(default)]
    gui: Option<TransformJson>,
}

/// gui display 变换，照 ItemTransform 反序列化语义：translation ×0.0625 后 clamp ±5，scale clamp ±4
#[derive(Deserialize)]
struct TransformJson {
    /// 旋转角度，单位度，绕 x/y/z 轴
    #[serde(default)]
    rotation: Vec<f32>,
    /// 平移，原始 JSON 单位，resolved 时 ×0.0625
    #[serde(default)]
    translation: Vec<f32>,
    /// 缩放系数，未写整体为 1
    #[serde(default)]
    scale: Vec<f32>,
}

impl TransformJson {
    /// 反序列化即完成 clamp，照 ItemTransform.Deserializer
    fn resolved(&self) -> GuiTransform {
        let take = |v: &Vec<f32>, i: usize| v.get(i).copied().unwrap_or(0.0);
        let mut translation = [
            take(&self.translation, 0) * 0.0625,
            take(&self.translation, 1) * 0.0625,
            take(&self.translation, 2) * 0.0625,
        ];
        for t in &mut translation {
            *t = t.clamp(-5.0, 5.0);
        }
        let scale = |i: usize| {
            // 未写scale整体为1；写了的按分量缺省1，clamp±4，允许负值，照Mth.clamp
            if self.scale.is_empty() {
                1.0
            } else {
                self.scale.get(i).copied().unwrap_or(1.0).clamp(-4.0, 4.0)
            }
        };
        GuiTransform {
            rotation: [
                take(&self.rotation, 0),
                take(&self.rotation, 1),
                take(&self.rotation, 2),
            ],
            translation,
            scale: [scale(0), scale(1), scale(2)],
        }
    }
}

/// 模型 JSON 的 elements 元素，坐标 0..16
#[derive(Clone, Deserialize)]
pub(crate) struct ElementJson {
    /// 最小角，0..16，缺省 [0,0,0]
    #[serde(default = "d_from")]
    pub(crate) from: Vec<f32>,
    /// 最大角，0..16，缺省 [16,16,16]
    #[serde(default = "d_to")]
    pub(crate) to: Vec<f32>,
    /// 六个面的定义，面名 → face
    #[serde(default)]
    pub(crate) faces: Option<HashMap<String, FaceJson>>,
    /// 元素整体旋转
    #[serde(default)]
    pub(crate) rotation: Option<RotationJson>,
}

/// elements.from 的缺省值，即元素最小角 [0,0,0]，0..16 坐标
fn d_from() -> Vec<f32> {
    vec![0.0, 0.0, 0.0]
}

/// elements.to 的缺省值，即元素最大角 [16,16,16]，0..16 坐标
fn d_to() -> Vec<f32> {
    vec![16.0, 16.0, 16.0]
}

/// 元素 rotation 定义
#[derive(Clone, Deserialize)]
pub(crate) struct RotationJson {
    /// 旋转原点，0..16 坐标，缺省 [8,8,8]
    #[serde(default = "d_rot_origin")]
    pub(crate) origin: Vec<f32>,
    /// 旋转轴，取 x/y/z，缺省 y
    #[serde(default)]
    pub(crate) axis: Option<String>,
    /// 旋转角度，单位度，合法值 ±22.5/±45，缺省 0
    #[serde(default)]
    pub(crate) angle: Option<f32>,
    /// 是否按 1/max|R·axis| 缩放补偿
    #[serde(default)]
    pub(crate) rescale: Option<bool>,
}

/// rotation.origin 的缺省值，即元素旋转原点 [8,8,8]，0..16 坐标
fn d_rot_origin() -> Vec<f32> {
    vec![8.0, 8.0, 8.0]
}

/// 单个面的定义
#[derive(Clone, Deserialize)]
pub(crate) struct FaceJson {
    /// 贴图引用，如 "#side"，指向 textures 表
    #[serde(default)]
    pub(crate) texture: Option<String>,
    /// uv 矩形 `[u0,v0,u1,v1]`，0..16 像素，缺省走 defaultFaceUV 投影
    #[serde(default)]
    pub(crate) uv: Option<Vec<f32>>,
    /// 贴图旋转角，0/90/180/270
    #[serde(default)]
    pub(crate) rotation: Option<u32>,
    /// 染色索引，索引 items/*.json 的 tint 表或群系常量色
    #[serde(default)]
    pub(crate) tintindex: Option<u32>,
}

/// ===== 模型解析与 quad 烘焙，照 FaceBakery/ResolvedModel.findTop 语义 =====

/// 父链解析产物：合并后的贴图表、元素集、gui display、gui_light
pub(crate) struct Resolved {
    /// 父链逐层合并后的贴图引用表，子优先
    pub(crate) textures: HashMap<String, TextureRefObj>,
    /// 第一个带定义的祖先的元素集
    pub(crate) elements: Vec<ElementJson>,
    /// display.gui 变换，取离当前模型最近的祖先
    pub(crate) transform: GuiTransform,
    /// gui_light：true=side 走 3D 光照，false=front 走平面光照
    pub(crate) gui_light_3d: bool,
    /// 链终止于 builtin/generated：平面物品，无模型文件，走extrude挤出
    pub(crate) generated: bool,
}

/// 沿 parent 链向上收集：textures 逐层合并，子优先；elements 取第一个带定义的祖先；
/// display.gui 与 gui_light 取离当前模型最近的祖先。findTop 必须走完整条链：
/// block/block 的 gui 旋转在 cube 找到 elements 之后仍在更上层
///
/// - `archive`: 客户端 jar 归档
/// - `rel`: 模型文件相对路径，如 `block/stone.json`
///
/// # 返回值
///
/// 返回解析产物，任一环节的模型文件读不到 / JSON 无效时返回 `None`
pub(crate) fn resolve(archive: &BaseArchive, rel: &str) -> Option<Resolved> {
    let mut textures: HashMap<String, TextureRefObj> = HashMap::new();
    let mut transform: Option<GuiTransform> = None;
    let mut gui_light: Option<bool> = None;
    let mut elements: Option<Vec<ElementJson>> = None;
    let mut generated = false;
    let mut current = rel.to_string();
    for _ in 0..16 {
        // builtin/generated 是物品生成器标记，走extrude几何，无模型文件，到此为止
        if current == "builtin/generated" {
            generated = true;
            break;
        }
        let data = archive
            .read(&format!("assets/minecraft/models/{current}.json"))
            .ok()?;
        let model = serialize_tools::json_from_bytes::<ModelJson>(&data).ok()?;

        // 子模型同名键覆盖父模板："#up"等自引用占位必须被子模型的实际贴图替换，
        // 如template_bed_head的east:"#east"由white_bed_head的east覆盖
        let mut merged = model.textures;
        for (k, v) in textures {
            merged.insert(k, v);
        }
        textures = merged;
        if transform.is_none() {
            if let Some(t) = model.display.and_then(|d| d.gui) {
                transform = Some(t.resolved());
            }
        }
        if gui_light.is_none() {
            if let Some(l) = model.gui_light {
                gui_light = Some(l == "front");
            }
        }
        if elements.is_none() {
            elements = model.elements;
        }

        let Some(parent) = model.parent else {
            break;
        };
        current = parent
            .strip_prefix("minecraft:")
            .unwrap_or(&parent)
            .to_string();
    }
    Some(Resolved {
        textures,
        // generated链即builtin/generated，无elements也有效，走extrude路径；
        // 普通链缺elements视为无效，父模板
        elements: elements
            .filter(|e| !e.is_empty() || generated)
            .unwrap_or_default(),
        transform: transform.unwrap_or_default(),
        gui_light_3d: !gui_light.unwrap_or(false),
        generated,
    })
}
