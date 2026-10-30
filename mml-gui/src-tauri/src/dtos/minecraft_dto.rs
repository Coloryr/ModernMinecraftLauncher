//! 从官方启动器 `.minecraft` 导入实例的跨 IPC 传输对象：探测结果 / 版本 / 进度 / 结果

use serde::Serialize;

/// `.minecraft` 探测结果，只用于"首次启动问一次"那个弹窗
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftInfoDto {
    /// 探测到的 `.minecraft` 目录
    pub path: String,
    /// 可导入的版本，按版本 id 排序
    pub versions: Vec<MinecraftVersionDto>,
}

/// 一个可导入的版本
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftVersionDto {
    /// 版本 id，即 `versions` 下的目录名
    pub id: String,
    /// M²L 的版本清单里有没有这个版本号；没有时按自定义版本导入
    pub known: bool,
    /// 加载器 id：`normal` / `forge` / `neoforge` / `fabric` / `quilt` / `custom`
    pub loader: String,
}

/// 导入进度，长任务期间推送
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftProgressDto {
    /// 阶段，目前只有 `copy`
    pub stage: String,
    /// 已复制文件数
    pub done: u32,
    /// 总文件数
    pub total: u32,
    /// 当前文件，相对 `.minecraft` 的路径
    pub text: String,
}

/// 导入结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftImportDto {
    /// 新实例名
    pub name: String,
    /// 实例用的游戏版本
    pub version: String,
    /// 搬进来的文件数
    pub files: u32,
    /// 没搬成功的条目，相对路径
    pub failed: Vec<String>,
}
