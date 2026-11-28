//! 自定义主页面的持久存储
//!
//! 页面把要记住的内容原样存进运行目录的 `.custom_config`，启动器不解析、不改写；
//! 文件不存在时读取返回空串，页面自己决定空串代表"还没存过"。
//!
//! 命令分成 `read_custom_config` / `write_custom_config` 两条；`read_cutsom_config`
//! 是最初接口名里的拼写，保留成别名，避免已经按那个名字写的页面失效。

use std::path::PathBuf;

use mml_sys::path_helper;

/// 自定义存储的文件名，放运行根目录；用点开头避免和普通配置混在一起
const CONFIG_FILE: &str = ".custom_config";

/// 自定义存储的绝对路径
///
/// # 返回值
///
/// 运行根目录下的 `.custom_config`
fn config_path() -> PathBuf {
    mml_base::get_base_dir().join(CONFIG_FILE)
}

/// 读自定义存储
///
/// # 返回值
///
/// 文件全文；文件不存在时返回空串，读盘失败返回错误文案
async fn read_config() -> Result<String, String> {
    let path = config_path();
    tauri::async_runtime::spawn_blocking(move || {
        if !path.is_file() {
            return Ok(String::new());
        }
        path_helper::read_text(&path).map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

/// 写自定义存储
///
/// - `data`: 页面给的字符串，原样覆盖文件
///
/// # 返回值
///
/// 写盘成功返回 `Ok(())`，失败返回错误文案
async fn write_config(data: String) -> Result<(), String> {
    let path = config_path();
    tauri::async_runtime::spawn_blocking(move || {
        path_helper::write_text(&path, &data).map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

/// 读取自定义存储
///
/// # 返回值
///
/// `.custom_config` 的全文；文件不存在时是空串
#[gui_macros::command]
pub async fn read_custom_config() -> Result<String, String> {
    read_config().await
}

/// 读取自定义存储（兼容最初接口名里的拼写）
///
/// # 返回值
///
/// 与 [`read_custom_config`] 相同
#[gui_macros::command]
pub async fn read_cutsom_config() -> Result<String, String> {
    read_config().await
}

/// 写入自定义存储
///
/// - `data`: 要保存的字符串，按原样覆盖，启动器不校验内容格式
///
/// # 返回值
///
/// 写盘成功返回 `Ok(())`，失败返回错误文案
#[gui_macros::command]
pub async fn write_custom_config(data: String) -> Result<(), String> {
    write_config(data).await
}
