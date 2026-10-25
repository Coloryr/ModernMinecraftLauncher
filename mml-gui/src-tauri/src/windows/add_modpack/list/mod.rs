//! 下载整合包窗口的查询类命令：项目列表 / 详情 / 文件列表，文件即版本
//!
//! 从 `add_modpack/mod.rs` 拆出来的，光 `list_projects` 就有 118 行、`add_modpack_file` 132 行。
//! 列表页顺手把项目与文件写进 `CURSEFOGRE_*` / `MODRINTH_*` 缓存，详情与安装都从缓存取。
//! 命令带分组标记，值填原始组名 add_modpack，把组键钉回 `addModpack`，见 AGENTS.md §4。

pub(crate) mod detail;
mod installed;
pub(crate) mod search;
