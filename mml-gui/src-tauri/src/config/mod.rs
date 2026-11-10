//! 配置与用户数据
//!
//! 这些文件都落在运行根目录，由 Rust 读写：GUI 配置、实例 GUI 设置、客户端设置锁、
//! 收藏、方块分组与跨电脑绑定。统一口径是内存一份 + `config_save` 异步落盘，
//! 前端只经 IPC 整份读写，不把永久数据放进 localStorage。

pub mod block_groups;
pub(crate) mod client_lock;
pub mod collect_utils;
pub mod gui_config;
pub mod gui_setting;
pub mod integrations;
