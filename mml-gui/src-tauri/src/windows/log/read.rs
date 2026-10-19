//! 日志读取
//!
//! 列日志文件、按行读回，并把内核的日志条目转成前端要的 DTO；
//! 解析规则跟着内核的日志格式走。

use std::path::PathBuf;

use mml_game::game_log::{GameLog, GameLogItemObj, InstanceRuntimeLog};

use crate::dtos::main_dto::LogLine;
use crate::err_keys;
use crate::log_text::LogText;

use crate::windows::instance_handle;


/// 列出实例的日志文件，取自 logs 与 crash-reports 目录，返回绝对路径
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 日志文件绝对路径列表；uuid 非法或实例不存在返回错误文案
#[gui_macros::ipc_group("log")]
#[tauri::command]
pub fn log_get_files(uuid: String) -> Result<Vec<String>, String> {
    let instance = instance_handle(&uuid)?;
    let game = instance.read().unwrap();
    Ok(game
        .get_log_files()
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

/// 读取单个日志文件并解析为日志行
///
/// - `uuid`: 实例 uuid
/// - `path`: `log_get_files` 返回的路径，仅接受列表内的路径，避免任意文件读取
///
/// # 返回值
///
/// 解析出的日志行；路径不在列表内或实例不存在返回错误文案
#[gui_macros::ipc_group("log")]
#[tauri::command]
pub fn log_read_file(uuid: String, path: String) -> Result<Vec<LogLine>, String> {
    let instance = instance_handle(&uuid)?;
    let game = instance.read().unwrap();
    let path = PathBuf::from(&path);
    if !game.get_log_files().contains(&path) {
        return Err(err_keys::FILE_NOT_FOUND.to_string());
    }
    let runtime = InstanceRuntimeLog::from_file(&path, game.encoding);
    let logs = runtime.logs.read().unwrap();
    Ok(logs.iter().map(log_line_from_item).collect())
}

/// 当前时间，格式 `时:分:秒.毫秒`，用作日志行时间戳
pub(super) fn now_time() -> String {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = d.as_secs() % 86400;
    let ms = d.subsec_millis();
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60,
        ms
    )
}

/// 内核日志条目的捕获时间，格式 `时:分:秒.毫秒`
pub(super) fn item_time(item: &GameLogItemObj) -> String {
    item.time.format("%H:%M:%S%.3f").to_string()
}

/// 把内核日志条目拼成一份可上传的文本，一行一条
///
/// - `items`: 内核日志条目迭代器
///
/// # 返回值
///
/// 换行拼接后的日志全文
pub(super) fn join_log_lines<'a, I: Iterator<Item = &'a GameLogItemObj>>(items: I) -> String {
    items
        .map(|item| log_line_from_item(item).text)
        .collect::<Vec<_>>()
        .join("\n")
}

/// 内核日志条目 → 前端日志行
///
/// - `item`: 内核日志条目
///
/// # 返回值
///
/// 前端日志行；启动器消息变体如耗时 / 路径 / 参数等转为可读文本，标准游戏日志直接取
/// 解析四字段，行内无时间戳时回退条目捕获时间
pub(crate) fn log_line_from_item(item: &GameLogItemObj) -> LogLine {
    let (mut time, text, thread, level, category) = match &item.log {
        GameLog::GameLog(obj) => (
            obj.time.clone(),
            obj.log.clone(),
            obj.thread.clone(),
            obj.level.as_str().to_string(),
            obj.category.clone(),
        ),
        GameLog::Text(s) => (
            item_time(item),
            s.clone(),
            String::new(),
            String::new(),
            String::new(),
        ),
        GameLog::RuntimeLib(p) => (
            item_time(item),
            LogText::LogRuntimeLib {
                path: p.display().to_string(),
            }
            .text(),
            String::new(),
            String::new(),
            String::new(),
        ),
        GameLog::JavaRedirect => (
            item_time(item),
            LogText::LogJavaRedirect.text(),
            String::new(),
            String::new(),
            String::new(),
        ),
        GameLog::JavaLocalRedirect => (
            item_time(item),
            LogText::LogJavaLocalRedirect.text(),
            String::new(),
            String::new(),
            String::new(),
        ),
        GameLog::LoginTime(d) => (
            item_time(item),
            LogText::LogLoginTime {
                secs: d.as_secs_f64(),
            }
            .text(),
            String::new(),
            String::new(),
            String::new(),
        ),
        GameLog::ServerPackCheckTime(d) => (
            item_time(item),
            LogText::LogServerPackCheckTime {
                secs: d.as_secs_f64(),
            }
            .text(),
            String::new(),
            String::new(),
            String::new(),
        ),
        GameLog::CheckGameFileTime(d) => (
            item_time(item),
            LogText::LogCheckGameFileTime {
                secs: d.as_secs_f64(),
            }
            .text(),
            String::new(),
            String::new(),
            String::new(),
        ),
        GameLog::DownloadFileTime(d) => (
            item_time(item),
            LogText::LogDownloadFileTime {
                secs: d.as_secs_f64(),
            }
            .text(),
            String::new(),
            String::new(),
            String::new(),
        ),
        GameLog::LaunchTime(d) => (
            item_time(item),
            LogText::LogLaunchTime {
                secs: d.as_secs_f64(),
            }
            .text(),
            String::new(),
            String::new(),
            String::new(),
        ),
        GameLog::CmdPreTime(d) => (
            item_time(item),
            LogText::LogCmdPreTime {
                secs: d.as_secs_f64(),
            }
            .text(),
            String::new(),
            String::new(),
            String::new(),
        ),
        GameLog::CmdPostTime(d) => (
            item_time(item),
            LogText::LogCmdPostTime {
                secs: d.as_secs_f64(),
            }
            .text(),
            String::new(),
            String::new(),
            String::new(),
        ),
        GameLog::LaunchArgs(s) => (
            item_time(item),
            s.clone(),
            String::new(),
            String::new(),
            String::new(),
        ),
        GameLog::JavaPath(p) => (
            item_time(item),
            LogText::LogJavaPath {
                path: p.display().to_string(),
            }
            .text(),
            String::new(),
            String::new(),
            String::new(),
        ),
    };
    if time.is_empty() {
        time = item_time(item);
    }
    LogLine {
        time,
        text,
        thread,
        level,
        category,
    }
}
