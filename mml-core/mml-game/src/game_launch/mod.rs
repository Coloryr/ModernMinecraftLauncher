//! 游戏实例启动
//!
//! 登录、文件检查、拼启动命令、起进程与退出轮询。原先的单个大文件按阶段拆开，
//! 公开类型与句柄仍从本模块导出。

mod auth;
mod check;
mod cmd;
mod handle;
mod java;
mod start;
mod types;

pub(crate) use self::handle::InstanceHandle;
pub use self::types::{
    AutoJoinType, GameLaunchArg, GameRunObj, LaunchCmd, LaunchDebugger, MixinArg,
};

#[cfg(test)]
#[path = "../../tests/unit/game_launch_tests.rs"]
mod tests;
