//! NBT 读写模块，NBT 即 Named Binary Tag
//!
//! # 子模块
//!
//! | 模块 | 用途 |
//! |------|------|
//! | [`chunk`] | 区块数据的读写 |
//! | [`nbt_file`] | NBT 文件的整体读写 |
//! | [`nbt_types`] | NBT 各类型标签的数据结构定义 |

use std::io::{Read, Write};

use mml_names::i18_items::error_type::CoreResult;

use crate::nbt_types::{
    NbtByte, NbtByteArray, NbtCompound, NbtDouble, NbtEnd, NbtFloat, NbtInt, NbtIntArray, NbtList,
    NbtLong, NbtLongArray, NbtShort, NbtString,
};

pub mod chunk;
pub mod nbt_file;
pub mod nbt_types;

/// TAG_End 的类型序号
pub const NBT_END_ORDER: u8 = 0;
/// TAG_Byte 的类型序号
pub const NBT_BYTE_ORDER: u8 = 1;
/// TAG_Short 的类型序号
pub const NBT_SHORT_ORDER: u8 = 2;
/// TAG_Int 的类型序号
pub const NBT_INT_ORDER: u8 = 3;
/// TAG_Long 的类型序号
pub const NBT_LONG_ORDER: u8 = 4;
/// TAG_Float 的类型序号
pub const NBT_FLOAT_ORDER: u8 = 5;
/// TAG_Double 的类型序号
pub const NBT_DOUBLE_ORDER: u8 = 6;
/// TAG_Byte_Array 的类型序号
pub const NBT_BYTE_ARRAY_ORDER: u8 = 7;
/// TAG_String 的类型序号
pub const NBT_STRING_ORDER: u8 = 8;
/// TAG_List 的类型序号
pub const NBT_LIST_ORDER: u8 = 9;
/// TAG_Compound 的类型序号
pub const NBT_COMPOUND_ORDER: u8 = 10;
/// TAG_Int_Array 的类型序号
pub const NBT_INT_ARRAY_ORDER: u8 = 11;
/// TAG_Long_Array 的类型序号
pub const NBT_LONG_ARRAY_ORDER: u8 = 12;

/// NBT 读写流接口
///
/// 定义了 NBT 标签与二进制流之间序列化和反序列化的统一接口。
/// 所有 NBT 标签类型均需实现此 trait，以便通过统一的 `read`/`write`
/// 方法进行二进制读写操作。
pub(crate) trait NbtStream {
    /// 从流中读出一个本类型的标签值并写回自身
    ///
    /// - `stream`: 数据来源流
    fn read<R: Read>(&mut self, stream: &mut R) -> CoreResult<()>;
    /// 把自身的标签值写入流
    ///
    /// - `stream`: 目标流
    fn write<W: Write>(&self, stream: &mut W) -> CoreResult<()>;
}

/// NBT 类型枚举
///
/// 封装了 Minecraft NBT 规范中定义的全部 13 种标签类型。
/// 每种变体对应 Minecraft 数据存储中的一种基本数据类型，用于序列化和
/// 反序列化游戏数据，如区块、玩家存档、物品等。
///
/// # 标签类型对照
///
/// | 变体       | 类型 ID | Minecraft 名称       | 说明             |
/// |-----------|--------|---------------------|-----------------|
/// | `End`     | 0      | TAG_End             | 复合标签结束标记    |
/// | `Byte`    | 1      | TAG_Byte            | 有符号 8 位整数    |
/// | `Short`   | 2      | TAG_Short           | 有符号 16 位整数   |
/// | `Int`     | 3      | TAG_Int             | 有符号 32 位整数   |
/// | `Long`    | 4      | TAG_Long            | 有符号 64 位整数   |
/// | `Float`   | 5      | TAG_Float           | 32 位 IEEE 浮点数 |
/// | `Double`  | 6      | TAG_Double          | 64 位 IEEE 浮点数 |
/// | `ByteArray` | 7    | TAG_Byte_Array      | 字节数组          |
/// | `String`  | 8      | TAG_String          | UTF-8 字符串     |
/// | `List`    | 9      | TAG_List            | 同类型标签列表     |
/// | `Compound`| 10     | TAG_Compound        | 键值对复合结构     |
/// | `IntArray`| 11     | TAG_Int_Array       | 32 位整数数组     |
/// | `LongArray`| 12    | TAG_Long_Array      | 64 位整数数组     |
pub enum NbtType {
    /// TAG_End，类型 ID 0；无数据负载，只作 Compound 键值对的结束标记
    End(NbtEnd),
    /// TAG_Byte，类型 ID 1；8 位整数，占 1 字节，无字节序问题
    Byte(NbtByte),
    /// TAG_Short，类型 ID 2；有符号 16 位整数，大端序，占 2 字节
    Short(NbtShort),
    /// TAG_Int，类型 ID 3；有符号 32 位整数，大端序，占 4 字节
    Int(NbtInt),
    /// TAG_Long，类型 ID 4；有符号 64 位整数，大端序，占 8 字节
    Long(NbtLong),
    /// TAG_Float，类型 ID 5；IEEE 754 单精度浮点，大端序，占 4 字节
    Float(NbtFloat),
    /// TAG_Double，类型 ID 6；IEEE 754 双精度浮点，大端序，占 8 字节
    Double(NbtDouble),
    /// TAG_Byte_Array，类型 ID 7；4 字节元素个数打头，后跟原始字节
    ByteArray(NbtByteArray),
    /// TAG_String，类型 ID 8；2 字节字节数打头，后跟 UTF-8 数据
    String(NbtString),
    /// TAG_List，类型 ID 9；元素类型 ID 加 4 字节元素个数打头，元素类型必须一致
    List(NbtList),
    /// TAG_Compound，类型 ID 10；键值对循环，以 TAG_End 结束
    Compound(NbtCompound),
    /// TAG_Int_Array，类型 ID 11；4 字节元素个数打头，每元素 4 字节大端序
    IntArray(NbtIntArray),
    /// TAG_Long_Array，类型 ID 12；4 字节元素个数打头，每元素 8 字节大端序
    LongArray(NbtLongArray),
}

mod nbt_type_access;
mod nbt_type_ctor;
mod nbt_type_display;

pub(crate) use self::nbt_type_display::{io_error, is_nbt_num};

#[cfg(test)]
#[path = "../tests/unit/lib_tests.rs"]
mod tests;
