//! 资源管理窗口的模组更新检查：查有在线信息的模组在平台上有没有新文件
//!
//! 只认实例 `online_info.json` 里记着的模组，按 SHA1 关联，项目号与文件号都非空才算；
//! 手动放进 mods 的包不在表里，无从判断有没有更新，直接跳过。
//!
//! 判定口径：按实例的游戏版本与加载器过滤，取平台上**最新**的那个文件，
//! 文件号与装着的不一样就算有更新。平台在当前版本 / 加载器下一个文件都没有时不报 ——
//! 那说明这个包在本实例上没有可用的新版本，不是漏了。
//!
//! 检查要重扫一遍 mods 目录（在线信息按 SHA1 关联），代价与打开模组分类相同；
//! 之后逐个问平台，进度经 `resource-mods-update-progress` 事件上报。

pub(crate) mod check;
mod emit;
mod newfile;
mod prepare;
mod query;
pub(crate) mod record;
pub(crate) mod run;

#[cfg(test)]
mod tests;

pub(super) use self::query::{LatestFile, newest_curseforge, newest_modrinth};
