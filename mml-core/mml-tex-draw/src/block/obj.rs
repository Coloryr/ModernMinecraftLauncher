use std::collections::HashMap;

use serde::{Deserialize, Serialize};

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
