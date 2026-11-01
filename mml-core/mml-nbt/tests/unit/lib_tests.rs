use super::*;
use crate::nbt_types;

/// is_nbt_num 的合法边界为 0–12
#[test]
fn is_nbt_num_boundaries() {
    assert!(is_nbt_num(NBT_END_ORDER));
    assert!(is_nbt_num(NBT_BYTE_ORDER));
    assert!(is_nbt_num(6));
    assert!(is_nbt_num(NBT_LONG_ARRAY_ORDER));
    // 越界
    assert!(!is_nbt_num(13));
    assert!(!is_nbt_num(255));
}

/// get_nbt 按类型序号创建对应变体，越界返回 None
#[test]
fn get_nbt_mapping() {
    assert!(matches!(NbtType::get_nbt(0), Some(NbtType::End(_))));
    assert!(matches!(NbtType::get_nbt(1), Some(NbtType::Byte(_))));
    assert!(matches!(NbtType::get_nbt(2), Some(NbtType::Short(_))));
    assert!(matches!(NbtType::get_nbt(3), Some(NbtType::Int(_))));
    assert!(matches!(NbtType::get_nbt(4), Some(NbtType::Long(_))));
    assert!(matches!(NbtType::get_nbt(5), Some(NbtType::Float(_))));
    assert!(matches!(NbtType::get_nbt(6), Some(NbtType::Double(_))));
    assert!(matches!(NbtType::get_nbt(7), Some(NbtType::ByteArray(_))));
    assert!(matches!(NbtType::get_nbt(8), Some(NbtType::String(_))));
    assert!(matches!(NbtType::get_nbt(9), Some(NbtType::List(_))));
    assert!(matches!(NbtType::get_nbt(10), Some(NbtType::Compound(_))));
    assert!(matches!(NbtType::get_nbt(11), Some(NbtType::IntArray(_))));
    assert!(matches!(NbtType::get_nbt(12), Some(NbtType::LongArray(_))));
    // 越界
    assert!(NbtType::get_nbt(13).is_none());
    assert!(NbtType::get_nbt(255).is_none());
}

/// get_num 与 get_nbt 互为逆映射
#[test]
fn get_num_round_trip() {
    let samples = [
        NbtType::end(),
        nbt_types::byte(1).to_nbt(),
        nbt_types::short(1).to_nbt(),
        nbt_types::int(1).to_nbt(),
        nbt_types::long(1).to_nbt(),
        nbt_types::float(1.0).to_nbt(),
        nbt_types::double(1.0).to_nbt(),
        nbt_types::byte_array(vec![]).to_nbt(),
        nbt_types::string("").to_nbt(),
        nbt_types::list(NBT_INT_ORDER).to_nbt(),
        nbt_types::compound().to_nbt(),
        nbt_types::int_array(vec![]).to_nbt(),
        nbt_types::long_array(vec![]).to_nbt(),
    ];

    for sample in samples {
        let num = sample.get_num();
        assert!(is_nbt_num(num));
        // get_nbt 创建的是默认值实例，只校验序号映射的一致性
        let rebuilt = NbtType::get_nbt(num).unwrap();
        assert_eq!(rebuilt.get_num(), num);
    }
}

/// 类型序号常量的值应符合 NBT 规范
#[test]
fn order_constants() {
    assert_eq!(NBT_END_ORDER, 0);
    assert_eq!(NBT_BYTE_ORDER, 1);
    assert_eq!(NBT_SHORT_ORDER, 2);
    assert_eq!(NBT_INT_ORDER, 3);
    assert_eq!(NBT_LONG_ORDER, 4);
    assert_eq!(NBT_FLOAT_ORDER, 5);
    assert_eq!(NBT_DOUBLE_ORDER, 6);
    assert_eq!(NBT_BYTE_ARRAY_ORDER, 7);
    assert_eq!(NBT_STRING_ORDER, 8);
    assert_eq!(NBT_LIST_ORDER, 9);
    assert_eq!(NBT_COMPOUND_ORDER, 10);
    assert_eq!(NBT_INT_ARRAY_ORDER, 11);
    assert_eq!(NBT_LONG_ARRAY_ORDER, 12);
}

