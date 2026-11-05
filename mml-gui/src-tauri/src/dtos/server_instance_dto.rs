//! 服务器实例生成 DTO：前端 wire，camelCase
//!
//! 生成选项 / 进度事件 / 结果。清单本身的结构在 `mml-game` 的 `serverpack` 下。

use serde::{Deserialize, Serialize};

/// 生成服务器实例的选项，即 `server_instance_generate` 的入参
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInstanceOptionsDto {
    /// 目标格式：`mml` 写 `server.json`，`hmcl` 写 `server-manifest.json`
    pub format: String,
    /// 输出文件夹，前端目录对话框选好的完整路径；不存在会创建
    pub folder: String,
    /// 发布地址，玩家填进启动器的那个网址；托管文件按它拼出下载地址
    pub url: String,
    /// 服务器实例名，写进清单
    pub name: String,
    /// 包版本，写进 `PackVersion` / HMCL 的 `version`
    pub version: String,
    /// 说明，写进 `Text` / HMCL 的 `description`
    #[serde(default)]
    pub text: String,
    /// 勾选的模组，路径相对游戏目录；目录会展开成其中的文件
    #[serde(default)]
    pub mods: Vec<ServerInstancePickDto>,
    /// 要同步的配置目录，每个目录各自带同步方式
    #[serde(default)]
    pub configs: Vec<ServerInstanceConfigDto>,
    /// 基础文件，相对实例根目录；打包成固定 zip，只在首次同步时下载
    #[serde(default)]
    pub base_files: Vec<String>,
    /// 要同步的资源包文件，路径相对游戏目录，可带手填编号与地址
    #[serde(default)]
    pub resource_packs: Vec<ServerInstancePickDto>,
}

/// 前端勾选的一个路径
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInstancePickDto {
    /// 相对游戏目录的路径；目录会展开成其中的文件
    pub path: String,
    /// 这一条是取消勾选，把该目录或文件从勾选里去掉；默认是勾选
    #[serde(default)]
    pub exclude: bool,
    /// 手动填的下载地址；空串表示用实例记录的那一份
    #[serde(default)]
    pub url: String,
    /// 手动填的项目编号；空串表示用实例记录的那一份
    #[serde(default)]
    pub pid: String,
    /// 手动填的文件编号；空串表示用实例记录的那一份
    #[serde(default)]
    pub fid: String,
    /// 是否使用自定义编号；用于保留「自定义」标记
    #[serde(default)]
    pub custom: bool,
    /// 编号是否被用户改过；改过时优先用手填值，不再套用映射
    #[serde(default)]
    pub edited: bool,
    /// 编号类型是否被用户改过；改过时以 custom 为准
    #[serde(default)]
    pub type_changed: bool,
}

/// 一个要同步的配置目录
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInstanceConfigDto {
    /// 目录名，相对游戏目录，只认顶层目录，如 `config`
    pub dir: String,
    /// 同步方式：`archive` 打成压缩包，`files` 逐个文件下发
    #[serde(default)]
    pub mode: String,
    /// 压缩包同步时是否先删掉目标目录；默认删，与旧行为一致
    #[serde(default = "default_true")]
    pub delete_old: bool,
}

/// 缺省值：默认删除目标目录
fn default_true() -> bool {
    true
}

/// 生成进度事件，事件名 `server-instance-progress`
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInstanceProgressDto {
    /// 当前处理数，从 0 开始
    pub now: u64,
    /// 总数
    pub total: u64,
    /// 当前处理的文件名，相对游戏目录
    pub text: String,
}

/// 已保存的自定义编号，即 `server_instance_get_ids` 的返回值
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInstanceIdDto {
    /// 相对游戏目录的路径
    pub path: String,
    /// 项目编号
    pub project_id: String,
    /// 文件编号
    pub file_id: String,
}

/// 上次生成保存的选项，即 `server_instance_get_meta` 的返回值
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInstanceMetaDto {
    /// 服务器实例名
    pub name: String,
    /// 包版本
    pub version: String,
    /// 说明
    pub text: String,
    /// 发布地址
    pub url: String,
    /// 格式：mml / hmcl
    pub format: String,
    /// 保存文件夹
    pub folder: String,
    /// 勾选的基础文件，相对实例根目录
    pub base_files: Vec<String>,
}

/// 条目变化汇总，用于生成后给模组 / 资源包表格着色
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInstanceChangesDto {
    /// 新增的模组路径
    pub added: Vec<String>,
    /// 删除的模组路径
    pub removed: Vec<String>,
    /// 内容或路径变化的模组路径
    pub modified: Vec<String>,
}

/// 生成结果，即 `server_instance_generate` 的返回值
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInstanceResultDto {
    /// 写出的主清单文件名，如 `server.json`
    pub manifest: String,
    /// 写出的顶层条目，相对输出文件夹：配置压缩包名或复制进去的目录名
    pub files: Vec<String>,
    /// 同步的文件数
    pub count: usize,
    /// 与上次生成相比有变化的相对路径或配置目录
    pub changed: Vec<String>,
    /// 模组新增 / 删除 / 修改汇总
    pub mod_changes: ServerInstanceChangesDto,
    /// 资源包新增 / 删除 / 修改汇总
    pub pack_changes: ServerInstanceChangesDto,
}
