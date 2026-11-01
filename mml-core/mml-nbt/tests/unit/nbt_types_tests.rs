use std::io::Cursor;

/// Java 写的 NBT 会把补充平面字符拆成两个 3 字节序列，`from_utf8` 直接拒绝
#[test]
fn decode_string_accepts_cesu8_surrogate_pair() {
    // ED A0 BE ED BA 93 = D83E DE93 = U+1FA93
    let text = decode_string(vec![0xED, 0xA0, 0xBE, 0xED, 0xBA, 0x93]);
    assert_eq!(text, "\u{1FA93}");
}

/// 合法 UTF-8 与落单代理项都要有结果，不能把整棵树读挂
#[test]
fn decode_string_keeps_plain_utf8_and_survives_broken_bytes() {
    assert_eq!(decode_string("普通文本".as_bytes().to_vec()), "普通文本");
    assert_eq!(decode_string(vec![0xFF, 0xFE]), "\u{FFFD}\u{FFFD}");
}

use super::*;
use crate::{
    NBT_BYTE_ARRAY_ORDER, NBT_BYTE_ORDER, NBT_COMPOUND_ORDER, NBT_INT_ARRAY_ORDER,
    NBT_INT_ORDER, NBT_LIST_ORDER, NBT_LONG_ARRAY_ORDER, NBT_LONG_ORDER, NBT_SHORT_ORDER,
    NBT_STRING_ORDER,
};

/// 从字节切片中读取标签数据
fn read_from<T: NbtStream>(mut nbt: T, bytes: &[u8]) -> T {
    let mut cursor = Cursor::new(bytes);
    nbt.read(&mut cursor).unwrap();
    nbt
}

/// 将标签写入内存并返回字节序列
fn write_to<T: NbtStream>(nbt: &T) -> Vec<u8> {
    let mut cursor = Cursor::new(Vec::<u8>::new());
    nbt.write(&mut cursor).unwrap();
    cursor.into_inner()
}

// ---------- 标量类型 ----------

/// Byte 往返，写入仅占 1 字节
#[test]
fn byte_round_trip() {
    for value in [0u8, 1, 127, 128, 255] {
        let nbt = byte(value);
        assert_eq!(write_to(&nbt), vec![value]);
        assert_eq!(read_from(NbtByte::default(), &[value]).data, value);
    }
}

/// Short 使用大端序读写
#[test]
fn short_big_endian() {
    let nbt = short(0x0102);
    assert_eq!(write_to(&nbt), vec![0x01, 0x02]);
    assert_eq!(read_from(NbtShort::default(), &[0xFF, 0xFE]).data, -2);
    assert_eq!(read_from(NbtShort::default(), &[0x80, 0x00]).data, i16::MIN);
}

/// Int 使用大端序读写
#[test]
fn int_big_endian() {
    let nbt = int(0x01020304);
    assert_eq!(write_to(&nbt), vec![0x01, 0x02, 0x03, 0x04]);
    assert_eq!(
        read_from(NbtInt::default(), &[0xFF, 0xFF, 0xFF, 0xFE]).data,
        -2
    );
    assert_eq!(
        read_from(NbtInt::default(), &[0x00, 0x00, 0x00, 0x00]).data,
        0
    );
}

