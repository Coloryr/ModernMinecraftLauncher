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
/// 从下载的 JAR 包中提取 `META-INF` 目录下的原生库文件。
/// 这些文件是 LWJGL 等底层库在各平台上的本地实现，如 .dll / .so / .dylib。
pub fn unpack_native<R: Read + Seek>(native: &Path, read: R) -> CoreResult<()> {
    let mut archive = ZipArchive::new(read).map_err(|err| {
        ErrorType::ArchiveOpenError(FileSystemErrorData {
            path: Default::default(),
            error: err.to_string(),
        })
    })?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|err| {
            ErrorType::ArchiveReadError(ErrorData {
                error: err.to_string(),
            })
        })?;

        if file.is_dir() {
            continue;
        }

        // 仅提取 META-INF 目录下的文件，那是 native 库的存放位置
        if let Some(name) = file.enclosed_name()
            && name.starts_with("META-INF")
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
    /// 返回完整 zip 字节，含 META-INF 下的 native 库、子目录库与普通 class 文件
    fn make_test_zip() -> Vec<u8> {
        let buf = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(buf);
        let options = SimpleFileOptions::default();

        // META-INF 下的 native 库，应被提取
        writer.start_file("META-INF/lwjgl.dll", options).unwrap();
        writer.write_all(b"native-data").unwrap();

        // META-INF 子目录中的 native 库，应被提取并展平到目标目录
        writer.add_directory("META-INF/sub", options).unwrap();
        writer
            .start_file("META-INF/sub/liblwjgl.so", options)
            .unwrap();
        writer.write_all(b"so-data").unwrap();

        // 普通 class 文件，不应被提取
        writer
            .start_file("net/minecraft/Main.class", options)
            .unwrap();
        writer.write_all(b"class-data").unwrap();

        writer.finish().unwrap().into_inner()
    }

    /// 只提取 META-INF 下的文件，并展平目录结构
    #[test]
    fn unpack_native_extracts_meta_inf_only() {
        let dir = crate::test_util::make_temp_dir("native");
        let native = dir.join("native");

        unpack_native(&native, Cursor::new(make_test_zip())).unwrap();

        // META-INF 根下的文件被提取
        assert_eq!(fs::read(native.join("lwjgl.dll")).unwrap(), b"native-data");
        // 子目录文件被展平提取，只保留文件名
        assert_eq!(fs::read(native.join("liblwjgl.so")).unwrap(), b"so-data");
        // 非 META-INF 文件不提取
        assert!(!native.join("Main.class").exists());
        assert!(!native.join("net").exists());
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
