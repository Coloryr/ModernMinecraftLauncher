//! 窗口生命周期：句柄表 / 模型表 / 建窗 / 开关窗 / 窗口事件
//!
//! 从 `windows/mod.rs` 拆出来的。这里管"窗口从生到死"：建窗，含几何恢复 + 装饰时机；
//! 句柄与模型的登记清理、关闭保护，以及 `on_window_event` 里的关闭 / 销毁 / 缩放事件。
//! 最小尺寸不在这里设，原因见 [`create_window`] 的说明。
//!
//! **`window_*` 命令留在 `mod.rs`**：`bindings.ts` 的组键取"来源 .rs 模块最后一段"，
//! 命令搬文件会让组键从 `windows` 变成别的名字，前端会断，见 AGENTS.md §4。

mod models;
mod create;
mod close;
mod events;

pub(crate) use self::models::{window_model, window_model_by_uuid, window_model_or_insert, model_for_kind, ensure_model_for_kind, drop_model_for_kind};

#[cfg(test)]
pub(crate) use self::models::{remove_window_model};

pub use self::close::{show_main_window};

pub(crate) use self::close::{close_instance_windows, close_window, close_window_from_uuid, open_window, open_window_from_uuid};

pub use self::events::{on_window_event};