/// Long 使用大端序读写
#[test]
fn long_big_endian() {
    let nbt = long(0x0102030405060708);
    assert_eq!(
        write_to(&nbt),
        vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
    );
    assert_eq!(
        read_from(
            NbtLong::default(),
            &[0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
        )
        .data,
        i64::MIN
    );
}

/// Float / Double 按位精确往返
#[test]
fn float_double_round_trip() {
    let f = float(1.5);
    assert_eq!(write_to(&f), f32::to_be_bytes(1.5).to_vec());
    let bytes = f32::to_be_bytes(-0.25);
    assert_eq!(read_from(NbtFloat::default(), &bytes).data, -0.25);

    let d = double(3.141592653589793);
    assert_eq!(write_to(&d), f64::to_be_bytes(3.141592653589793).to_vec());
    let bytes = f64::to_be_bytes(-2.5);
    assert_eq!(read_from(NbtDouble::default(), &bytes).data, -2.5);
}

// ---------- 数组类型 ----------

/// ByteArray 的二进制格式：4 字节长度 + 数据
#[test]
fn byte_array_format() {
    let nbt = byte_array(vec![0xAB, 0xCD]);
    assert_eq!(write_to(&nbt), vec![0, 0, 0, 2, 0xAB, 0xCD]);

    let back = read_from(NbtByteArray::default(), &[0, 0, 0, 3, 1, 2, 3]);
    assert_eq!(back.data, vec![1, 2, 3]);

    // 空数组
    let empty = read_from(NbtByteArray::default(), &[0, 0, 0, 0]);
    assert!(empty.data.is_empty());
}

/// IntArray 的二进制格式：4 字节元素个数 + 每元素 4 字节，大端序
#[test]
fn int_array_format() {
    let nbt = int_array(vec![1, -1]);
    assert_eq!(
        write_to(&nbt),
        vec![0, 0, 0, 2, 0, 0, 0, 1, 0xFF, 0xFF, 0xFF, 0xFF]
    );

    let back = read_from(
        NbtIntArray::default(),
        &[0, 0, 0, 2, 0, 0, 0, 1, 0xFF, 0xFF, 0xFF, 0xFF],
    );
    assert_eq!(back.data, vec![1, -1]);
}

/// LongArray 的二进制格式：4 字节元素个数 + 每元素 8 字节，大端序
#[test]
fn long_array_format() {
    let nbt = long_array(vec![i64::MIN, 1]);
    let bytes = write_to(&nbt);
    assert_eq!(&bytes[..4], &[0, 0, 0, 2]);
    assert_eq!(bytes.len(), 4 + 16);

    let back = read_from(NbtLongArray::default(), &bytes);
    assert_eq!(back.data, vec![i64::MIN, 1]);
}

// ---------- 字符串类型 ----------

/// String 长度前缀为字节数而非字符数，多字节 UTF-8 可正确往返
#[test]
fn string_utf8_length_is_bytes() {
    // "颜" 占 3 个 UTF-8 字节
    let nbt = string("颜");
    let bytes = write_to(&nbt);
    assert_eq!(&bytes[..2], &[0, 3]);
    assert_eq!(bytes.len(), 2 + 3);

    let back = read_from(NbtString::default(), &bytes);
    assert_eq!(back.data, "颜");

    // 空字符串
    let empty = write_to(&string(""));
    assert_eq!(empty, vec![0, 0]);
}

/// String 读取遇到非法 UTF-8 不再整棵树失败，按替换符收下
#[test]
fn string_invalid_utf8_falls_back_to_replacement() {
    let mut nbt = NbtString::default();
    let mut cursor = Cursor::new([0u8, 1, 0xFF]);
    assert!(nbt.read(&mut cursor).is_ok());
    assert_eq!(nbt.data, "\u{FFFD}");
}

// ---------- List 类型 ----------

/// add_item 强制元素类型一致
#[test]
fn list_type_enforcement() {
    let mut list = list(NBT_INT_ORDER);
    // 类型不匹配应被拒绝
    assert!(!list.add_item(byte(1).to_nbt()));
    assert_eq!(list.len(), 0);
    // 类型匹配应成功
    assert!(list.add_item(int(1).to_nbt()));
    assert!(list.add_item(int(2).to_nbt()));
    assert_eq!(list.len(), 2);
}

/// set_num / set_type 的行为：非法序号被忽略，合法序号清空数据
#[test]
fn list_set_num_and_set_type() {
    let mut list = list(NBT_INT_ORDER);
    assert!(list.add_item(int(1).to_nbt()));

    // 非法序号：忽略操作，数据保留
    list.set_num(13);
    assert_eq!(list.len(), 1);

    // 合法序号：清空数据
    list.set_num(NBT_BYTE_ORDER);
    assert_eq!(list.len(), 0);
    assert!(!list.add_item(int(1).to_nbt()));
    assert!(list.add_item(byte(2).to_nbt()));

    // set_type 同样清空数据
    list.set_type(NbtType::long_array());
    assert_eq!(list.len(), 0);
    assert!(list.add_item(long_array(vec![1]).to_nbt()));
}

/// 空列表写入时元素类型序号为 0，即 TAG_End
#[test]
fn list_empty_writes_end_type() {
    let list = list(NBT_INT_ORDER);
    let bytes = write_to(&list);
    assert_eq!(bytes, vec![0, 0, 0, 0, 0]);
}

/// List 读写往返，含 get_item / remove / iter
#[test]
fn list_round_trip_and_accessors() {
    let mut list = list(NBT_SHORT_ORDER);
    assert!(list.add_item(short(1).to_nbt()));
    assert!(list.add_item(short(-2).to_nbt()));
    assert!(list.add_item(short(3).to_nbt()));

    let bytes = write_to(&list);
    // 类型序号 + 元素个数 + 每元素 2 字节
    assert_eq!(bytes, vec![2, 0, 0, 0, 3, 0, 1, 0xFF, 0xFE, 0, 3]);

    let mut back = NbtList::default();
    let mut cursor = Cursor::new(bytes.as_slice());
    back.read(&mut cursor).unwrap();

    assert_eq!(back.len(), 3);
    assert_eq!(back.get_item(1).unwrap().as_short().unwrap().data, -2);
    assert_eq!(back.iter().count(), 3);

    // remove 返回被移除的元素
    let removed = back.remove(0);
    assert_eq!(removed.as_short().unwrap().data, 1);
    assert_eq!(back.len(), 2);
}

/// List 读取时元素类型序号非法应返回错误
#[test]
fn list_invalid_type_returns_error() {
    let mut nbt = NbtList::default();
    let mut cursor = Cursor::new([13u8, 0, 0, 0, 0]);
    assert!(nbt.read(&mut cursor).is_err());
}

// ---------- Compound 类型 ----------

/// 单条目 Compound 的写入格式：类型序号 + 键名 + 值 + End
#[test]
fn compound_write_format() {
    let mut com = compound();
    com.data.insert("k".into(), byte(7).to_nbt());
    let bytes = write_to(&com);
    assert_eq!(
        bytes,
        vec![
            NBT_BYTE_ORDER,
            0,
            1,
            b'k', // 键名长度 + 键名
            7,    // 值
            0,    // TAG_End 终止
        ]
    );
}

/// 手工构造字节读取 Compound，并验证各类 getter
#[test]
fn compound_read_and_getters() {
    // { "a": byte 1, "s": short 2, "i": int 3, "l": long 4, "str": "hi" }
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&[NBT_BYTE_ORDER, 0, 1, b'a', 1]);
    bytes.extend_from_slice(&[NBT_SHORT_ORDER, 0, 1, b's', 0, 2]);
    bytes.extend_from_slice(&[NBT_INT_ORDER, 0, 1, b'i', 0, 0, 0, 3]);
    bytes.extend_from_slice(&[NBT_LONG_ORDER, 0, 1, b'l', 0, 0, 0, 0, 0, 0, 0, 4]);
    bytes.extend_from_slice(&[NBT_STRING_ORDER, 0, 3, b's', b't', b'r', 0, 2, b'h', b'i']);
    bytes.push(0); // TAG_End

    let mut nbt = NbtCompound::default();
    let mut cursor = Cursor::new(bytes.as_slice());
    nbt.read(&mut cursor).unwrap();

    assert_eq!(nbt.get_byte("a"), Some(1));
    assert_eq!(nbt.get_short("s"), Some(2));
    assert_eq!(nbt.get_int("i"), Some(3));
    assert_eq!(nbt.get_long("l"), Some(4));
    assert_eq!(nbt.get_string("str"), Some("hi".to_string()));

    // 类型不匹配时 getter 应返回 None
    assert_eq!(nbt.get_int("a"), None);
    assert_eq!(nbt.get_byte("s"), None);
    assert_eq!(nbt.get_string("i"), None);
    // 不存在的键
    assert_eq!(nbt.get_byte("missing"), None);

    // 引用型 getter
    assert!(nbt.get("a").is_some());
    assert!(nbt.get_mut("a").is_some());
    assert!(nbt.get_compound("a").is_none());
    assert!(nbt.get_list("a").is_none());
    assert!(nbt.get_byte_array("a").is_none());
    assert!(nbt.get_long_array("a").is_none());
}

