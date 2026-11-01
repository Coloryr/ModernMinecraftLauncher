use std::{
    fs,
    io::{Cursor, Seek, SeekFrom},
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use super::*;
use crate::nbt_types;

/// 构造一个简单的根 Compound：{ "byte": 1b, "int": 2 }
fn sample_compound() -> NbtType {
    let mut com = NbtCompound::new();
    com.data.insert("byte".into(), nbt_types::byte(1).to_nbt());
    com.data.insert("int".into(), nbt_types::int(2).to_nbt());
    com.to_nbt()
}

/// 压缩类型在内存流中写入后读取，内容与压缩类型均应保持一致
#[test]
fn round_trip_all_compressions() {
    for compress in [
        CompressType::None,
        CompressType::GZip,
        CompressType::Zlib,
        CompressType::Lz4,
    ] {
        let file = NbtFile::new(sample_compound(), compress.clone());

        let mut stream = Cursor::new(Vec::<u8>::new());
        file.write(&mut stream).unwrap();

        stream.seek(SeekFrom::Start(0)).unwrap();
        let back = NbtFile::read(&mut stream).unwrap();

        assert_eq!(back.compress, compress);
        assert!(file.nbt.eq(&back.nbt));
    }
}

/// 单字节 0x00 文件应识别为 End 标签
#[test]
fn read_end_only_file() {
    let mut stream = Cursor::new(vec![0u8]);
    let file = NbtFile::read(&mut stream).unwrap();
    assert!(matches!(file.nbt, NbtType::End(_)));
    assert_eq!(file.compress, CompressType::None);
}

/// 两字节 `[TAG_Byte, 值]` 文件应识别为单个 Byte 标签
#[test]
fn read_single_byte_file() {
    let mut stream = Cursor::new(vec![NBT_BYTE_ORDER, 42]);
    let file = NbtFile::read(&mut stream).unwrap();
    assert_eq!(file.nbt.as_byte().unwrap().data, 42);
    assert_eq!(file.compress, CompressType::None);
}

/// 过短的非法文件应返回错误
#[test]
fn read_too_short_returns_error() {
    // 2 字节但首字节为 TAG_End，不符合任何已知格式
    let mut stream = Cursor::new(vec![0u8, 5]);
    assert!(NbtFile::read(&mut stream).is_err());
}

/// 压缩格式的 Display 名称
#[test]
fn compress_display() {
    assert_eq!(CompressType::None.to_string(), "none");
    assert_eq!(CompressType::GZip.to_string(), "gzip");
    assert_eq!(CompressType::Zlib.to_string(), "zlib");
    assert_eq!(CompressType::Lz4.to_string(), "lz4");
}

/// NbtFile 的 Display 包含压缩类型信息
#[test]
fn file_display() {
    let file = NbtFile::new(NbtType::end(), CompressType::GZip);
    let text = file.to_string();
    assert!(text.contains("END"));
    assert!(text.contains("gzip"));
}

/// NbtFile 的默认值为 End 标签 + 无压缩
#[test]
fn default_values() {
    let file = NbtFile::default();
    assert!(matches!(file.nbt, NbtType::End(_)));
    assert_eq!(file.compress, CompressType::None);
}

/// 在临时目录下以真实文件做读写往返，验证跨平台的文件 IO 行为
#[test]
fn file_round_trip_on_disk() {
    // 唯一临时子目录，避免并发冲突
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir =
        mml_testutil::temp_dir().join(format!("mml-nbt-test-{}-{}", std::process::id(), nanos));
    fs::create_dir_all(&dir).unwrap();

    let path: PathBuf = dir.join("test.dat");
    let result = (|| -> CoreResult<()> {
        let file = NbtFile::new(sample_compound(), CompressType::GZip);
        {
            let mut out = fs::File::create(&path).map_err(|err| io_error(err))?;
            file.write(&mut out)?;
        }

        let mut input = fs::File::open(&path).map_err(|err| io_error(err))?;
        let back = NbtFile::read(&mut input)?;
        assert_eq!(back.compress, CompressType::GZip);
        assert!(file.nbt.eq(&back.nbt));
        Ok(())
    })();

    // 测试结束清理临时目录
    let _ = fs::remove_dir_all(&dir);
    result.unwrap();
}

