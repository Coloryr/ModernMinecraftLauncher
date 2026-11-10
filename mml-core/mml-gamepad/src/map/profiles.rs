//! 映射管理接口
//!
//! 列表、读取、保存、新建、删除与切换生效项。

use super::*;

/// 列出配置目录里的全部映射
///
/// # 返回值
///
/// 按名字排序；读取或校验失败的条目也列出来，原因放在 error 里
pub fn profiles() -> Vec<ProfileInfo> {
    let Some(dir) = DIR.get() else {
        return Vec::new();
    };
    let scan = scan(dir);
    let active = pick(&scan.profiles).map(|item| item.path.clone());

    let mut items: Vec<ProfileInfo> = scan
        .profiles
        .iter()
        .map(|item| ProfileInfo {
            name: file_stem(&item.path),
            active: active.as_deref() == Some(item.path.as_path()),
            error: None,
        })
        .collect();
    for item in &scan.errors {
        items.push(ProfileInfo {
            name: file_stem(&item.path),
            active: false,
            error: Some(item.error.clone()),
        });
    }
    items.sort_by(|a, b| a.name.cmp(&b.name));
    items
}

/// 读取一份映射
///
/// - `name`: 文件名去掉 .json
///
/// # 返回值
///
/// 配置内容；文件不存在时返回 Missing，内容不合法时返回 InvalidConfig
pub fn config(name: &str) -> Result<GamepadConfig, GamepadError> {
    let path = path_of(name)?;
    if !path.is_file() {
        return Err(GamepadError::Missing);
    }
    match load_file(&path) {
        Ok(config) => Ok(config),
        Err(error) => {
            log_load_failed(&path, &error);
            Err(GamepadError::InvalidConfig)
        }
    }
}

/// 保存一份映射，内容不合法时不落盘
///
/// - `name`: 文件名去掉 .json
/// - `config`: 界面编辑后的整份映射
///
/// # 返回值
///
/// 保存并重新加载成功返回 Ok；文件不存在返回 Missing，内容不合法返回 InvalidConfig
pub fn save(name: &str, config: &GamepadConfig) -> Result<(), GamepadError> {
    let dir = dir()?;
    let path = path_of(name)?;
    if !path.is_file() {
        return Err(GamepadError::Missing);
    }
    if let Err(error) = config.compile() {
        log_load_failed(&path, &error);
        return Err(GamepadError::InvalidConfig);
    }
    write_config(&path, config).map_err(GamepadError::Io)?;
    refresh(dir);
    Ok(())
}

/// 新建一份映射，复制当前生效的那份
///
/// - `name`: 新配置的显示名，同时作为文件名
///
/// # 返回值
///
/// 建好并重新加载返回 Ok；重名返回 Exists，名字不合法返回 InvalidName
pub fn create(name: &str) -> Result<(), GamepadError> {
    let dir = dir()?;
    let path = path_of(name)?;
    if path.exists() {
        return Err(GamepadError::Exists);
    }

    let mut config = {
        let scan = scan(dir);
        match pick(&scan.profiles) {
            Some(item) => load_file(&item.path).map_err(GamepadError::Io)?,
            None => GamepadConfig::default(),
        }
    };
    config.selected = false;
    write_config(&path, &config).map_err(GamepadError::Io)?;
    refresh(dir);
    Ok(())
}

/// 删除一份映射，至少保留一份
///
/// - `name`: 要删除的配置
///
/// # 返回值
///
/// 删除并重新加载返回 Ok；只剩一份返回 Last，文件不存在返回 Missing
pub fn delete(name: &str) -> Result<(), GamepadError> {
    let dir = dir()?;
    let path = path_of(name)?;
    if !path.is_file() {
        return Err(GamepadError::Missing);
    }
    if config_files(dir).len() <= 1 {
        return Err(GamepadError::Last);
    }
    std::fs::remove_file(&path).map_err(|err| GamepadError::Io(err.to_string()))?;
    refresh(dir);
    Ok(())
}

/// 把一份映射设为当前生效，其余配置取消选中
///
/// - `name`: 要生效的配置
///
/// # 返回值
///
/// 改完并重新加载返回 Ok；文件不存在返回 Missing
pub fn set_active(name: &str) -> Result<(), GamepadError> {
    let dir = dir()?;
    let path = path_of(name)?;
    if !path.is_file() {
        return Err(GamepadError::Missing);
    }

    for item in config_files(dir) {
        let Ok(mut config) = load_file(&item) else {
            continue;
        };
        let want = item == path;
        if config.selected != want {
            config.selected = want;
            write_config(&item, &config).map_err(GamepadError::Io)?;
        }
    }
    refresh(dir);
    Ok(())
}

/// 配置界面可选的键鼠动作名
///
/// # 返回值
///
/// 按键盘 / 功能键 / 鼠标 / 滚轮排好的动作名，都能被 [`parse_action`] 解析
pub fn action_names() -> Vec<String> {
    let mut names = Vec::new();
    for ch in 'A'..='Z' {
        names.push(ch.to_string());
    }
    for ch in '0'..='9' {
        names.push(ch.to_string());
    }
    for name in [
        "Space",
        "Enter",
        "Tab",
        "Backspace",
        "Escape",
        "Insert",
        "Delete",
        "Home",
        "End",
        "PageUp",
        "PageDown",
        "Up",
        "Down",
        "Left",
        "Right",
        "LeftShift",
        "LeftControl",
        "LeftAlt",
        // 标点与这些功能键也要能绑：键盘图上它们都是实键，不是占位
        "Grave",
        "Minus",
        "Equal",
        "LeftBracket",
        "RightBracket",
        "Backslash",
        "Semicolon",
        "Apostrophe",
        "Comma",
        "Period",
        "Slash",
        "CapsLock",
        "LeftSuper",
        "Menu",
    ] {
        names.push(name.to_string());
    }
    for number in 1..=12 {
        names.push(format!("F{number}"));
    }
    for name in ["MouseLeft", "MouseRight", "MouseMiddle", "Mouse4", "Mouse5"] {
        names.push(name.to_string());
    }
    for name in ["ScrollUp", "ScrollDown"] {
        names.push(name.to_string());
    }
    names
}
