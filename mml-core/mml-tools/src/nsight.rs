//! 本机安装的 NVIDIA Nsight Graphics
//!
//! 它不像 RenderDoc 那样能由启动器下载：安装包数 GB、要 NVIDIA 账号，
//! 所以这里只做检测，找到就用它的命令行工具 `ngfx-capture` 把游戏包起来。
//! 没有装、或不是 NVIDIA 显卡时，界面上的这个入口不该出现。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 本机找到的 Nsight Graphics
pub struct NsightInstall {
    /// 命令行抓帧工具 `ngfx-capture.exe`，启动游戏时用它包住 Java
    pub cli: PathBuf,
    /// 带界面的主程序，包里没有时为空
    pub gui: Option<PathBuf>,
}

/// Nsight Graphics 的捕获文件目录，`tools/nsight-captures`
///
/// 与 RenderDoc 的捕获目录并列，同样不在工具安装目录里，重装工具不会把它删掉。
///
/// # 返回值
///
/// 目录路径，不保证存在
pub fn captures_dir() -> PathBuf {
    crate::tools_dir().join("nsight-captures")
}

/// 找本机安装的 Nsight Graphics
///
/// 只认 NVIDIA 的默认安装位置；装在别处的用户拿不到这个入口，改用「生成启动指令」手动填。
///
/// # 返回值
///
/// 找到返回安装信息；没装或当前平台不支持时返回 `None`
pub fn find() -> Option<NsightInstall> {
    candidate_roots()
        .iter()
        .find_map(|root| find_in(root))
}

/// 在指定目录下找 `Nsight Graphics <版本>` 安装
///
/// - `root`: 厂商目录，如 `C:\Program Files\NVIDIA Corporation`
///
/// # 返回值
///
/// 找到返回安装信息，找不到返回 `None`
pub fn find_in(root: &Path) -> Option<NsightInstall> {
    let dirs = std::fs::read_dir(root).ok()?.flatten().filter_map(|entry| {
        let path = entry.path();
        let name = path.file_name()?.to_str()?.to_lowercase();
        // 版本号在目录名里，升级就会换目录，所以只按前缀认
        (path.is_dir() && name.starts_with("nsight graphics")).then_some(path)
    });
    for dir in dirs {
        if let Some(found) = search(&dir, 3) {
            return Some(found);
        }
    }
    None
}

/// 在安装目录里按有限深度找命令行工具
///
/// - `dir`: 要搜索的目录
/// - `depth`: 还能往下几层
///
/// # 返回值
///
/// 找到返回安装信息，找不到返回 `None`
fn search(dir: &Path, depth: u32) -> Option<NsightInstall> {
    let cli_name = if cfg!(windows) {
        "ngfx-capture.exe"
    } else {
        "ngfx-capture"
    };
    let gui_name = if cfg!(windows) {
        "Nsight Graphics.exe"
    } else {
        "nsight-graphics"
    };

    let cli = dir.join(cli_name);
    if cli.is_file() {
        let gui = dir.join(gui_name);
        return Some(NsightInstall {
            cli,
            gui: gui.is_file().then_some(gui),
        });
    }

    if depth == 0 {
        return None;
    }
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir()
            && let Some(found) = search(&path, depth - 1)
        {
            return Some(found);
        }
    }
    None
}

/// 默认安装位置的上层目录
#[cfg(target_os = "windows")]
fn candidate_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for key in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(dir) = std::env::var_os(key) {
            roots.push(PathBuf::from(dir).join("NVIDIA Corporation"));
        }
    }
    roots
}

/// 默认安装位置的上层目录
#[cfg(target_os = "linux")]
fn candidate_roots() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/opt/nvidia"),
        PathBuf::from("/opt"),
        PathBuf::from("/usr/local"),
    ]
}

/// 默认安装位置的上层目录
#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn candidate_roots() -> Vec<PathBuf> {
    Vec::new()
}

/// 生成 `ngfx-capture` 的启动参数
///
/// 命令行形如：
/// `--exe <java.exe> --working-dir <游戏目录> --output-dir <捕获目录> --env KEY=VALUE… --args <原 Java 参数…>`。
/// `--env` 与 `--args` 都是可变长，`--args` 必须放最后，否则会把后面的选项一起当成游戏参数。
///
/// - `java`: 原 Java 可执行文件
/// - `args`: 原 Java 启动参数
/// - `game_dir`: 游戏根目录，作为子进程工作目录
/// - `captures`: 捕获文件目录，必须已存在
/// - `env`: 要注入的环境变量
///
/// # 返回值
///
/// `ngfx-capture` 的完整参数列表
pub fn capture_args(
    java: &Path,
    args: &[String],
    game_dir: &Path,
    captures: &Path,
    env: &HashMap<String, String>,
) -> Vec<String> {
    let mut out = vec![
        "--exe".to_string(),
        java.to_string_lossy().into_owned(),
        "--working-dir".to_string(),
        game_dir.to_string_lossy().into_owned(),
        "--output-dir".to_string(),
        captures.to_string_lossy().into_owned(),
    ];
    if !env.is_empty() {
        out.push("--env".to_string());
        let mut keys: Vec<&String> = env.keys().collect();
        keys.sort();
        for key in keys {
            out.push(format!("{key}={}", env[key]));
        }
    }
    out.push("--args".to_string());
    out.extend(args.iter().cloned());
    out
}

#[cfg(test)]
#[path = "../tests/unit/nsight_tests.rs"]
mod tests;
