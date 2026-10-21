//! 嵌套结构、命名根标签、getter、List 操作、类型编号映射与错误处理测试。
//!
//! 与 `nbt_types.rs` 的标量/数组往返、`nbt_file.rs` 的压缩往返互补，
//! 重点覆盖：
//! - 多层嵌套 Compound / List-of-Compound 的写入-读取往返；
//! - 非空名称的根标签读取后应被包装进外层 Compound；
//! - `NbtCompound` 的各类 getter；
//! - `NbtList` 的 `set_num` / `set_type` / `remove` 与类型强制；
//! - `NbtType::get_num` / `get_nbt` 的类型编号映射；
//! - 空数组、空列表、空 Compound 的往返；
//! - 损坏 / 截断输入应返回错误。

use std::io::Cursor;

use mml_nbt::{
    NBT_BYTE_ARRAY_ORDER, NBT_BYTE_ORDER, NBT_COMPOUND_ORDER, NBT_DOUBLE_ORDER, NBT_END_ORDER,
    NBT_FLOAT_ORDER, NBT_INT_ARRAY_ORDER, NBT_INT_ORDER, NBT_LIST_ORDER, NBT_LONG_ARRAY_ORDER,
    NBT_LONG_ORDER, NBT_SHORT_ORDER, NBT_STRING_ORDER, NbtType,
    nbt_file::{CompressType, NbtFile},
    nbt_types,
};

/// 往返写入-读取并断言内容与压缩类型一致。
fn round_trip(nbt: NbtType, compress: CompressType) {
    let mut stream = Cursor::new(Vec::<u8>::new());
    let file = NbtFile::new(nbt, compress);
    file.write(&mut stream).unwrap();

    stream.set_position(0);
    let back = NbtFile::read(&mut stream).unwrap();

    assert_eq!(file.compress, back.compress);
    assert!(file.nbt.eq(&back.nbt), "往返后 NBT 内容不一致");
}

/// 构造一个多层嵌套结构：
/// 外层 Compound 内包含 嵌套 Compound、List-of-Compound、List-of-Int、
/// 空 ByteArray、String、标量，以及各种数组。
fn nested_sample() -> NbtType {
    let mut inner = nbt_types::compound();
    inner
        .data
        .insert("count".into(), nbt_types::int(3).to_nbt());
    inner
        .data
        .insert("name".into(), nbt_types::string("nested").to_nbt());

    let mut list_of_comp = nbt_types::list(NBT_COMPOUND_ORDER);
    let mut item1 = nbt_types::compound();
    item1.data.insert("x".into(), nbt_types::int(1).to_nbt());
    item1.data.insert("y".into(), nbt_types::int(2).to_nbt());
    assert!(list_of_comp.add_item(item1.to_nbt()));
    let mut item2 = nbt_types::compound();
    item2.data.insert("x".into(), nbt_types::int(10).to_nbt());
    assert!(list_of_comp.add_item(item2.to_nbt()));

    let mut list_of_int = nbt_types::list(NBT_INT_ORDER);
    assert!(list_of_int.add_item(nbt_types::int(100).to_nbt()));
    assert!(list_of_int.add_item(nbt_types::int(200).to_nbt()));

    let mut root = nbt_types::compound();
    root.data.insert("inner".into(), inner.to_nbt());
    root.data
        .insert("list_of_comp".into(), list_of_comp.to_nbt());
    root.data.insert("list_of_int".into(), list_of_int.to_nbt());
    root.data.insert(
        "byte_array".into(),
        nbt_types::byte_array(vec![1, 2, 3]).to_nbt(),
    );
    root.data.insert(
        "int_array".into(),
        nbt_types::int_array(vec![4, 5, 6]).to_nbt(),
    );
    root.data.insert(
        "long_array".into(),
        nbt_types::long_array(vec![7, 8]).to_nbt(),
    );
    root.data.insert("byte".into(), nbt_types::byte(9).to_nbt());
    root.data
        .insert("short".into(), nbt_types::short(10).to_nbt());
    root.data
        .insert("long".into(), nbt_types::long(11).to_nbt());
    root.data
        .insert("float".into(), nbt_types::float(1.5).to_nbt());
    root.data
        .insert("double".into(), nbt_types::double(2.5).to_nbt());
    root.to_nbt()
}

/// 多层嵌套结构在四种压缩格式下均能往返。
#[test]
fn nested_round_trip_all_compressions() {
    for compress in [
        CompressType::None,
        CompressType::GZip,
        CompressType::Zlib,
        CompressType::Lz4,
    ] {
        round_trip(nested_sample(), compress);
    }
}

