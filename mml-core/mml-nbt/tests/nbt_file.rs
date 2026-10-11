//! 四种压缩格式的 NBT 文件往返测试，外加一个真实 `level.dat` 样本。

use std::{
    fs::File,
    io::{Cursor, Seek, SeekFrom},
    path::Path,
};

use mml_nbt::{
    NBT_COMPOUND_ORDER, NbtType,
    nbt_file::{CompressType, NbtFile},
    nbt_types,
};

/// GZip 压缩的 Compound 文件往返后内容与压缩类型一致
#[test]
fn nbt_gzip() {
    let mut stream = Cursor::new(Vec::<u8>::new());

    let mut nbt = nbt_types::compound();
    nbt.data.insert(String::new(), NbtType::compound());
    nbt.data
        .insert(String::from("byte"), nbt_types::byte(20).to_nbt());
    nbt.data
        .insert(String::from("int"), nbt_types::int(30).to_nbt());
    nbt.data
        .insert(String::from("long"), nbt_types::long(1).to_nbt());

    let nbt = NbtFile::new(nbt.to_nbt(), CompressType::GZip);

    nbt.write(&mut stream).unwrap();

    stream.seek(SeekFrom::Start(0)).unwrap();

    let nbt1 = NbtFile::read(&mut stream).unwrap();

    assert_eq!(nbt.compress, nbt1.compress);
    assert!(nbt.nbt.eq(&nbt1.nbt));
}

/// Zlib 压缩的 Compound 文件往返
#[test]
fn nbt_zlib() {
    let mut stream = Cursor::new(Vec::<u8>::new());

    let mut nbt = nbt_types::compound();
    nbt.data.insert(String::new(), NbtType::compound());
    nbt.data
        .insert(String::from("byte"), nbt_types::byte(20).to_nbt());
    nbt.data
        .insert(String::from("int"), nbt_types::int(30).to_nbt());
    nbt.data
        .insert(String::from("long"), nbt_types::long(1).to_nbt());

    let nbt = NbtFile::new(nbt.to_nbt(), CompressType::Zlib);

    nbt.write(&mut stream).unwrap();

    stream.seek(SeekFrom::Start(0)).unwrap();

    let nbt1 = NbtFile::read(&mut stream).unwrap();

    assert_eq!(nbt.compress, nbt1.compress);
    assert!(nbt.nbt.eq(&nbt1.nbt));
}

/// Lz4 压缩的 Compound 文件往返
#[test]
fn nbt_lz4() {
    let mut stream = Cursor::new(Vec::<u8>::new());

    let mut nbt = nbt_types::compound();
    nbt.data.insert(String::new(), NbtType::compound());
    nbt.data
        .insert(String::from("byte"), nbt_types::byte(20).to_nbt());
    nbt.data
        .insert(String::from("int"), nbt_types::int(30).to_nbt());
    nbt.data
        .insert(String::from("long"), nbt_types::long(1).to_nbt());

    let nbt = NbtFile::new(nbt.to_nbt(), CompressType::Lz4);

    nbt.write(&mut stream).unwrap();

    stream.seek(SeekFrom::Start(0)).unwrap();

    let nbt1 = NbtFile::read(&mut stream).unwrap();

    assert_eq!(nbt.compress, nbt1.compress);
    assert!(nbt.nbt.eq(&nbt1.nbt));
}

/// 读取真实样本 tests/level.dat，写出后再读回应与原件一致
#[test]
fn load_dat() {
    let path = Path::new("tests").join("level.dat");
    let mut stream = File::open(path).unwrap();

    let nbt = NbtFile::read(&mut stream).unwrap();

    assert_eq!(nbt.nbt.get_num(), NBT_COMPOUND_ORDER);

    let mut stream1 = Cursor::new(Vec::<u8>::new());
    nbt.write(&mut stream1).unwrap();

    stream1.seek(SeekFrom::Start(0)).unwrap();

    let nbt1 = NbtFile::read(&mut stream1).unwrap();

    assert_eq!(nbt.compress, nbt1.compress);
    assert!(nbt.nbt.eq(&nbt1.nbt));
}
