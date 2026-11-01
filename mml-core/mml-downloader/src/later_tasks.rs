//! 下载后处理模块
//!
//! 处理文件下载完成后的后续操作，如解压 native 库等。

use std::{
    io::{self, Read, Seek},
    path::Path,
};

use mml_names::i18_items::error_type::{
    ArchiveErrorData, CoreResult, ErrorData, ErrorType, FileSystemErrorData,
};
use mml_sys::path_helper;
use zip::ZipArchive;

/// 解压 Minecraft native 库
///
/// - `native`: 解压目标目录
/// - `read`: 可读取 + 可定位的输入流，通常是下载的 jar 文件
///
/// 把 jar 里的原生库（.dll / .so / .dylib）解到 `native` 目录，目录层级展平。
/// **跳过 `META-INF/`**：Mojang 的库用 `extract.exclude: ["META-INF/"]` 表达同一件事，
/// 原生库要么在包根、要么在 `windows/x64` 这类平台目录下 —— 只解 META-INF 会一个库都解不出来。
///
/// # 返回值
///
/// 解压成功返回 `Ok(())`；jar 打不开或条目解压失败返回对应错误
pub fn unpack_native<R: Read + Seek>(native: &Path, read: R) -> CoreResult<()> {
    let mut archive = ZipArchive::new(read).map_err(|err| {
        ErrorType::ArchiveOpenError(FileSystemErrorData {
            path: Default::default(),
            error: err.to_string(),
        })
    })?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|err| ErrorType::ArchiveReadError(ErrorData::from_error(err)))?;

        if file.is_dir() {
            continue;
        }

        // 跳过 META-INF：那是元数据，原生库不在里面
        if let Some(name) = file.enclosed_name()
            && !name.starts_with("META-INF")
        {
            let outpath = native.join(name.file_name().unwrap());

            let mut outfile = path_helper::open_write(&outpath)?;
            io::copy(&mut file, &mut outfile).map_err(|err| {
                ErrorType::ArchiveError(ArchiveErrorData {
                    source: file.name().unwrap_or_default().to_string(),
                    target: outpath.display().to_string(),
                    error: err.to_string(),
                })
            })?;
        }
    }

    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/later_tasks_tests.rs"]
mod tests;
