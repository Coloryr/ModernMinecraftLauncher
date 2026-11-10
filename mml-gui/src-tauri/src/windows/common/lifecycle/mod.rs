//! 窗口生命周期：句柄表 / 模型表 / 建窗 / 开关窗 / 窗口事件
//!
//! 从 `windows/lifecycle` 收进 common：这里管"窗口从生到死"——建窗，含几何恢复 +
//! 装饰时机；句柄与模型的登记清理、关闭保护，以及 `on_window_event` 里的关闭 / 销毁 /
//! 缩放事件。最小尺寸不在这里设，原因见 `create.rs` 里 `create_window` 的说明。

mod close;
mod create;
mod events;
mod models;

pub(crate) use self::close::{
    close_instance_windows, close_other_windows, close_window, close_window_from_uuid, open_window,
    open_window_from_uuid,
};
pub use self::close::show_main_window;
pub use self::events::on_window_event;
pub(crate) use self::models::{
    drop_model_for_kind, ensure_model_for_kind, model_for_kind, window_model, window_model_by_uuid,
    window_model_or_insert,
};
#[cfg(test)]
pub(crate) use self::models::remove_window_model;
