//! 启动器自带的 RenderDoc
//!
//! Windows / Linux 用得到：用 `renderdoccmd capture` 把游戏拉起来并注入捕获库。
//! 整个启动器一份，放 `tools/renderdoc`，换整合包、删实例都不会把它带走。

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Windows 便携包地址，稳定版最新
#[cfg(windows)]
const RENDERDOC_URL: &str = "https://renderdoc.org/stable/latest/RenderDoc_latest_64.zip";
/// Linux 便携包地址，稳定版最新
#[cfg(target_os = "linux")]
const RENDERDOC_URL: &str = "https://renderdoc.org/stable/latest/renderdoc_latest.tar.gz";

/// 自带的 RenderDoc 目录，`tools/renderdoc`
///
/// # 返回值
///
/// 目录路径，不保证存在
pub fn renderdoc_dir() -> PathBuf {
    crate::tools_dir().join("renderdoc")
}

/// RenderDoc 捕获文件目录，`tools/renderdoc-captures`
///
/// 不放在 `tools/renderdoc` 里面：那个目录是安装目录，重新下载 / 解压时会被整体删掉。
///
/// # 返回值
///
/// 目录路径，不保证存在
pub fn captures_dir() -> PathBuf {
    crate::tools_dir().join("renderdoc-captures")
}

/// 找自带的 RenderDoc 命令行工具
///
/// # 返回值
///
/// 找到返回 `renderdoccmd` 路径；还没下载过返回 `None`
pub fn find() -> Option<PathBuf> {
    find_in(&renderdoc_dir())
}

/// 在指定目录里找 `renderdoccmd`
///
/// 便携包会多套一层 `RenderDoc_<版本>_64` 或 `renderdoc_<版本>` 目录，
/// 所以这里按有限深度递归找；Windows 认 `renderdoccmd.exe`，Linux 认 `renderdoccmd`。
///
/// - `dir`: 要搜索的根目录
///
/// # 返回值
///
/// 找到返回路径，找不到返回 `None`
pub fn find_in(dir: &Path) -> Option<PathBuf> {
    fn search(dir: &Path, depth: u32) -> Option<PathBuf> {
        for name in ["renderdoccmd.exe", "renderdoccmd"] {
            let direct = dir.join(name);
            if direct.is_file() {
                return Some(direct);
            }
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

    search(dir, 4)
}

/// 找自带的 RenderDoc 界面程序，即 Qt 版的 `qrenderdoc`
///
/// 便携包里它与 `renderdoccmd` 同目录，所以按命令行工具的位置找。
///
/// # 返回值
///
/// 找到返回界面程序路径；还没下载过或包里没带界面程序时返回 `None`
pub fn find_gui() -> Option<PathBuf> {
    let cmd = find()?;
    let dir = cmd.parent()?;
    for name in ["qrenderdoc.exe", "qrenderdoc"] {
        let path = dir.join(name);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

/// 捕获目录里 `since` 之后生成的最新一份捕获
///
/// - `since`: 这一局启动的时刻，早于它的是之前几局留下的捕获
///
/// # 返回值
///
/// 返回最新 `.rdc` 的路径；没有新捕获时返回 `None`
pub fn latest_capture(since: SystemTime) -> Option<PathBuf> {
    let mut best: Option<(SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(captures_dir()).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rdc") {
            continue;
        }
        let Ok(modified) = entry.metadata().and_then(|meta| meta.modified()) else {
            continue;
        };
        if modified < since {
            continue;
        }
        if best.as_ref().is_none_or(|(time, _)| modified > *time) {
            best = Some((modified, path));
        }
    }
    best.map(|(_, path)| path)
}

/// 生成 `renderdoccmd capture` 的参数
///
/// 命令行形如：
/// `capture -w -d <游戏目录> -c <捕获文件前缀> <java.exe> <原 Java 参数...>`。
/// `-w` 让 renderdoccmd 等游戏退出，启动器才能继续跟踪进程句柄。
///
/// - `java`: 原 Java 可执行文件
/// - `args`: 原 Java 启动参数
/// - `game_dir`: 游戏根目录，作为子进程工作目录
/// - `captures`: 捕获文件目录，必须已存在
/// - `stem`: 捕获文件名前缀，不含扩展名
///
/// # 返回值
///
/// `renderdoccmd` 的完整参数列表
pub fn capture_args(
    java: &Path,
    args: &[String],
    game_dir: &Path,
    captures: &Path,
    stem: &str,
) -> Vec<String> {
    let mut out = vec![
        "capture".to_string(),
        "-w".to_string(),
        "-d".to_string(),
        game_dir.to_string_lossy().into_owned(),
        "-c".to_string(),
        captures
            .join(format!("{stem}_"))
            .to_string_lossy()
            .into_owned(),
        java.to_string_lossy().into_owned(),
    ];
    out.extend(args.iter().cloned());
    out
}

/// 下载并解出自带的 RenderDoc
///
/// 便携包解到 `tools/renderdoc`，里面自带 `renderdoccmd` 与捕获库。
///
/// # 返回值
///
/// 返回 `renderdoccmd` 路径；下载、解压或找不到命令行工具时返回对应错误
#[cfg(any(windows, target_os = "linux"))]
pub async fn ensure() -> mml_names::i18_items::error_type::CoreResult<PathBuf> {
    use mml_base::{
        archives::BaseArchive,
        file_item::{FileHash, FileItemObj, LaterRun},
    };
    use mml_names::{
        i18,
        i18_items::{
            error_type::{ErrorType, PathNotExistsData},
            info_type::InfoType,
        },
    };
    use mml_sys::path_helper;

    if let Some(path) = find() {
        return Ok(path);
    }

    let mut file = mml_downloader::gen_temp_file();
    // 后缀要留着：解包按后缀判断压缩格式
    #[cfg(windows)]
    file.set_extension("zip");
    #[cfg(target_os = "linux")]
    file.set_extension("tar.gz");

    let done = mml_downloader::start_download_task(vec![FileItemObj {
        name: i18::get_info(InfoType::RenderDoc),
        file: file.clone(),
        url: RENDERDOC_URL.to_string(),
        hash: FileHash::None,
        later: LaterRun::None,
    }])
    .await;

    if !done {
        let _ = std::fs::remove_file(&file);
        return Err(ErrorType::DownloadFileFail);
    }

    let dir = renderdoc_dir();
    // 上一次下载 / 解压留下的半套目录先清掉，避免旧文件混进来
    let _ = std::fs::remove_dir_all(&dir);
    path_helper::create_dir_all(&dir)?;

    let result = (|| -> mml_names::i18_items::error_type::CoreResult<PathBuf> {
        BaseArchive::open_readonly(&file)?.extract_all(&dir, None, None, None)?;
        find_in(&dir)
            .ok_or_else(|| ErrorType::PathNotExists(PathNotExistsData { path: dir.clone() }))
    })();
    // 解压失败也把下载的临时包删掉，别让它留在 cache 里
    let _ = std::fs::remove_file(&file);
    result
}

#[cfg(test)]
#[path = "../tests/unit/renderdoc_tests.rs"]
mod tests;
