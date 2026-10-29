//! 资源管理窗口的模组分类：扫描列表 / 启用禁用 / 删除 / 分组用的行数据
//!
//! 列表由内核 `read_mod` 扫描，含 jar-in-jar 的嵌套项与模组元数据；进度经
//! `resource-list-mods-progress` 事件上报；启用 / 禁用只改文件名，加减 `.disabled` 后缀，
//! 删除进回收站。
//!
//! 备注的定位键 `mod_note_key` 与读取 `mod_note` 在 `item` 里：写备注的命令在
//! `groups/cmds.rs`，`mod.rs` 只把 `mod_note` 再导出给测试。

pub(crate) mod actions;
pub(crate) mod export;
mod item;
pub(crate) mod list;

pub(super) use self::list::{ModDependencyRef, mod_dependencies};

#[cfg(test)]
pub(super) use self::item::mod_note;
pub(super) use self::item::{find_mod, mod_note_key, sha1_of};
