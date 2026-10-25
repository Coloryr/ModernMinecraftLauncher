//! 添加资源窗口的列表与详情查询：项目列表的搜索与分类翻页，某项目的文件列表与项目详情
//!
//! 从 `add_resource/mod.rs` 拆出来的几个大函数。列表页顺手把项目与文件数据写进
//! `CURSEFOGRE_*` / `MODRINTH_*` 缓存，详情与下载都从缓存里取。

pub(crate) mod detail;
mod query;
pub(crate) mod search;
