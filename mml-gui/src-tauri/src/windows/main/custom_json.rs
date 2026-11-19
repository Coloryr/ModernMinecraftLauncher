//! 实例自定义启动配置（`json/` 目录）的读写
//!
//! 主窗口那张表只显示摘要，这里补上编辑要用的原文读写、新建与打开。

use std::path::PathBuf;

use mml_sys::open_helper;

use crate::err_keys;
use crate::windows::main::core_instance;

/// 一份自定义 JSON 的绝对路径
///
/// - `uuid`: 实例 uuid
/// - `file`: `json/` 里的文件名，只接受纯文件名
///
/// # 返回值
///
/// 见 [`PathBuf`]；uuid 非法、实例不存在或名字里带路径时返回错误文案
fn json_path(uuid: &str, file: &str) -> Result<PathBuf, String> {
    let core = core_instance(uuid).ok_or_else(|| err_keys::GAME_NOT_FOUND.to_string())?;
    let inst = core.instance.read().unwrap();
    // 只认纯文件名：名字里带 / 或 .. 的一律拒绝，免得被拿去读写别处的文件
    let name = std::path::Path::new(file)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| *name == file)
        .ok_or_else(|| err_keys::FILE_NAME.to_string())?;
    Ok(inst.get_json_path().join(name))
}

/// 读一份自定义 JSON 的原文
///
/// - `uuid`: 实例 uuid
/// - `file`: `json/` 里的文件名
///
/// # 返回值
///
/// 文件内容；读不到返回 `err.fileNotFound`
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_read_custom_json(uuid: String, file: String) -> Result<String, String> {
    let path = json_path(&uuid, &file)?;
    std::fs::read_to_string(&path).map_err(|_| err_keys::FILE_NOT_FOUND.to_string())
}

/// 写回一份自定义 JSON
///
/// 落盘前先按 JSON 解析一遍：存进坏文件的话，列表会直接少一条，用户看不出是自己写错了。
///
/// - `uuid`: 实例 uuid
/// - `file`: `json/` 里的文件名
/// - `text`: 新的文件内容
///
/// # 返回值
///
/// 写成功返回 `Ok(())`；不是合法 JSON 或写不进去时返回错误文案
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_save_custom_json(uuid: String, file: String, text: String) -> Result<(), String> {
    let path = json_path(&uuid, &file)?;
    serde_json::from_str::<serde_json::Value>(&text).map_err(|err| err.to_string())?;
    std::fs::write(&path, text).map_err(|err| err.to_string())
}

/// 新建一份自定义 JSON
///
/// 名字里的路径分隔符与引号会被去掉；没写扩展名时补 `.json`。
///
/// - `uuid`: 实例 uuid
/// - `name`: 用户填的文件名
///
/// # 返回值
///
/// 实际写下的文件名；重名返回 `err.nameTaken`
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_create_custom_json(uuid: String, name: String) -> Result<String, String> {
    let stem: String = name
        .trim()
        .trim_end_matches(".json")
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect();
    if stem.is_empty() {
        return Err(err_keys::FILE_NAME.to_string());
    }
    let file = format!("{stem}.json");
    let path = json_path(&uuid, &file)?;
    if path.exists() {
        return Err(err_keys::NAME_TAKEN.to_string());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
    }
    // 模板只给能直接跑的最小字段，其余按 base 的默认值走
    let text = format!(
        "{{\n  \"name\": \"{stem}\",\n  \"uid\": \"custom.{stem}\",\n  \"version\": \"1.0\",\n  \"order\": 0,\n  \"mainJar\": {{}}\n}}\n"
    );
    std::fs::write(&path, text).map_err(|err| err.to_string())?;
    Ok(file)
}

/// 打开一份自定义 JSON
///
/// - `uuid`: 实例 uuid
/// - `file`: `json/` 里的文件名
/// - `reveal`: 为真时在文件管理器里定位，否则用系统默认程序打开
///
/// # 返回值
///
/// 打开成功返回 `Ok(())`；文件不存在返回 `err.fileNotFound`
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_open_custom_json(uuid: String, file: String, reveal: bool) -> Result<(), String> {
    let path = json_path(&uuid, &file)?;
    if !path.is_file() {
        return Err(err_keys::FILE_NOT_FOUND.to_string());
    }
    if reveal {
        open_helper::open_file_with_explorer(&path);
    } else {
        open_helper::open_file(&path);
    }
    Ok(())
}
