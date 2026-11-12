//! 启动器自身的日志：帮助窗口里的「打开日志 / 上传日志」
//!
//! 与实例日志（日志窗口里那份游戏运行日志）不是一回事：这里是 `mml_log` 写在运行目录根的
//! `logs.log`，里面有 panic、前端错误、下载与渲染这些启动器自己的记录 —— 反馈问题时要看的就是它。

use std::path::PathBuf;

use mml_names::names;

use crate::err_keys;

/// 上传行数上限：启动器日志动辄几 MB，只发尾部
///
/// 前面绝大部分是启动过程，贴给谁看都得先翻到底；尾部才是出事那段，也避开对方对体积的限制。
const UPLOAD_MAX_LINES: usize = 1000;

/// 启动器日志文件：运行目录根的 `logs.log`
fn launcher_log_file() -> PathBuf {
    mml_base::get_base_dir().join(names::LOG_FILE)
}

/// 用系统默认程序打开启动器日志
///
/// # 返回值
///
/// 日志文件还不存在时返回 `err.fileNotFound`（一次都没写过日志，或目录被清过）
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub fn window_open_launcher_log() -> Result<(), String> {
    let file = launcher_log_file();
    if !file.exists() {
        return Err(err_keys::FILE_NOT_FOUND.to_string());
    }
    mml_sys::open_helper::open_file(&file);
    Ok(())
}

/// 把启动器日志的最后 [`UPLOAD_MAX_LINES`] 行上传到 mclo.gs
///
/// # 返回值
///
/// 成功返回分享网址；没有可上传的内容、网络或对方拒绝都返回错误文案
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub async fn window_upload_launcher_log() -> Result<String, String> {
    let file = launcher_log_file();
    let text = mml_sys::path_helper::read_text(&file).map_err(|_| err_keys::LOG_EMPTY.to_string())?;
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(UPLOAD_MAX_LINES);
    let tail = lines[start..].join("\n");
    if tail.trim().is_empty() {
        return Err(err_keys::LOG_EMPTY.to_string());
    }
    mml_net::mclo_api::push(tail)
        .await
        .map_err(|_| err_keys::LOG_UPLOAD.to_string())
}