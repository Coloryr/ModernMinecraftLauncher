//! 实例导出窗口 DTO：前端 wire，camelCase
//!
//! 导出信息 / 导出配置 / 导出进度事件的负载。

use serde::{Deserialize, Serialize};

/// 单个模组文件的导出信息
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportModDto {
    /// 文件名，即 mods 目录下的文件名
    pub name: String,
    /// 是否有在线来源，即 mod_info.json 里有 modid / fileid
    pub online: bool,
    /// 项目 ID，在线模组才有
    pub mod_id: Option<String>,
    /// 文件 ID，在线模组才有
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
    /// 在线模组，进 manifest / index 下载清单
    pub online_mods: Vec<ExportModDto>,
    /// 本地模组，打进 overrides
    pub local_mods: Vec<ExportModDto>,
    /// config 目录是否有内容
    pub has_config: bool,
    /// resourcepacks 目录是否有内容
    pub has_resource_packs: bool,
    /// shaderpacks 目录是否有内容
    pub has_shader_packs: bool,
}

/// 前端提交的导出配置，即 `export_run` 的入参
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportConfigDto {
    /// 导出格式：curseforge / modrinth
    pub pack: String,
    /// 保存位置，前端 save 对话框选好的完整路径
    pub file: String,
    /// 包含模组
    pub include_mods: bool,
    /// 包含配置文件
    pub include_config: bool,
    /// 包含资源包
    pub include_resource_packs: bool,
    /// 包含光影包
    pub include_shader_packs: bool,
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
