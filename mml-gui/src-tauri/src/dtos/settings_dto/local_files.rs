//! 设置窗口「本地文件」标签的 DTO：运行目录下各个文件夹的占用
//!
//! 只描述现状，不含操作参数；清理与打开目录都按 `id` 定位，见 `windows/settings/local_files.rs`。

use serde::{Deserialize, Serialize};

/// 一个本地文件夹的占用
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalFileEntryDto {
    /// 稳定 id，如 `cache` / `image` / `download` / `webview` / `instances` / `java`
    pub id: String,
    /// 文件夹绝对路径，界面直接展示
    pub path: String,
    /// 占用字节数
    pub size: u64,
    /// 是否允许清理；界面数据、游戏实例这类只统计不删除
    pub deletable: bool,
}
