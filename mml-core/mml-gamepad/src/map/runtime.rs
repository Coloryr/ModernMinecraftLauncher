//! 运行时状态
//!
//! 当前生效的映射、配置目录扫描与热重载。

use std::path::{Path, PathBuf};

use mml_names::{i18_items::info_type::InfoType, names};

use super::*;

/// 加载配置目录，首次运行时建目录并写一份默认配置
///
/// - `base`: 运行目录，配置目录是其中的 gamepad
pub fn init(base: &Path) {
    let dir = base.join(names::GAMEPAD_DIR);
    let _ = DIR.set(dir.clone());

    if let Err(error) = std::fs::create_dir_all(&dir) {
        log_load_failed(&dir, &error.to_string());
        return;
    }

    if signature(&dir).is_empty() {
        match write_default(&dir) {
            Ok(file) => mml_log::info_type(InfoType::GamepadMapDefaultWritten {
                path: file.display().to_string(),
            }),
            Err(error) => log_load_failed(&dir, &error),
        }
    }

    refresh(&dir);
}

/// 配置目录有变化时重新加载
pub fn reload_if_changed() {
    let Some(dir) = DIR.get() else {
        return;
    };
    if *SIGNATURE.read().unwrap() == signature(dir) {
        return;
    }
    refresh(dir);
}

/// 当前生效的映射
pub fn current() -> CompiledMap {
    MAP.read().unwrap().clone()
}

/// 轴值是否落在区间内，起点大于终点时自动交换
pub fn in_range(value: f32, from: f32, to: f32) -> bool {
    let (low, high) = if from <= to { (from, to) } else { (to, from) };
    value >= low && value <= high
}

/// 重新扫描配置目录并应用生效的那份
pub(super) fn reload(dir: &Path) {
    let scan = scan(dir);
    for item in &scan.errors {
        log_load_failed(&item.path, &item.error);
    }

    let Some(profile) = pick(&scan.profiles) else {
        *MAP.write().unwrap() = CompiledMap::built_in();
        mml_log::warn_type(InfoType::GamepadMapUnavailable {
            path: dir.display().to_string(),
        });
        return;
    };

    *MAP.write().unwrap() = profile.map.clone();
    mml_log::info_type(InfoType::GamepadMapLoaded {
        path: profile.path.display().to_string(),
    });
}

/// 选出生效的配置
///
/// 优先标记 Selected 的，否则取文件名排序第一份
pub(super) fn pick(profiles: &[Profile]) -> Option<&Profile> {
    profiles
        .iter()
        .find(|item| item.selected)
        .or_else(|| profiles.first())
}

/// 扫描配置目录
pub(super) fn scan(dir: &Path) -> Scan {
    let mut scan = Scan {
        profiles: Vec::new(),
        errors: Vec::new(),
    };

    for path in config_files(dir) {
        match load_file(&path) {
            Ok(config) => match config.compile() {
                Ok(map) => scan.profiles.push(Profile {
                    path,
                    selected: config.selected,
                    map,
                }),
                Err(error) => scan.errors.push(ProfileError { path, error }),
            },
            Err(error) => scan.errors.push(ProfileError { path, error }),
        }
    }

    scan
}

/// 配置目录里的全部 json 文件，按路径排序
pub(super) fn config_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        // 兼容旧版遗留的读取选项，避免 serde(default) 将其解析成空白映射。
        .filter(|path| path.file_name().and_then(|name| name.to_str()) != Some("reader.json"))
        .collect();
    files.sort();
    files
}

/// 配置目录的签名：文件名与修改时间
pub(super) fn signature(dir: &Path) -> String {
    config_files(dir)
        .iter()
        .map(|path| format!("{}:{:?}", file_stem(path), file_mtime(path)))
        .collect::<Vec<_>>()
        .join("|")
}

/// 重新加载并记下当前目录签名
pub(super) fn refresh(dir: &Path) {
    reload(dir);
    *SIGNATURE.write().unwrap() = signature(dir);
}
