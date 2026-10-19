//! 实例导出窗口：导出为 CurseForge / Modrinth 标准整合包
//!
//! 打包逻辑在内核 `mml_game::game_export` 的 `InstanceSettingObj::export`，
//! 这里只做实例文件到在线清单 / overrides 的收集与任务托管：
//! `export_run` 后台跑导出，进度经 `export-progress` 事件上报前端。

pub(crate) mod list;
pub(crate) mod run;

pub use self::list::{emit_export_focus};

