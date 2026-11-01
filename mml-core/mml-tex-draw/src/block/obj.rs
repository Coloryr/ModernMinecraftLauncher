use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// 一组共用同一张贴图的 2D 面
///
/// 顶面和底面用同一张图很常见（石头、原木的年轮面），这时它们合成一组，
/// 界面上只给一个选项，而不是两个一模一样的。
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct BlockFace2d {
    /// 组内的面名，按顶 / 侧 / 底顺序，如 `["top", "bottom"]`
    #[serde(rename = "Faces")]
    pub faces: Vec<String>,
    /// 平面贴图文件名
    #[serde(rename = "File")]
    pub file: String,
}

/// 方块渲染结果数据
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct BlocksObj {
    /// 已经渲染好的游戏版本
    #[serde(rename = "Id")]
    pub id: String,
    /// 方块ID → 图标 PNG 文件名
    #[serde(rename = "Tex")]
    pub tex: HashMap<String, String>,
    /// 方块ID → 2D 平面贴图的分组，按贴图去重，与 [`Self::tex`] 一一对应
    #[serde(rename = "Tex2d")]
    pub tex2d: HashMap<String, Vec<BlockFace2d>>,
    /// 方块ID → 语言键，翻译经 `get_lang`
    #[serde(rename = "Name")]
    pub name: HashMap<String, String>,
    /// 创造模式分类，取 itemGroup lang键尾段，如 buildingBlocks/natural
    #[serde(rename = "Cat")]
    pub cat: HashMap<String, String>,
}

/// 物品渲染结果数据
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct ItemsObj {
    /// 已经渲染好的游戏版本
    #[serde(rename = "Id")]
    pub id: String,
    /// 物品ID → 图标 PNG 文件名
    #[serde(rename = "Tex")]
    pub tex: HashMap<String, String>,
    /// 物品ID → 语言键，翻译经 `get_lang`
    #[serde(rename = "Name")]
    pub name: HashMap<String, String>,
    /// 创造模式分类，取 itemGroup lang键尾段，与BlocksObj.cat同一套
    #[serde(rename = "Cat")]
    pub cat: HashMap<String, String>,
}
