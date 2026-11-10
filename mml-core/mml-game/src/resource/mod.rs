//! 实例内资源
//!
//! 模组、资源包、光影包、存档、截图、结构文件、数据包与资源链接。
//! 这些模块都按实例的游戏目录工作，对外仍按原模块名从 crate 根导出。

pub mod class_scan;
pub mod data_res;
pub mod game_link;
pub mod game_mods;
pub mod game_resourcepacks;
pub mod game_saves;
pub mod game_schematics;
pub mod game_screenshots;
pub mod game_shaderpacks;