/// 含非空名称的根标签：读取后应包装进外层 Compound。
#[test]
fn named_root_wraps_in_compound() {
    // 手工构造：根 Compound 名称 "Level"，内含 int "foo"=42。
    let mut bytes = Vec::new();
    bytes.push(NBT_COMPOUND_ORDER); // 根标签类型
    bytes.extend_from_slice(&5u16.to_be_bytes()); // 名称长度
    bytes.extend_from_slice(b"Level"); // 名称
    bytes.push(NBT_INT_ORDER); // 子标签类型
    bytes.extend_from_slice(&3u16.to_be_bytes()); // 子标签名称长度
    bytes.extend_from_slice(b"foo");
    bytes.extend_from_slice(&42i32.to_be_bytes());
    bytes.push(NBT_END_ORDER); // Compound 结束

    let mut stream = Cursor::new(bytes);
    let file = NbtFile::read(&mut stream).unwrap();

    // 根被包装：外层 Compound 内以 "Level" 为键
    let outer = match &file.nbt {
        NbtType::Compound(c) => c,
        _ => panic!("命名根标签应被包装进 Compound"),
    };
    let level = outer
        .get_compound("Level")
        .expect("应存在键 Level 的嵌套 Compound");
    assert_eq!(level.get_int("foo"), Some(42));
}

/// NbtCompound 各类 getter 的正确值与类型不匹配时返回 None。
#[test]
fn compound_getters() {
    let mut list = nbt_types::list(NBT_INT_ORDER);
    assert!(list.add_item(nbt_types::int(7).to_nbt()));
    assert!(list.add_item(nbt_types::int(8).to_nbt()));

    let mut inner = nbt_types::compound();
    inner.data.insert("deep".into(), nbt_types::int(9).to_nbt());

    let mut com = nbt_types::compound();
    com.data.insert("byte".into(), nbt_types::byte(1).to_nbt());
    com.data
        .insert("short".into(), nbt_types::short(2).to_nbt());
    com.data.insert("int".into(), nbt_types::int(3).to_nbt());
    com.data.insert("long".into(), nbt_types::long(4).to_nbt());
    com.data
        .insert("string".into(), nbt_types::string("hello").to_nbt());
    com.data.insert(
        "byte_array".into(),
        nbt_types::byte_array(vec![5, 6]).to_nbt(),
    );
    com.data.insert(
        "long_array".into(),
        nbt_types::long_array(vec![7, 8]).to_nbt(),
    );
    com.data.insert("list".into(), list.to_nbt());
    com.data.insert("inner".into(), inner.to_nbt());

    assert_eq!(com.get_byte("byte"), Some(1));
    assert_eq!(com.get_short("short"), Some(2));
    assert_eq!(com.get_int("int"), Some(3));
    assert_eq!(com.get_long("long"), Some(4));
    assert_eq!(com.get_string("string"), Some("hello".into()));
    assert_eq!(com.get_byte_array("byte_array").unwrap().data, vec![5, 6]);
    assert_eq!(com.get_long_array("long_array").unwrap().data, vec![7, 8]);

    let got_list = com.get_list("list").expect("应存在 list");
    assert_eq!(got_list.len(), 2);
    let second = got_list.get_item(1).expect("索引 1 应有元素");
    assert!(second.eq(&nbt_types::int(8).to_nbt()));

    let got_inner = com.get_compound("inner").expect("应存在 inner");
    assert_eq!(got_inner.get_int("deep"), Some(9));

    // 键不存在
    assert_eq!(com.get_int("missing"), None);
    assert_eq!(com.get_string("missing"), None);
    assert!(com.get_compound("missing").is_none());
    // 类型不匹配
    assert_eq!(com.get_int("string"), None);
    assert!(com.get_compound("int").is_none());
}

/// NbtList 的 set_num / set_type / remove 与类型强制。
#[test]
fn list_operations() {
    // set_num：合法序号更新类型并清空已有数据
    let mut list = nbt_types::list(NBT_INT_ORDER);
    assert!(list.add_item(nbt_types::int(1).to_nbt()));
    assert!(list.add_item(nbt_types::int(2).to_nbt()));
    assert_eq!(list.len(), 2);

    list.set_num(NBT_STRING_ORDER);
    assert_eq!(list.len(), 0, "set_num 应清空已有数据");
    assert!(list.add_item(nbt_types::string("a").to_nbt()));
    assert!(!list.add_item(nbt_types::int(3).to_nbt()), "类型不符应拒绝");

    // 非法序号被忽略：类型不变、数据不被清空
    let mut list2 = nbt_types::list(NBT_INT_ORDER);
    assert!(list2.add_item(nbt_types::int(5).to_nbt()));
    list2.set_num(99);
    assert_eq!(list2.len(), 1, "非法 set_num 不应清空数据");
    assert!(list2.add_item(nbt_types::int(6).to_nbt()));
    assert!(!list2.add_item(nbt_types::string("x").to_nbt()));

    // set_type 通过 NbtType 实例更新并清空
    list2.set_type(NbtType::compound());
    assert_eq!(list2.len(), 0);
    let mut item = nbt_types::compound();
    item.data.insert("k".into(), nbt_types::byte(0).to_nbt());
    assert!(list2.add_item(item.to_nbt()));

    // remove 返回被移除的元素
    let mut list3 = nbt_types::list(NBT_INT_ORDER);
    list3.add_item(nbt_types::int(10).to_nbt());
    list3.add_item(nbt_types::int(20).to_nbt());
    let removed = list3.remove(0);
    assert!(removed.eq(&nbt_types::int(10).to_nbt()));
    assert_eq!(list3.len(), 1);
}

