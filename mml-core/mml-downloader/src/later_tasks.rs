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
mod tests {
    use std::fs;
    use std::io::{Cursor, Write};

    use zip::write::{SimpleFileOptions, ZipWriter};

    use super::*;

    /// 在内存中构造一个包含 native 库结构的 zip 包
    ///
    /// # 返回值
    ///
    /// 返回完整 zip 字节，含包根的原生库、平台子目录里的原生库与 META-INF 元数据
    fn make_test_zip() -> Vec<u8> {
        let buf = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(buf);
        let options = SimpleFileOptions::default();

        // 包根的原生库，应被提取
        writer.start_file("lwjgl.dll", options).unwrap();
        writer.write_all(b"native-data").unwrap();

        // 平台子目录里的原生库，应被提取并展平到目标目录
        writer.add_directory("windows/x64", options).unwrap();
        writer
            .start_file("windows/x64/OpenAL32.dll", options)
            .unwrap();
        writer.write_all(b"so-data").unwrap();

        // META-INF 是元数据，不应被提取
        writer.start_file("META-INF/MANIFEST.MF", options).unwrap();
        writer.write_all(b"manifest-data").unwrap();

        writer.finish().unwrap().into_inner()
    }

    /// 提取包根与平台子目录的原生库并展平，跳过 META-INF
    #[test]
    fn unpack_native_extracts_libs_but_skips_meta_inf() {
        let dir = crate::test_util::make_temp_dir("native");
        let native = dir.join("native");

        unpack_native(&native, Cursor::new(make_test_zip())).unwrap();

        // 包根的原生库被提取
        assert_eq!(fs::read(native.join("lwjgl.dll")).unwrap(), b"native-data");
        // 平台子目录里的原生库被展平提取，只保留文件名
        assert_eq!(fs::read(native.join("OpenAL32.dll")).unwrap(), b"so-data");
        // META-INF 里的元数据不提取
        assert!(!native.join("MANIFEST.MF").exists());
        assert!(!native.join("META-INF").exists());
    }

    /// 非法 zip 数据返回错误
    #[test]
    fn unpack_native_invalid_zip_errors() {
        let dir = crate::test_util::make_temp_dir("native-bad");
        let native = dir.join("native");

        let result = unpack_native(&native, Cursor::new(b"not a zip file".to_vec()));
        assert!(result.is_err(), "非法 zip 应返回错误");
    }

    /// 空 zip 无任何条目时应成功返回
    #[test]
    fn unpack_native_empty_zip_ok() {
        let dir = crate::test_util::make_temp_dir("native-empty");
        let native = dir.join("native");

        let writer = ZipWriter::new(Cursor::new(Vec::new()));

        // finish 消费 writer 并返回底层缓冲区，直接作为输入流
        let data = writer.finish().unwrap();

        unpack_native(&native, data).unwrap();
    }
}
