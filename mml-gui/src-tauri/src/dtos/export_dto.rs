//! 实例导出窗口 DTO：前端 wire，camelCase
//!
//! 导出信息 / 目录浏览 / 导出配置 / 导出进度事件的负载。

use serde::{Deserialize, Serialize};

/// 导出时可勾选的一个条目，模组与普通文件共用
///
/// 路径一律**相对游戏目录**（`.minecraft`），如 `mods/xxx.jar`、`config/abc.toml`。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportEntryDto {
    /// 相对游戏目录的路径
    pub path: String,
    /// 是否目录
    pub dir: bool,
    /// 文件大小，单位字节；目录为 0
    pub size: u64,
    /// 下载地址；没有在线来源时是空串
    pub url: String,
    /// 项目 ID，只有在线来源的模组才有
    pub mod_id: Option<String>,
    /// 文件 ID，只有在线来源的模组才有
    pub file_id: Option<String>,
}

/// 实例导出信息，即 `export_get_info` 的返回值
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportInfoDto {
    /// 实例名
    pub name: String,
    /// 游戏版本
    pub version: String,
    /// 加载器类型：Normal / Forge / Fabric / Quilt / NeoForge
    pub loader: String,
    /// 加载器版本
    pub loader_version: Option<String>,
    /// `mods` 目录下的模组，有在线来源的带上项目号与下载地址
    pub mods: Vec<ExportEntryDto>,
}

/// 前端勾选的一个条目，即 `export_run` 入参里的一份
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPickDto {
    /// 相对游戏目录的路径；目录会被展开成其中的文件
    pub path: String,
    /// 下载地址；Modrinth 格式下填了才写进 index，空串进 overrides
    pub url: String,
}

/// 前端提交的导出配置，即 `export_run` 的入参
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportConfigDto {
    /// 导出格式：curseforge / modrinth
    pub pack: String,
    /// 保存位置，前端 save 对话框选好的完整路径
    pub file: String,
    /// 勾选的模组
    pub mods: Vec<ExportPickDto>,
    /// 勾选的文件与目录
    pub files: Vec<ExportPickDto>,
    /// 整合包名
    pub name: String,
    /// 作者
    pub author: String,
    /// 版本
    pub version: String,
    /// 简介
    pub summary: String,
}

/// 导出进度事件，事件名为 `export-progress`
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProgressDto {
    /// 状态：running / done / failed
    pub state: String,
    /// 当前处理数
    pub now: u64,
    /// 总数
    pub total: u64,
    /// 当前处理的文件名
    pub text: String,
    /// 失败原因，failed 时有值
    pub error: Option<String>,
    /// 导出文件保存路径，done 时前端可提示
    pub file: String,
}
