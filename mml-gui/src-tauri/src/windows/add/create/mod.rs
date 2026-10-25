//! 添加实例窗口的"新建 / 导入"：文件夹预览、压缩包探测、按文件夹 / 压缩包 / 网址导入
//!
//! 从 `add/mod.rs` 拆出来的。三个导入入口最后都汇到内核的安装流程，任务登记在共享的
//! `modpack_task` 注册表里。命令带分组标记，值填原始组名 add，把组键钉回 `add`，
//! 见 AGENTS.md §4。

pub(crate) mod import;
pub(crate) mod prepare;