/// 嵌套 Compound 往返
#[test]
fn compound_nested_round_trip() {
    let mut inner = compound();
    inner.data.insert("x".into(), int(9).to_nbt());

    let mut root = compound();
    root.data.insert("inner".into(), inner.to_nbt());
    root.data
        .insert("arr".into(), byte_array(vec![1, 2, 3]).to_nbt());
    root.data
        .insert("ia".into(), int_array(vec![1, 2, 3, -4]).to_nbt());
    root.data
        .insert("la".into(), long_array(vec![5, -6]).to_nbt());

    let bytes = write_to(&root);
    let mut back = NbtCompound::default();
    let mut cursor = Cursor::new(bytes.as_slice());
    back.read(&mut cursor).unwrap();

    // 先取嵌套数据，再比较整体；to_nbt 会消耗所有权，不能提前调用
    assert_eq!(back.get_compound("inner").unwrap().get_int("x"), Some(9));
    assert_eq!(back.get_byte_array("arr").unwrap().data, vec![1, 2, 3]);
    // Compound 没有 get_int_array，需通过 get + as_int_array 取值
    assert!(back.get_long_array("ia").is_none());
    assert_eq!(
        back.get("ia").unwrap().as_int_array().unwrap().data,
        vec![1, 2, 3, -4]
    );
    assert_eq!(
        back.get("la").unwrap().as_long_array().unwrap().data,
        vec![5, -6]
    );

    assert!(root.eq(&back.to_nbt()));
}

