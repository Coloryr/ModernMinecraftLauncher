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

