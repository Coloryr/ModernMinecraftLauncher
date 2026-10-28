//! 窗口几何：口径换算 + `window_save.json` 读写
//!
//! 从 `windows/mod.rs` 拆出来的。这里是全仓**最贵的一段经验**：
//! 外框 / 客户区两种口径、逻辑 / 物理两种单位、以及"摘掉原生装饰会把标题栏并进
//! 客户区"导致的高度漂移 —— 都集中在这个文件里，改之前先读注释。
//!
//! 口径速记，详见 `WindowState` 的文档：
//! - 存盘 / 恢复一律按**客户区**，即 `inner_size()`，且折算回**带装饰**时的口径；
//! - `WINDOWS_INFO` 里的最小尺寸是**客户区 + 逻辑像素**，见 `super::registry`；
//! - 只有下发给 Windows 的外框最小尺寸才需要加上那圈边框带，见 [`reconcile_min_size`]；
//!   而下限**只在装饰激活后设**：建窗时窗口是带装饰口径，那时算出的下限比恢复值高一个标题栏，
//!   tao 重设尺寸时 Windows 会顺着它把窗口顶上去，实测每开一次高 31px。

mod placement;
mod size;
mod state;

pub use self::state::init;

pub(crate) use self::state::{remember_decorated_inset, save_window_state, window_state_for};

pub(crate) use self::size::{clamp_runtime_min_size, reconcile_min_size};

pub(crate) use self::placement::{Placement, RectF, SizeF, center_placement, child_placement};
