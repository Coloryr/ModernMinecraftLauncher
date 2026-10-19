//! 游戏日志的落盘
//!
//! 启动器进程内的日志按行排队，够一批或到间隔再写；
//! 写盘失败只记一条，不让日志本身把启动打断。

use std::sync::{LazyLock, Mutex};

use tauri::{AppHandle, Emitter};

use crate::dtos::LogEvent;
use crate::dtos::main_dto::LogLine;
use crate::listens;
use mml_game::{InstanceLog, InstanceLogType};

use super::read::{now_time, log_line_from_item};


/// 游戏日志事件
///
/// - `app`: 应用句柄，广播用它
/// - `event`: 一行日志或清空指令
#[gui_macros::emit]
pub(super) fn emit_game_log(app: &AppHandle, event: LogEvent) {
    let _ = app.emit(listens::GAME_LOG, event);
}

/// 日志攒批周期，单位 ms；也是前端拿到日志的最大延迟
pub(crate) const LOG_FLUSH_INTERVAL_MS: u64 = 100;

/// 单个实例一批最多带多少行：一次性刷几万行时别把单个事件负载做大，剩下的下一轮再发
pub(super) const LOG_FLUSH_MAX_LINES: usize = 1000;

/// 攒批缓冲上限：前端只留最近 10000 行，攒再多也是白占内存
pub(super) const LOG_PENDING_MAX_LINES: usize = 20_000;

/// 攒批缓冲里的一条待发日志
pub(super) struct PendingLog {
    /// 日志所属实例 uuid
    pub(super) uuid: String,
    /// 已转换好的日志行
    pub(super) line: LogLine,
}

/// 待发日志：内核逐行推来，先攒在这里，由 flusher 按周期成批发出去
pub(super) static PENDING_LOG: LazyLock<Mutex<Vec<PendingLog>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// 一行日志放进攒批缓冲，超上限丢最旧的
///
/// - `uuid`: 日志所属实例 uuid
/// - `line`: 已转换好的日志行
pub(super) fn queue_game_log(uuid: String, line: LogLine) {
    let mut pending = PENDING_LOG.lock().unwrap();
    pending.push(PendingLog { uuid, line });
    if pending.len() > LOG_PENDING_MAX_LINES {
        let drop = pending.len() - LOG_PENDING_MAX_LINES;
        pending.drain(..drop);
    }
}

/// 把攒下的日志按实例成批发给前端
///
/// 每个实例一个事件，单批最多 [`LOG_FLUSH_MAX_LINES`] 行，多出来的留到下一轮 ——
/// 逐行发事件时，游戏一次性刷几千行就是几千次 IPC 往返，日志窗口会被刷到卡住。
///
/// - `app`: 应用句柄，广播用它
pub(crate) fn flush_pending_logs(app: &AppHandle) {
    let mut groups: Vec<(String, Vec<LogLine>)> = Vec::new();
    let mut rest: Vec<PendingLog> = Vec::new();
    {
        let mut pending = PENDING_LOG.lock().unwrap();
        if pending.is_empty() {
            return;
        }
        for item in pending.drain(..) {
            let PendingLog { uuid, line } = item;
            // 日志基本来自同一个实例，线性找一下足够
            let idx = match groups.iter().position(|(u, _)| u == &uuid) {
                Some(idx) => idx,
                None => {
                    groups.push((uuid.clone(), Vec::new()));
                    groups.len() - 1
                }
            };
            if groups[idx].1.len() < LOG_FLUSH_MAX_LINES {
                groups[idx].1.push(line);
            } else {
                rest.push(PendingLog { uuid, line });
            }
        }
        *pending = rest;
    }

    for (uuid, lines) in groups {
        emit_game_log(
            app,
            LogEvent {
                uuid,
                lines,
                clear: false,
            },
        );
    }
}

/// 发出一行游戏日志，自动解析 thread / level / category 筛选字段
///
/// - `app`: 应用句柄，广播用它
/// - `uuid`: 日志所属实例 uuid
/// - `text`: 日志原文
/// - `clear`: 为真时这条是清空指令，前端清掉已有行
///
/// 解析用 mml-core `InstanceRuntimeLog::add_game_log` 同一套正则，
/// 保证事件里的字段与核心侧处理结果一致
pub(crate) fn emit_log_line(app: &AppHandle, uuid: &str, text: &str, clear: bool) {
    let obj = mml_game::game_log::InstanceRuntimeLog::parse_game_log_line(text);
    let line = LogLine {
        time: now_time(),
        text: text.to_string(),
        thread: obj.thread,
        level: obj.level.as_str().to_string(),
        category: obj.category,
    };
    if !clear {
        queue_game_log(uuid.to_string(), line);
        return;
    }

    // 清空是分界线：先把已攒的行发出去，顺序反了会把清空之后的行一起清掉
    flush_pending_logs(app);
    emit_game_log(
        app,
        LogEvent {
            uuid: uuid.to_string(),
            lines: vec![line],
            clear: true,
        },
    );
}

/// 转发内核实例运行日志到前端，lib.rs setup 里订阅 `mml_game::add_run_log`
///
/// - `app`: 应用句柄，广播用它
/// - `log`: 内核抛出的实例日志，含新增一行与清空两种
pub(crate) fn forward_run_log(app: &AppHandle, log: &InstanceLog) {
    let uuid = log.uuid.to_string();
    match &log.log {
        InstanceLogType::AddLog(item) => queue_game_log(uuid, log_line_from_item(item)),
        InstanceLogType::ClearLog => {
            // 同上：清空前先把已攒的行发出去
            flush_pending_logs(app);
            emit_game_log(
                app,
                LogEvent {
                    uuid,
                    lines: Vec::new(),
                    clear: true,
                },
            );
        }
    }
}