/// Compound 读取时遇到非法类型序号应返回错误
#[test]
fn compound_invalid_type_returns_error() {
    let mut nbt = NbtCompound::default();
    // 类型序号 13 非法
    let mut cursor = Cursor::new([13u8]);
    assert!(nbt.read(&mut cursor).is_err());
}

/// eq 语义：同型同值为真，异型或异值为假
#[test]
fn eq_semantics() {
    let mut com1 = compound();
    com1.data.insert("a".into(), int(1).to_nbt());
    let nbt1 = com1.to_nbt();

    // 同内容
    let mut com2 = compound();
    com2.data.insert("a".into(), int(1).to_nbt());
    let nbt2 = com2.to_nbt();
    assert!(nbt1.eq(&nbt2));

    // 值不同
    let mut com3 = compound();
    com3.data.insert("a".into(), int(2).to_nbt());
    let nbt3 = com3.to_nbt();
    assert!(!nbt1.eq(&nbt3));

    // 键不同
    let mut com4 = compound();
    com4.data.insert("b".into(), int(1).to_nbt());
    let nbt4 = com4.to_nbt();
    assert!(!nbt1.eq(&nbt4));

    // 数量不同
    let mut com5 = compound();
    com5.data.insert("a".into(), int(1).to_nbt());
    com5.data.insert("b".into(), int(2).to_nbt());
    let nbt5 = com5.to_nbt();
    assert!(!nbt1.eq(&nbt5));

    // 与其它类型比较
    assert!(!nbt1.eq(&NbtType::end()));
    assert!(!int(1).to_nbt().eq(&long(1).to_nbt()));
    assert!(int(1).to_nbt().eq(&int(1).to_nbt()));

    // List eq：类型序号、数量、逐元素比较
    let mut l1 = list(NBT_INT_ORDER);
    assert!(l1.add_item(int(1).to_nbt()));
    let mut l2 = list(NBT_INT_ORDER);
    assert!(l2.add_item(int(1).to_nbt()));
    let l1_nbt = l1.to_nbt();
    assert!(l1_nbt.eq(&l2.to_nbt()));

    let mut l3 = list(NBT_SHORT_ORDER);
    assert!(l3.add_item(short(1).to_nbt()));
    assert!(!l1_nbt.eq(&l3.to_nbt()));
}

