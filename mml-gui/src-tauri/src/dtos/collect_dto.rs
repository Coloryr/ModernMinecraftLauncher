//! 收藏窗口 DTO
//!
//! 磁盘上的 `collect.json` 用 Rust 命名，见 `../collect_utils.rs`，
//! 这里转成 camelCase 的 wire 形态给前端。
//! 类型过滤状态不在这里——它属于 `gui_config`，见 `../gui_config.rs` 的 `CollectConfig`。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::collect_utils::{CollectItemObj, CollectObj};

/// 收藏项
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectItemDto {
    /// 收藏项 uuid，即 `CollectObj::items` 的 key，前端寻址用
    pub uuid: String,
    /// 资源名字
    pub name: String,
    /// 下载源：curseforge / modrinth
    pub source: String,
    /// 资源类型：modpack / mod / resourcepack / shaderpack …
    pub file_type: String,
    /// 项目 ID
    pub pid: String,
    /// 图标地址，URL 形式
    pub icon: Option<String>,
    /// 项目网页地址
    pub url: String,
}

impl CollectItemDto {
    /// 由磁盘收藏项构造
    ///
    /// - `uuid`: `CollectObj::items` 里的 key
    /// - `data`: 磁盘收藏项
    ///
    /// # 返回值
    ///
    /// 返回转换后的 DTO
    pub fn new(uuid: &str, data: &CollectItemObj) -> Self {
        Self {
            uuid: uuid.to_string(),
            name: data.name.clone(),
            source: data.source.to_string(),
            file_type: data.file_type.to_string(),
            pid: data.pid.clone(),
            icon: data.icon.clone(),
            url: data.url.clone(),
        }
    }
}

/// 收藏窗口数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectDataDto {
    /// 收藏项，按名字排序
    pub items: Vec<CollectItemDto>,
    /// 分组：分组名 → 收藏项 uuid 列表
    pub groups: HashMap<String, Vec<String>>,
}

impl From<CollectObj> for CollectDataDto {
    /// 磁盘收藏数据 → 前端 wire 形态
    ///
    /// - `obj`: 磁盘收藏数据
    ///
    /// # 返回值
    ///
    /// 返回转换后的 DTO，收藏项与各分组的 uuid 列表均按名字 / 值排序
    fn from(obj: CollectObj) -> Self {
        let mut items: Vec<CollectItemDto> = obj
            .items
            .iter()
            .map(|(uuid, data)| CollectItemDto::new(uuid, data))
            .collect();
        // `items` 是 HashMap，迭代顺序不稳定；按名字排序保证前端展示稳定
        items.sort_by(|a, b| a.name.cmp(&b.name));

        let groups = obj
            .groups
            .into_iter()
            .map(|(name, uuids)| {
                let mut uuids: Vec<String> = uuids.into_iter().collect();
                uuids.sort();
                (name, uuids)
            })
            .collect();

        Self { items, groups }
    }
}
