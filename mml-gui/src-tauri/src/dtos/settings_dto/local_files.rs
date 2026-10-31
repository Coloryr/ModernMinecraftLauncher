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
    /// 可展开的占用明细；实例行是各实例，下载内容行是 mods / resourcepacks
    pub children: Vec<LocalFileChildDto>,
}

/// 本地文件夹下的明细行
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalFileChildDto {
    /// 稳定 id：实例 uuid，或 mods / resourcepacks
    pub id: String,
    /// 明细类型：instance / mods / resourcepacks
    pub kind: String,
    /// 实例显示名；下载子目录留空，由界面按 kind 翻译
    pub name: String,
    /// 绝对路径，界面悬停展示
    pub path: String,
    /// 占用字节数
    pub size: u64,
}
