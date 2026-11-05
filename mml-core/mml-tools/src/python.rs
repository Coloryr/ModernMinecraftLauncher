//! 启动器自带的 Python 运行环境
//!
//! 系统里没有 Python 时下到 `tools/python`：整个启动器只有这一份，
//! 换整合包、删实例都不会把它带走。

use std::path::{Path, PathBuf};

#[cfg(windows)]
use mml_base::{
    archives::BaseArchive,
    file_item::{FileHash, FileItemObj, LaterRun},
};
#[cfg(windows)]
use mml_names::{
    i18,
    i18_items::{
        error_type::{CoreResult, ErrorType, PathNotExistsData},
        info_type::InfoType,
    },
};
#[cfg(windows)]
use mml_sys::path_helper;

/// 下载的 Python 版本
const PYTHON_VERSION: &str = "3.13.5";

/// 自带的 Python 目录，`tools/python`
///
/// # 返回值
///
/// 目录路径，不保证存在
pub fn python_dir() -> PathBuf {
    crate::tools_dir().join("python")
}

/// 找自带的 Python 解释器
///
/// # 返回值
///
/// 找到返回解释器路径；还没下载过返回 `None`
pub fn find() -> Option<PathBuf> {
    find_in(&python_dir())
}

/// 在指定目录里找解释器
///
/// - `dir`: 解压出来的 Python 目录
///
/// # 返回值
///
/// 找到返回解释器路径，找不到返回 `None`
pub fn find_in(dir: &Path) -> Option<PathBuf> {
    for rel in ["python.exe", "bin/python3", "bin/python", "python3"] {
        let cand = dir.join(rel);
        if cand.is_file() {
            return Some(cand);
        }
    }

    None
}

/// 下载并解出自带的 Python
///
/// 只支持 Windows：用 python.org 的嵌入式包，解出来就能跑脚本。
///
/// # 返回值
///
/// 返回解释器路径；下载或解压失败时返回对应错误
#[cfg(windows)]
pub async fn download() -> CoreResult<PathBuf> {
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        _ => return Err(ErrorType::InvalidOperation),
    };
    let url = format!(
        "https://www.python.org/ftp/python/{v}/python-{v}-embed-{arch}.zip",
        v = PYTHON_VERSION
    );

    // 后缀要留着：解包按后缀判断压缩格式
    let mut file = mml_downloader::gen_temp_file();
    file.set_extension("zip");

    let done = mml_downloader::start_download_task(vec![FileItemObj {
        name: i18::get_info(InfoType::PythonRuntime),
        file: file.clone(),
        url,
        hash: FileHash::None,
        later: LaterRun::None,
    }])
    .await;

    let result = if done {
        extract(&file)
    } else {
        Err(ErrorType::DownloadFileFail)
    };

    let _ = std::fs::remove_file(&file);
    result
}

/// 把下载到的压缩包解到 `tools/python`
///
/// - `file`: 下载好的压缩包
///
/// # 返回值
///
/// 返回解出来的解释器路径
#[cfg(windows)]
fn extract(file: &Path) -> CoreResult<PathBuf> {
    let dir = python_dir();
    path_helper::create_dir_all(&dir)?;

    BaseArchive::open_readonly(file)?.extract_all(&dir, None, None, None)?;

    find_in(&dir).ok_or_else(|| {
        ErrorType::PathNotExists(PathNotExistsData {
            path: dir.clone(),
        })
    })
}

#[cfg(test)]
#[path = "../tests/unit/python_tests.rs"]
mod tests;
