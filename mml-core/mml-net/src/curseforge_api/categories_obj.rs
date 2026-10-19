//! CurseForge 分类 DTO

use serde::{Deserialize, Serialize};

/// CurseForge 分类列表，对应 `categories` 接口响应
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct CurseForgeCategoriesObj {
    /// 分类数据列表，对应响应键 `data`
    pub data: Vec<CurseForgeCategoriesDataObj>,
}

/// 单个分类
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct CurseForgeCategoriesDataObj {
    /// 分类 ID，对应响应键 `id`
    pub id: u64,
    /// 分类名称，对应响应键 `name`
    pub name: String,
    /// 所属大类 ID，对应响应键 `classId`，用于区分模组 / 整合包等
    #[serde(rename = "classId")]
    pub class_id: u32,
}
