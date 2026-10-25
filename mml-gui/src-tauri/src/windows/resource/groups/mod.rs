//! 资源管理窗口的模组自定义分组与备注：分组表读写、顺序、折叠、成员、备注
//!
//! 分组数据落在实例的 `gui_setting.json` 的 `Mod.Groups` / `Mod.ModName`，与 ColorMC 同一套
//! 口径；分组以 **SHA1** 为成员身份，启用 / 禁用只改文件名，SHA1 不变。
//!
//! 所有写入都经 [`edit_mod_setting`]：它用**写锁**把"读文件 → 改 → 写回整份"串起来，
//! 避免几个命令并发时互相覆盖。

pub(crate) mod cmds;
mod state;
