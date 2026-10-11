// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use mml_core::CoreInitObj;
use mml_gui_lib::{collect_utils, err_box, gui_config, image_manager, windows};
use mml_names::names;
use mml_sys::path_helper;

/// 程序入口：定运行路径、初始化各单例，最后进入 Tauri 事件循环
fn main() {
    let mut obj = CoreInitObj {
        path: Default::default(),
        curseforge_key: "$2a$10$6L8AkVsaGMcZR36i8XvCr.O4INa2zvDwMhooYdLZU0bb/E78AsT0m".to_string(),
        oauth_key: "aa0dd576-d717-4950-b257-a478d2c20968".to_string(),
    };

    let path = get_run_path();

    println!("Run path: {}", path.display());

    // 先试写一次探测目录可写性，不可写就退回系统数据目录
    let temp = path.join("test");
    let res = path_helper::write_text(temp, "test write");
    obj.path = if res.is_err() { get_save_path() } else { path };

    // 窗口状态 / GUI 配置与核心共用同一运行路径
    windows::init(&obj.path);
    gui_config::init(&obj.path);
    collect_utils::init(&obj.path);

    if let Err(e) = mml_core::init(obj) {
        err_box::fatal_error(e);
    }

    image_manager::init();

    mml_gui_lib::run()
}

/// 获取兜底运行路径：系统数据目录下的 M²L 目录
///
/// # 返回值
///
/// 返回运行路径
fn get_save_path() -> PathBuf {
    let dir = dirs::data_dir()
        .or(dirs::data_local_dir())
        .or(dirs::home_dir())
        .unwrap();

    dir.join(names::MML_DIR).to_path_buf()
}

/// 调试构建：取 exe 同目录下的 M²L 目录
///
/// # 返回值
///
/// 返回运行路径
#[cfg(debug_assertions)]
fn get_run_path() -> PathBuf {
    use mml_names::names;
    use std::env;

    let exe_path = env::current_exe().expect("Failed to get exe path");
    let exe_dir = exe_path.parent().expect("Failed to get exe directory");

    exe_dir.join(names::MML_DIR)
}

/// 发布构建：读数据目录下 `run` 文件里记录的运行路径，路径需存在才有效
///
/// # 返回值
///
/// 返回记录的运行路径；文件缺失或路径无效返回 `None`
#[cfg(not(debug_assertions))]
fn get_config_path() -> Option<PathBuf> {
    use std::path::Path;

    let dir = dirs::data_local_dir()?;
    let file = dir.join(names::MML_DIR).join("run");

    if file.exists() && file.is_file() {
        if let Ok(text) = path_helper::read_text(file) {
            let path = Path::new(&text);
            if path.exists() && path.is_dir() {
                return Some(path.to_path_buf());
            }
        }
    }

    None
}

/// 发布构建 Windows：取 `run` 文件记录的路径，取不到用 exe 同目录
///
/// # 返回值
///
/// 返回运行路径
#[cfg(not(debug_assertions))]
#[cfg(target_os = "windows")]
fn get_run_path() -> PathBuf {
    use mml_names::names;
    use std::env;

    get_config_path().unwrap_or({
        let exe_path = env::current_exe().expect("Failed to get exe path");
        let exe_dir = exe_path.parent().expect("Failed to get exe directory");

        exe_dir.join(names::MML)
    })
}

/// 发布构建 Linux：取 `run` 文件记录的路径，取不到用家目录下的安装目录
///
/// # 返回值
///
/// 返回运行路径
#[cfg(not(debug_assertions))]
#[cfg(target_os = "linux")]
fn get_run_path() -> PathBuf {
    use mml_names::names;

    get_config_path().unwrap_or({
        let dir = dirs::home_dir().unwrap();

        dir.join(names::MML_INNER_DIR).to_path_buf()
    })
}

/// 发布构建 macOS：取 `run` 文件记录的路径，取不到用共享目录
///
/// # 返回值
///
/// 返回运行路径
#[cfg(not(debug_assertions))]
#[cfg(target_os = "macos")]
fn get_run_path() -> PathBuf {
    get_config_path().unwrap_or({ Path::new("/Users/shared/mml/").to_path_buf() })
}
