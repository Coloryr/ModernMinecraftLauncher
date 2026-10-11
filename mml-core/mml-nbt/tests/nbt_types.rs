//! 各标签类型经 `NbtFile` 读写往返的冒烟测试，覆盖全部 13 种类型。
//!
//! 结构嵌套、getter 与错误处理见 `structure.rs`。

use std::io::{Cursor, Seek, SeekFrom};

use mml_nbt::{
    NbtType,
    nbt_file::{CompressType, NbtFile},
    nbt_types,
};

/// End 标签往返后内容与压缩类型一致
#[test]
fn nbt_end() {
    let mut stream = Cursor::new(Vec::<u8>::new());

    let nbt = NbtFile::new(nbt_types::end().to_nbt(), CompressType::None);

    nbt.write(&mut stream).unwrap();

    stream.seek(SeekFrom::Start(0)).unwrap();

    let nbt1 = NbtFile::read(&mut stream).unwrap();

    assert_eq!(nbt.compress, nbt1.compress);
    assert!(nbt.nbt.eq(&nbt1.nbt));
}

/// Byte 标签往返
#[test]
fn nbt_byte() {
    let mut stream = Cursor::new(Vec::<u8>::new());

    let nbt = NbtFile::new(nbt_types::byte(1).to_nbt(), CompressType::None);

    nbt.write(&mut stream).unwrap();

    stream.seek(SeekFrom::Start(0)).unwrap();

    let nbt1 = NbtFile::read(&mut stream).unwrap();

    assert_eq!(nbt.compress, nbt1.compress);
    assert!(nbt.nbt.eq(&nbt1.nbt));
}

/// Short 标签往返
#[test]
fn nbt_short() {
    let mut stream = Cursor::new(Vec::<u8>::new());

    let nbt = NbtFile::new(nbt_types::short(1).to_nbt(), CompressType::None);

    nbt.write(&mut stream).unwrap();

    stream.seek(SeekFrom::Start(0)).unwrap();

    let nbt1 = NbtFile::read(&mut stream).unwrap();

    assert_eq!(nbt.compress, nbt1.compress);
    assert!(nbt.nbt.eq(&nbt1.nbt));
}

/// Long 标签往返
#[test]
fn nbt_long() {
    let mut stream = Cursor::new(Vec::<u8>::new());

    let nbt = NbtFile::new(nbt_types::long(1).to_nbt(), CompressType::None);

    nbt.write(&mut stream).unwrap();

    stream.seek(SeekFrom::Start(0)).unwrap();

    let nbt1 = NbtFile::read(&mut stream).unwrap();

    assert_eq!(nbt.compress, nbt1.compress);
    assert!(nbt.nbt.eq(&nbt1.nbt));
}

/// Float 标签往返
#[test]
fn nbt_float() {
    let mut stream = Cursor::new(Vec::<u8>::new());

    let nbt = NbtFile::new(nbt_types::float(1.0).to_nbt(), CompressType::None);

    nbt.write(&mut stream).unwrap();

    stream.seek(SeekFrom::Start(0)).unwrap();

    let nbt1 = NbtFile::read(&mut stream).unwrap();

    assert_eq!(nbt.compress, nbt1.compress);
    assert!(nbt.nbt.eq(&nbt1.nbt));
}

/// Double 标签往返
#[test]
fn nbt_double() {
    let mut stream = Cursor::new(Vec::<u8>::new());

    let nbt = NbtFile::new(nbt_types::double(1.0).to_nbt(), CompressType::None);

    nbt.write(&mut stream).unwrap();

    stream.seek(SeekFrom::Start(0)).unwrap();

    let nbt1 = NbtFile::read(&mut stream).unwrap();

    assert_eq!(nbt.compress, nbt1.compress);
    assert!(nbt.nbt.eq(&nbt1.nbt));
}

