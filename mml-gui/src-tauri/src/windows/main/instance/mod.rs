//! 主窗口的实例与分组写操作：新建 / 改名 / 改配置 / 删除 / 移动
//!
//! 从 `main/mod.rs` 拆出来的，光 `main_update_instance` 一个人就 106 行。命令带
//! 分组标记，值填原始组名 main，把组键钉回 `main`，见 AGENTS.md §4。

pub(crate) mod group;
pub(crate) mod edit;