/// NbtType::get_num 与 get_nbt 的编号映射。
#[test]
fn type_number_mapping() {
    let cases: [(u8, NbtType); 13] = [
        (NBT_END_ORDER, NbtType::end()),
        (NBT_BYTE_ORDER, NbtType::byte()),
        (NBT_SHORT_ORDER, NbtType::short()),
        (NBT_INT_ORDER, NbtType::int()),
        (NBT_LONG_ORDER, NbtType::long()),
        (NBT_FLOAT_ORDER, NbtType::float()),
        (NBT_DOUBLE_ORDER, NbtType::double()),
        (NBT_BYTE_ARRAY_ORDER, NbtType::byte_array()),
        (NBT_STRING_ORDER, NbtType::string()),
        (NBT_LIST_ORDER, NbtType::list()),
        (NBT_COMPOUND_ORDER, NbtType::compound()),
        (NBT_INT_ARRAY_ORDER, NbtType::int_array()),
        (NBT_LONG_ARRAY_ORDER, NbtType::long_array()),
    ];

    for (num, nbt) in cases {
        assert_eq!(nbt.get_num(), num, "get_num 映射错误");
        let from_num = NbtType::get_nbt(num).expect("合法编号应能创建标签");
        assert_eq!(from_num.get_num(), num, "get_nbt 往返映射错误");
    }

    // 非法编号
    assert!(NbtType::get_nbt(13).is_none());
    assert!(NbtType::get_nbt(255).is_none());
}

/// 空数组与空 Compound 的往返。
///
/// 空 List 单独断言：按 NBT 规范，空列表写入时元素类型固定为 0，即 TAG_End，
/// 读回后元素类型不还原，故不与原值做全等比较。
#[test]
fn empty_containers_round_trip() {
    for nbt in [
        nbt_types::byte_array(vec![]).to_nbt(),
        nbt_types::int_array(vec![]).to_nbt(),
        nbt_types::long_array(vec![]).to_nbt(),
        nbt_types::compound().to_nbt(),
    ] {
        round_trip(nbt, CompressType::None);
    }

    // 空 List：往返后仍为空列表，元素类型按规范写为 0
    let mut stream = Cursor::new(Vec::<u8>::new());
    let file = NbtFile::new(nbt_types::list(NBT_INT_ORDER).to_nbt(), CompressType::None);
    file.write(&mut stream).unwrap();
    stream.set_position(0);
    let back = NbtFile::read(&mut stream).unwrap();
    match &back.nbt {
        NbtType::List(list) => {
            assert_eq!(list.len(), 0);
        }
        _ => panic!("空列表应读取为 List"),
    }
}

/// 损坏 / 截断输入应返回错误而非 panic。
#[test]
fn corrupt_input_returns_error() {
    // 空输入：不足 3 字节
    let mut empty = Cursor::new(Vec::<u8>::new());
    assert!(NbtFile::read(&mut empty).is_err());

    // 2 字节但非合法单标签
    let mut two = Cursor::new(vec![0x02, 0x00]);
    assert!(NbtFile::read(&mut two).is_err());

    // 根标签类型非法：0x63 = 99，不在 0-12 范围内
    let mut bad_type = Cursor::new(vec![0x63, 0x00, 0x00]);
    assert!(NbtFile::read(&mut bad_type).is_err());

    // 声明为 int 但缺少 4 字节数据
    let mut truncated = Cursor::new(vec![NBT_INT_ORDER, 0x00, 0x00]);
    assert!(NbtFile::read(&mut truncated).is_err());

    // 空 Compound 却提前截断
    let mut truncated_compound = Cursor::new(vec![NBT_COMPOUND_ORDER, 0x00, 0x00]);
    assert!(NbtFile::read(&mut truncated_compound).is_err());

    // GZip 魔数但数据不是有效压缩流
    let mut fake_gzip = Cursor::new(vec![0x1F, 0x8B, 0x08, 0x00, 0x00, 0x00]);
    assert!(NbtFile::read(&mut fake_gzip).is_err());
}
