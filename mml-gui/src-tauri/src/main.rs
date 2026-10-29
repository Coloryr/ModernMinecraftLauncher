// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use mml_core::CoreInitObj;
use mml_gui_lib::{block_groups, collect_utils, err_box, gui_config, image_manager, windows};
use mml_names::names;
use mml_sys::path_helper;

/// 程序入口：定运行路径、初始化各单例，最后进入 Tauri 事件循环
fn main() {
    let mut obj = CoreInitObj {
        path: Default::default(),
        curseforge_key: "$2a$10$6L8AkVsaGMcZR36i8XvCr.O4INa2zvDwMhooYdLZU0bb/E78AsT0m".to_string(),
        oauth_key: "aa0dd576-d717-4950-b257-a478d2c20968".to_string(),
    };

    let path =
        mml_gui_lib::start_arg::run_dir(std::env::args().skip(1)).unwrap_or_else(get_run_path);

    println!("Run path: {}", path.display());

    // 先试写一次探测目录可写性，不可写就退回系统数据目录
    let temp = path.join("test");
    let res = path_helper::write_text(temp, "test write");
    obj.path = if res.is_err() { get_save_path() } else { path };

    // 提权前固定数据目录，避免 Linux 的 HOME 变化后读到另一套配置。
    let elevated = std::fs::read(obj.path.join(names::GUI_CONFIG_FILE))
        .ok()
        .and_then(|data| serde_json::from_slice::<gui_config::GuiConfig>(&data).ok())
        .is_some_and(|config| config.experimental.launcher_admin);
    if elevated && !mml_sys::process_helper::is_run_as_admin() {
        let mut args = vec![
            "--run-dir".to_string(),
            obj.path.to_string_lossy().into_owned(),
        ];
        args.extend(std::env::args().skip(1));
        if let Err(error) = mml_sys::process_helper::launch_admin(&args) {
            err_box::fatal_error_text(&format!("Administrator startup failed: {error}"));
        }
        return;
    }

    // 可以写入后，查找名为 temp 的端口文件：能独占打开说明没有别的实例；
    // 打不开说明已经有一个在跑 —— 读出端口，通过 mml-ipc 把这次启动交给它：
    // 无参数请求对方显示主窗口，有参数把参数转过去，本进程随即退出。
    // 有实例在跑时关窗后进程留在后台（见 tray），再启动一次 exe 就是「唤起」。
    let port_file = obj.path.join(names::IPC_PORT_FILE);
    // 重启拉起的新进程要真正重启：旧进程已经让出端口文件（见 window_restart_app），
    // 这里直接接着启动，不能把请求又交回那个马上要退的旧进程
    let restart = std::env::args().any(|arg| arg == names::COMMAND_RESTART);
    let mut lock = lock_port_file(&port_file, restart);
    if lock.is_none() && !restart {
        match mml_gui_lib::single_instance::read_port(&port_file) {
            Some(port) => {
                let args: Vec<String> = std::env::args().skip(1).collect();
                if mml_ipc::notify_existing(port, args) {
                    return;
                }
                // 端口过期：上一个实例已经不在了，再锁一次，接着当第一个实例启动
                lock = lock_port_file(&port_file, false);
            }
            // 占着却没有端口：另一个实例正在启动，本进程退出
            None => return,
        }
    }
    let Some(file) = lock else {
        // 占不到又连不上：不跟它抢，直接退出
        return;
    };
    // 占住端口文件：本进程活着就一直持有，端口也写在它上面（见 single_instance）
    mml_gui_lib::single_instance::keep_port_file(file);

    // 窗口状态 / GUI 配置与核心共用同一运行路径
    windows::init(&obj.path);
    gui_config::init(&obj.path);
    collect_utils::init(&obj.path);
    block_groups::init(&obj.path);
    mml_gui_lib::integrations::init(&obj.path);

    // 手柄配置放在运行目录，内核初始化后日志才可用
    let run_path = obj.path.clone();
    if let Err(e) = mml_core::init(obj) {
        err_box::fatal_error(e);
    }

    // 手柄：内核 crate 负责读映射、监听手柄并把输入转发给运行中的游戏实例
    mml_gamepad::init(&run_path);

    // 内核日志语言跟随界面语言：没有 lang.txt 时内核按系统语言走，可能与 GUI 配置不一致
    gui_config::sync_core_lang(gui_config::get().locale);

    image_manager::init();

    mml_gui_lib::run()
}

/// 占住端口文件
///
/// - `path`: 端口文件路径
/// - `wait`: 占不到时要不要等一会儿；重启拉起的新进程用，旧进程要过几百毫秒才让出文件
///
/// # 返回值
///
/// 占住的句柄；占不到返回 `None`
fn lock_port_file(path: &Path, wait: bool) -> Option<File> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(file) = mml_gui_lib::single_instance::lock_port_file(path) {
            return Some(file);
        }
        if !wait || Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
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

        exe_dir.join(names::MML_DIR)
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

/// 发布构建 macOS：取 `run` 文件记录的路径，取不到用家目录下的 `.mml`
///
/// 与 Linux 同一口径：数据落在当前账户的家目录，不写 `/Users/shared`。
///
/// # 返回值
///
/// 返回运行路径
#[cfg(not(debug_assertions))]
#[cfg(target_os = "macos")]
fn get_run_path() -> PathBuf {
    use mml_names::names;

    get_config_path().unwrap_or({
        let dir = dirs::home_dir().unwrap();
        dir.join(names::MML_INNER_DIR)
    })
}