/// String 标签往返
#[test]
fn nbt_string() {
    let mut stream = Cursor::new(Vec::<u8>::new());

    let nbt = NbtFile::new(nbt_types::string("value").to_nbt(), CompressType::None);

    nbt.write(&mut stream).unwrap();

    stream.seek(SeekFrom::Start(0)).unwrap();

    let nbt1 = NbtFile::read(&mut stream).unwrap();

    assert_eq!(nbt.compress, nbt1.compress);
    assert!(nbt.nbt.eq(&nbt1.nbt));
}

/// ByteArray 标签往返
#[test]
fn nbt_byte_array() {
    let mut stream = Cursor::new(Vec::<u8>::new());

    let nbt = nbt_types::byte_array([0, 1, 2, 3, 4, 5].to_vec());
    let nbt = NbtFile::new(nbt.to_nbt(), CompressType::None);

    nbt.write(&mut stream).unwrap();

    stream.seek(SeekFrom::Start(0)).unwrap();

    let nbt1 = NbtFile::read(&mut stream).unwrap();

    assert_eq!(nbt.compress, nbt1.compress);
    assert!(nbt.nbt.eq(&nbt1.nbt));
}

/// IntArray 标签往返
#[test]
fn nbt_int_array() {
    let mut stream = Cursor::new(Vec::<u8>::new());

    let nbt = nbt_types::int_array([78, 89, 12, 23, 46].to_vec());
    let nbt = NbtFile::new(nbt.to_nbt(), CompressType::None);

    nbt.write(&mut stream).unwrap();

    stream.seek(SeekFrom::Start(0)).unwrap();

    let nbt1 = NbtFile::read(&mut stream).unwrap();

    assert_eq!(nbt.compress, nbt1.compress);
    assert!(nbt.nbt.eq(&nbt1.nbt));
}

/// LongArray 标签往返
#[test]
fn nbt_long_array() {
    let mut stream = Cursor::new(Vec::<u8>::new());

    let nbt = nbt_types::long_array([234, 345, 456, 789, 3456].to_vec());
    let nbt = NbtFile::new(nbt.to_nbt(), CompressType::None);

    nbt.write(&mut stream).unwrap();

    stream.seek(SeekFrom::Start(0)).unwrap();

    let nbt1 = NbtFile::read(&mut stream).unwrap();

    assert_eq!(nbt.compress, nbt1.compress);
    assert!(nbt.nbt.eq(&nbt1.nbt));
}

/// List 标签往返，并验证元素类型不符时拒绝加入
#[test]
fn nbt_list() {
    let mut stream = Cursor::new(Vec::<u8>::new());

    let mut nbt = nbt_types::list(NbtType::byte().get_num());

    assert!(!nbt.add_item(NbtType::int()));
    assert!(nbt.add_item(nbt_types::byte(1).to_nbt()));
    assert!(nbt.add_item(nbt_types::byte(2).to_nbt()));

    let nbt = NbtFile::new(nbt.to_nbt(), CompressType::None);

    nbt.write(&mut stream).unwrap();

    stream.seek(SeekFrom::Start(0)).unwrap();

    let nbt1 = NbtFile::read(&mut stream).unwrap();

    assert_eq!(nbt.compress, nbt1.compress);
    assert!(nbt.nbt.eq(&nbt1.nbt));
}

/// Compound 标签往返，含空键名与嵌套 Compound
#[test]
fn nbt_compound() {
    let mut stream = Cursor::new(Vec::<u8>::new());

    let mut nbt = nbt_types::compound();
    nbt.data.insert(String::new(), NbtType::compound());
    nbt.data.insert(String::from("byte"), NbtType::byte());
    nbt.data.insert(String::from("int"), NbtType::int());
    nbt.data
        .insert(String::from("long"), nbt_types::long(1).to_nbt());

    let nbt = NbtFile::new(nbt.to_nbt(), CompressType::None);

    nbt.write(&mut stream).unwrap();

    stream.seek(SeekFrom::Start(0)).unwrap();

    let nbt1 = NbtFile::read(&mut stream).unwrap();

    assert_eq!(nbt.compress, nbt1.compress);
    assert!(nbt.nbt.eq(&nbt1.nbt));
}
