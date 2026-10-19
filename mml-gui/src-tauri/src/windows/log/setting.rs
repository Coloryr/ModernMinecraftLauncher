//! 日志设置与上传
//!
//! 日志等级、保留条数与上传走的都是配置里那一份，
//! 设置改完立即生效，不需要重启。

use std::path::PathBuf;

use mml_game::game_log::{InstanceRuntimeLog};
use uuid::Uuid;

use crate::dtos::main_dto::LogLine;
use crate::dtos::LogSettingDto;
use crate::err_keys;
use crate::gui_setting;

use crate::windows::instance_handle;
use super::read::{join_log_lines, log_line_from_item};


/// 获取实例的实时运行日志快照
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 实时日志行；实例不存在或没在跑返回空列表
#[gui_macros::ipc_group("log")]
#[gui_macros::command]
pub fn log_get_runtime(uuid: String) -> Vec<LogLine> {
    let Ok(id) = Uuid::parse_str(&uuid) else {
        return Vec::new();
    };
    let Some(instance) = mml_game::get_instance(&id) else {
        return Vec::new();
    };
    let game = instance.read().unwrap();
    let Some(runtime) = game.get_runtime_log() else {
        return Vec::new();
    };
    runtime.iter().map(log_line_from_item).collect()
}

/// 读取实例的日志窗口设置，即 `gui_setting.json` 的 `Log`
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 见 [`LogSettingDto`]；文件缺失或解析失败时给默认值，实例不存在返回错误文案
#[gui_macros::ipc_group("log")]
#[gui_macros::command]
pub fn log_get_setting(uuid: String) -> Result<LogSettingDto, String> {
    let instance = instance_handle(&uuid)?;
    let game = instance.read().unwrap();
    Ok(gui_setting::load(&game).log.into())
}

/// 保存实例的日志窗口设置，写入 `gui_setting.json` 的 `Log`
///
/// 界面上每改一项就整份回写：设置项少，读-改-写的代价可以忽略，省得前端攒状态。
///
/// - `uuid`: 实例 uuid
/// - `setting`: 新的日志设置
///
/// # 返回值
///
/// 已提交异步落盘返回 `Ok(())`；实例不存在返回错误文案
#[gui_macros::ipc_group("log")]
#[gui_macros::command]
pub fn log_set_setting(uuid: String, setting: LogSettingDto) -> Result<(), String> {
    let instance = instance_handle(&uuid)?;
    let game = instance.read().unwrap();
    let mut obj = gui_setting::load(&game);
    obj.log = setting.into();
    gui_setting::save(&game, &obj);
    Ok(())
}

/// 把日志上传到 mclo.gs，返回可分享的网址
///
/// 上传的是**整份日志**而不是界面上筛过的那几行：日志上传就是给他人排查用的，
/// 少一行都可能正好少掉关键那行。`file` 为空表示当前运行日志。
///
/// - `uuid`: 实例 uuid
/// - `file`: 日志文件绝对路径，取自 `log_get_files`；`None` 表示实时运行日志
///
/// # 返回值
///
/// 成功返回分享网址；没有可上传的内容、路径非法、网络或对方拒绝都返回错误文案
#[gui_macros::ipc_group("log")]
#[gui_macros::command]
pub async fn log_upload(uuid: String, file: Option<String>) -> Result<String, String> {
    let instance = instance_handle(&uuid)?;
    // 读盘 / 取运行日志都在锁里做完，别把锁带进 await。
    // 取的是**展示文本**（`LogLine::text`）：游戏日志的时间戳本来就在行内，
    // 上传的就是界面上看到的那一份
    let text = {
        let game = instance.read().unwrap();
        match file.as_deref().filter(|f| !f.is_empty()) {
            Some(path) => {
                let path = PathBuf::from(path);
                if !game.get_log_files().contains(&path) {
                    return Err(err_keys::FILE_NOT_FOUND.to_string());
                }
                let runtime = InstanceRuntimeLog::from_file(&path, game.encoding);
                let logs = runtime.logs.read().unwrap();
                join_log_lines(logs.iter())
            }
            None => match game.get_runtime_log() {
                Some(runtime) => join_log_lines(runtime.iter()),
                None => String::new(),
            },
        }
    };

    if text.trim().is_empty() {
        return Err(err_keys::LOG_EMPTY.to_string());
    }

    mml_net::mclo_api::push(text)
        .await
        .map_err(|_| err_keys::LOG_UPLOAD.to_string())
}
