//! 跨窗口共用与壳层命令
//!
//! 这里放不属于某个窗口的东西：窗口注册表、窗口几何、窗口生命周期、壳层命令、配置窗口的 NBT 会话，
//! 以及材质、拖放桥、任务表、整页站点、换皮包判定与 WebView2 快捷键。
//! 窗口模块只依赖这里；shell 作为壳层命令可以调用窗口模块，方向是 shell → 窗口 / common。

pub(crate) mod effects;
pub(crate) mod export_file;
#[cfg(windows)]
pub(crate) mod file_drop;
pub(crate) mod geometry;
pub(crate) mod instance_task;
pub(crate) mod lifecycle;
pub(crate) mod modpack_task;
pub(crate) mod nbt_session;
pub(crate) mod registry;
pub(crate) mod shell;
pub(crate) mod theme;
pub(crate) mod web_assets;
#[cfg(windows)]
pub(crate) mod webview_keys;
