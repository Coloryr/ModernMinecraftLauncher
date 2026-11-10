//! 便捷构造器
//!
//! 每个标签类型对应一个短函数，省去写结构体名。

use super::array::{NbtByteArray, NbtIntArray, NbtLongArray};
use super::compound::NbtCompound;
use super::list::NbtList;
use super::scalar::{NbtByte, NbtDouble, NbtEnd, NbtFloat, NbtInt, NbtLong, NbtShort};
use super::string::NbtString;

/// 创建空的 NbtEnd 实例
///
/// # 返回值
///
/// 新建的 `NbtEnd` 实例
pub fn end() -> NbtEnd {
    NbtEnd::new()
}

/// 创建 NbtByte 实例
///
/// - `data`: 字节数据
///
/// # 返回值
///
/// 新建的 `NbtByte` 实例
pub fn byte(data: u8) -> NbtByte {
    NbtByte::new(data)
}

/// 创建 NbtShort 实例
///
/// - `data`: 短整型数据
///
/// # 返回值
///
/// 新建的 `NbtShort` 实例
pub fn short(data: i16) -> NbtShort {
    NbtShort::new(data)
}

/// 创建 NbtInt 实例
///
/// - `data`: 整型数据
///
/// # 返回值
///
/// 新建的 `NbtInt` 实例
pub fn int(data: i32) -> NbtInt {
    NbtInt::new(data)
}

/// 创建 NbtLong 实例
///
/// - `data`: 长整型数据
///
/// # 返回值
///
/// 新建的 `NbtLong` 实例
pub fn long(data: i64) -> NbtLong {
    NbtLong::new(data)
}

/// 创建 NbtFloat 实例
///
/// - `data`: 浮点数据
///
/// # 返回值
///
/// 新建的 `NbtFloat` 实例
pub fn float(data: f32) -> NbtFloat {
    NbtFloat::new(data)
}

/// 创建 NbtDouble 实例
///
/// - `data`: 双精度浮点数据
///
/// # 返回值
///
/// 新建的 `NbtDouble` 实例
pub fn double(data: f64) -> NbtDouble {
    NbtDouble::new(data)
}

/// 创建 NbtByteArray 实例
///
/// - `data`: 字节数组数据
///
/// # 返回值
///
/// 新建的 `NbtByteArray` 实例
pub fn byte_array(data: Vec<u8>) -> NbtByteArray {
    NbtByteArray::new(data)
}

/// 创建 NbtString 实例
///
/// - `data`: 字符串数据
///
/// # 返回值
///
/// 新建的 `NbtString` 实例
pub fn string(data: &str) -> NbtString {
    NbtString::new(String::from(data))
}

/// 创建指定元素类型的空 NbtList 实例
///
/// - `nbt_num`: 元素类型序号，取值 0–12
///
/// # 返回值
///
/// 新建的 `NbtList` 实例
pub fn list(nbt_num: u8) -> NbtList {
    NbtList::new(nbt_num)
}

/// 创建空的 NbtCompound 实例
///
/// # 返回值
///
/// 新建的 `NbtCompound` 实例
pub fn compound() -> NbtCompound {
    NbtCompound::new()
}

/// 创建 NbtIntArray 实例
///
/// - `data`: 32 位整数数组数据
///
/// # 返回值
///
/// 新建的 `NbtIntArray` 实例
pub fn int_array(data: Vec<i32>) -> NbtIntArray {
    NbtIntArray::new(data)
}

/// 创建 NbtLongArray 实例
///
/// - `data`: 64 位整数数组数据
///
/// # 返回值
///
/// 新建的 `NbtLongArray` 实例
pub fn long_array(data: Vec<i64>) -> NbtLongArray {
    NbtLongArray::new(data)
}
