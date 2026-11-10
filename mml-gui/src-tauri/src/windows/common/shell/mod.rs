//! 壳层命令
//!
//! 不归属某个具体窗口、但由前端 `commands.windows.*` 调用的命令：
//! 窗口开关、外观 / 全屏 / 装饰、GUI 配置、拖入探测、旧数据升级与进程生命周期。
//! 各窗口自己的命令仍在 `windows/<窗口>` 下。

pub(crate) mod chrome;
pub(crate) mod drop;
pub(crate) mod drop_probe;
pub(crate) mod events;
pub(crate) mod gui_config;
pub(crate) mod lifecycle;
pub(crate) mod open;
pub(crate) mod shared;
pub(crate) mod upgrade;
