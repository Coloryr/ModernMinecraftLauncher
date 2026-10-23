//! 资源管理窗口的模组分类：扫描列表 / 启用禁用 / 删除 / 分组用的行数据
//!
//! 列表由内核 `read_mod` 扫描，含 jar-in-jar 的嵌套项与模组元数据；进度经
//! `resource-list-mods-progress` 事件上报；启用 / 禁用只改文件名，加减 `.disabled` 后缀，
//! 删除进回收站。
//!
//! `mod_note` / `mod_note_key` 是 `pub(super)`：模组备注的读写命令在 `mod.rs`
//! 的分组段里，测试也在那边。

pub(crate) mod list;
pub(crate) mod export;
mod item;
pub(crate) mod actions;

pub(super) use self::list::{ModDependencyRef, mod_dependencies};

#[cfg(test)]
pub(super) use self::item::mod_note;
pub(super) use self::item::{find_mod, mod_note_key, sha1_of};
