//! 游戏日志窗口：实例运行日志与历史日志文件的查看
//!
//! 实时日志流走全局 `game-log` 事件，壳层在 lib.rs setup 里订阅 mml-core
//! `add_run_log` 转发，前端按事件负载里的实例 uuid 过滤；历史运行日志复用
//! `main_get_game_log` 拉取；本模块补 logs 与 crash-reports 目录的日志文件浏览。
//! **一个实例一个日志窗口**：窗口身份由实例 uuid 派生、打开时就锁定，
//! 同一实例重复打开只聚焦，见 `windows/registry` 的窗口族。
//!
//! **日志行转换也归本模块**：核心日志条目 → 前端行的 `log_line_from_item`、实时日志
//! 事件转发 `forward_run_log` / `emit_game_log` / `emit_log_line` 都放在这里。
//! 主窗口只负责按钮开窗这个"入口"与启动 / 退出时推一行状态，转换逻辑不跟着它走。

pub(crate) mod read;
mod emit;
pub(crate) mod setting;

pub(crate) use self::emit::{LOG_FLUSH_INTERVAL_MS, flush_pending_logs, emit_log_line, forward_run_log};
