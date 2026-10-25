//! 主窗口的查询类命令：实例 / 分组 / 参数 / Java / 内存 / 版本 / 新闻 / MOTD
//!
//! 从 `main/mod.rs` 拆出来的。命令带分组标记，值填原始组名 main，把组键钉回
//! `main` —— 否则组键会跟着文件名变成 `query`，前端 `commands.main.*` 全断，见 AGENTS.md §4。

pub(crate) mod instance;
pub(crate) mod runtime;
