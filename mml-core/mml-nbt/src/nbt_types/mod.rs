//! NBT 标签类型定义模块
//!
//! 本模块定义了 Minecraft NBT 规范中的全部 13 种标签类型。
//! 每种类型都是一个独立的 Rust 结构体，封装了具体的数据和二进制序列化/反序列化逻辑。
//!
//! # 类型列表
//!
//! | 类型 | Rust 结构体 | 对应数据 | 二进制大小 |
//! |------|-----------|---------|-----------|
//! | TAG_End | [`NbtEnd`] | 无数据 | 0 字节 |
//! | TAG_Byte | [`NbtByte`] | `u8` | 1 字节 |
//! | TAG_Short | [`NbtShort`] | `i16` | 2 字节，大端序 |
//! | TAG_Int | [`NbtInt`] | `i32` | 4 字节，大端序 |
//! | TAG_Long | [`NbtLong`] | `i64` | 8 字节，大端序 |
//! | TAG_Float | [`NbtFloat`] | `f32` | 4 字节，IEEE 754 大端序 |
//! | TAG_Double | [`NbtDouble`] | `f64` | 8 字节，IEEE 754 大端序 |
//! | TAG_Byte_Array | [`NbtByteArray`] | `Vec<u8>` | 4 字节长度 + 数据 |
//! | TAG_String | [`NbtString`] | `String` | 2 字节长度 + UTF-8 数据 |
//! | TAG_List | [`NbtList`] | `Vec<NbtType>` | 1 字节元素类型 + 4 字节长度 + 数据 |
//! | TAG_Compound | [`NbtCompound`] | `HashMap<String, NbtType>` | 键值对循环，以 End 结束 |
//! | TAG_Int_Array | [`NbtIntArray`] | `Vec<i32>` | 4 字节长度 + 数据，每元素 4 字节 |
//! | TAG_Long_Array | [`NbtLongArray`] | `Vec<i64>` | 4 字节长度 + 数据，每元素 8 字节 |
//!
//! # 工厂函数
//!
//! 模块底部提供了 `end()`、`byte(data)`、`short(data)` 等便捷构造器函数，
//! 用于快速创建各类型实例。

use std::io::Read;

use mml_names::i18_items::error_type::{CoreResult, ErrorType};

use crate::io_error;

mod array;
mod compound;
mod factory;
mod list;
mod scalar;
mod string;

pub use self::array::{NbtByteArray, NbtIntArray, NbtLongArray};
pub use self::compound::NbtCompound;
pub use self::factory::{
    byte, byte_array, compound, double, end, float, int, int_array, list, long, long_array, short,
    string,
};
pub use self::list::NbtList;
pub use self::scalar::{NbtByte, NbtDouble, NbtEnd, NbtFloat, NbtInt, NbtLong, NbtShort};
pub use self::string::NbtString;
pub(crate) use self::string::decode_string;

/// 校验数组长度，并按实际读到的数据增长内存，避免伪造长度触发巨额分配。
fn read_array_bytes<R: Read>(stream: &mut R, count: i32, width: usize) -> CoreResult<Vec<u8>> {
    let len = usize::try_from(count)
        .ok()
        .and_then(|count| count.checked_mul(width))
        .filter(|len| *len <= isize::MAX as usize)
        .ok_or(ErrorType::NbtReadError)?;
    let mut data = Vec::new();
    let mut chunk = [0u8; 8192];
    while data.len() < len {
        let size = (len - data.len()).min(chunk.len());
        stream.read_exact(&mut chunk[..size]).map_err(io_error)?;
        data.try_reserve(size)
            .map_err(|_| ErrorType::NbtReadError)?;
        data.extend_from_slice(&chunk[..size]);
    }
    Ok(data)
}

#[cfg(test)]
#[path = "../../tests/unit/nbt_types_length_tests.rs"]
mod length_tests;
