//! 配置文件读写
//!
//! 目录、文件名与 json 的读写，含默认配置生成。

use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

use mml_names::{i18_items::info_type::InfoType, names};

use super::*;

/// 写出一份默认配置，字段用大驼峰并带缩进，方便用户直接改
///
/// # 返回值
///
/// 写出的文件路径
pub(crate) fn write_default(dir: &Path) -> Result<PathBuf, String> {
    let file = dir.join(names::GAMEPAD_DEFAULT_MAP_FILE);
    let config = GamepadConfig {
        selected: true,
        ..GamepadConfig::default()
    };
    write_config(&file, &config)?;
    Ok(file)
}

/// 读取一份配置
pub(crate) fn load_file(path: &Path) -> Result<GamepadConfig, String> {
    let text = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
    let mut config: GamepadConfig = serde_json::from_str(&text).map_err(|err| err.to_string())?;
    // 旧配置没有鼠标模式那套按钮：按游戏内那套补齐，行为与升级前一致
    config.normalize();
    Ok(config)
}

/// 写一份配置到文件，字段用大驼峰并带缩进，方便用户直接改
pub(crate) fn write_config(path: &Path, config: &GamepadConfig) -> Result<(), String> {
    let text = serde_json::to_string_pretty(config).map_err(|err| err.to_string())?;
    std::fs::write(path, text).map_err(|err| err.to_string())
}

/// 配置目录，未初始化时报错
pub(crate) fn dir() -> Result<&'static Path, GamepadError> {
    DIR.get().map(PathBuf::as_path).ok_or(GamepadError::Missing)
}

/// 配置文件名去掉扩展名
pub(crate) fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// 配置名对应的文件路径，同时校验名字
pub(crate) fn path_of(name: &str) -> Result<PathBuf, GamepadError> {
    if !valid_name(name) {
        return Err(GamepadError::InvalidName);
    }
    Ok(dir()?.join(format!("{name}.json")))
}

/// 配置名是否合法：非空、不含路径分隔符与 Windows 非法字符
pub(crate) fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= 32
        && !name.starts_with('.')
        && !name.ends_with('.')
        && !name
            .chars()
            .any(|ch| ch.is_control() || "/\\:*?\"<>|".contains(ch))
}

/// 配置文件修改时间
pub(crate) fn file_mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
}

/// 记一条映射不可用的日志
pub(crate) fn log_load_failed(file: &Path, error: &str) {
    mml_log::warn_type(InfoType::GamepadMapLoadFailed {
        path: file.display().to_string(),
        error: error.to_string(),
    });
}
