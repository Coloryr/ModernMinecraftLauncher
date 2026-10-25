//! 资源窗口的窗口模型
//!
//! 这一层是窗口运行态，跟窗口同生共死（登记在 windows/lifecycle 的模型表里）：
//!
//! - **扫描缓存**：模组列表扫一次要几秒，每个 jar 都得把元数据读一遍；而更新检查、
//!   依赖检测要的就是"这个窗口现在看到的这份列表"。列表命令扫完存进来，后面的操作直接读，
//!   不再重扫 `mods` 目录。
//! - **在跑任务的取消令牌**：关窗时逐个 cancel（见 [`cancel_window`]），
//!   扫描 / 检查 / 下载更新这些慢操作在循环里看它，别对着已经关掉的窗口继续跑。
//!
//! 模型按**派生窗口 uuid** 登记，与句柄表、几何同一套键：资源窗口是"一个实例一个窗口"
//! （见 registry 的窗口族），命令侧手里只有实例 uuid，用 [`window_uuid`] 换算。

mod cache;
mod window;

#[cfg(test)]
mod tests;

pub(crate) use self::window::{begin_if_open, cancel_window, instance_busy, window_model};

pub(crate) use self::cache::ModCache;