/// End 标签读写均为空操作
#[test]
fn end_no_op() {
    assert_eq!(write_to(&end()), Vec::<u8>::new());
    read_from(end(), &[]);
}

/// NbtType 的 as_* 系列方法在类型不匹配时应返回 None
#[test]
fn nbt_type_as_helpers() {
    let mut nbt = int(1).to_nbt();
    assert!(nbt.as_int().is_some());
    assert!(nbt.as_int_mut().is_some());
    assert!(nbt.as_byte().is_none());
    assert!(nbt.as_short().is_none());
    assert!(nbt.as_long().is_none());
    assert!(nbt.as_float().is_none());
    assert!(nbt.as_double().is_none());
    assert!(nbt.as_string().is_none());
    assert!(nbt.as_list().is_none());
    assert!(nbt.as_compound().is_none());
    assert!(nbt.as_byte_array().is_none());
    assert!(nbt.as_int_array().is_none());
    assert!(nbt.as_long_array().is_none());
    assert!(nbt.as_end().is_none());
    // get_compound 消耗所有权，非 Compound 返回 None
    assert!(nbt.get_compound().is_none());
}

/// Display 输出应符合 SNBT 风格
#[test]
fn display_format() {
    assert_eq!(byte(1).to_nbt().to_string(), "1b");
    assert_eq!(short(2).to_nbt().to_string(), "2s");
    assert_eq!(int(3).to_nbt().to_string(), "3");
    assert_eq!(long(4).to_nbt().to_string(), "4L");
    assert_eq!(float(1.5).to_nbt().to_string(), "1.5f");
    assert_eq!(double(2.5).to_nbt().to_string(), "2.5d");
    assert_eq!(string("a\"b").to_nbt().to_string(), "\"a\\\"b\"");
    assert_eq!(NbtType::end().to_string(), "END");

    assert_eq!(byte_array(vec![1, 2]).to_nbt().to_string(), "[B;1B, 2B]");
    assert_eq!(int_array(vec![1, 2]).to_nbt().to_string(), "[I;1, 2]");
    assert_eq!(long_array(vec![3]).to_nbt().to_string(), "[L;3L]");

    let mut list = list(NBT_INT_ORDER);
    assert!(list.add_item(int(1).to_nbt()));
    assert!(list.add_item(int(2).to_nbt()));
    assert_eq!(list.to_nbt().to_string(), "[1, 2]");

    let mut com = compound();
    com.data.insert("k".into(), byte(1).to_nbt());
    assert_eq!(com.to_nbt().to_string(), "{k: 1b}");
}

/// 数组类型序号常量与 get_num / get_nbt 的映射一致性，详见 lib.rs 测试
#[test]
fn type_orders() {
    assert_eq!(byte(0).to_nbt().get_num(), NBT_BYTE_ORDER);
    assert_eq!(byte_array(vec![]).to_nbt().get_num(), NBT_BYTE_ARRAY_ORDER);
    assert_eq!(long_array(vec![]).to_nbt().get_num(), NBT_LONG_ARRAY_ORDER);
    assert_eq!(list(NBT_INT_ORDER).to_nbt().get_num(), NBT_LIST_ORDER);
    assert_eq!(compound().to_nbt().get_num(), NBT_COMPOUND_ORDER);
    assert_eq!(int_array(vec![]).to_nbt().get_num(), NBT_INT_ARRAY_ORDER);
    assert_eq!(string("").to_nbt().get_num(), NBT_STRING_ORDER);
}

