//! 添加资源窗口的状态 DTO：下载任务条目 / 总览快照 / 存档选择项
//!
//! 从 `add_resource_dto/mod.rs` 拆出来的，即 `add-resource-status` 事件负载与查询返回。

use serde::{Deserialize, Serialize};

/// 实例存档条目，数据包安装时选择目标存档用
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSaveDto {
    /// 存档名，取自 level.dat 里的 LevelName
    pub name: String,
    /// 存档文件夹名，即 saves/ 下的目录名
    pub dir: String,
}

/// 资源下载任务条目，供添加资源窗口顶部进度条显示
///
/// `done` / `failed` / `cancelled` 为终态标记，三者互斥，与整合包任务同一套口径。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTaskDto {
    /// 项目 ID
    pub pid: String,
    /// 文件 ID
    pub fid: String,
    /// 显示名
    pub name: String,
    /// 项目图标地址；空串表示没有，界面用首字母占位
    pub icon: String,
    /// 下载进度，0–100
    pub progress: f64,
    /// 下载完成
    pub done: bool,
    /// 下载失败
    pub failed: bool,
    /// 下载被取消
    pub cancelled: bool,
}

/// 资源下载任务总览，即 add-resource-status 事件负载 / add_resource_status 返回
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceStatusDto {
    /// 添加资源窗口是否打开，关闭时由主窗口显示进度条
    pub window_open: bool,
    /// 任务列表
    pub tasks: Vec<ResourceTaskDto>,
}
