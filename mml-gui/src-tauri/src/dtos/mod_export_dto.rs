//! 模组列表导出选项。

use serde::Deserialize;

/// 模组清单的格式、字段与范围。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModExportDto {
    /// 输出文件的完整路径。
    pub file: String,
    /// 文件格式：txt、csv 或 json。
    pub format: String,
    /// 按输出顺序排列的字段名。
    pub fields: Vec<String>,
    /// 仅导出这些模组；为空表示全部顶层模组。
    pub ids: Vec<String>,
}
