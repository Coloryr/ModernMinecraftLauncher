//! NBT 读写模块，NBT 即 Named Binary Tag
//!
//! # 子模块
//!
//! | 模块 | 用途 |
//! |------|------|
//! | [`chunk`] | 区块数据的读写 |
//! | [`nbt_file`] | NBT 文件的整体读写 |
//! | [`nbt_types`] | NBT 各类型标签的数据结构定义 |

use std::fmt;
use std::io::{Read, Write};

use mml_names::i18_items::error_type::{CoreResult, ErrorData, ErrorType};

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

impl NbtType {
    /// 根据类型序号创建对应的默认 NBT 标签实例
    ///
    /// 序号 0 对应 `TAG_End`，1 对应 `TAG_Byte`，以此类推，超出 12 一律视为非法。
    ///
    /// - `nbt_type`: NBT 标签类型序号，取值 0–12
    ///
    /// # 返回值
    ///
    /// 序号有效时返回该类型默认值的实例，无效时返回 `None`
    pub fn get_nbt(nbt_type: u8) -> Option<NbtType> {
        if nbt_type > 12 {
            None
        } else {
            Some(match nbt_type {
                NBT_BYTE_ORDER => Self::byte(),
                NBT_SHORT_ORDER => Self::short(),
                NBT_INT_ORDER => Self::int(),
                NBT_LONG_ORDER => Self::long(),
                NBT_FLOAT_ORDER => Self::float(),
                NBT_DOUBLE_ORDER => Self::double(),
                NBT_BYTE_ARRAY_ORDER => Self::byte_array(),
                NBT_STRING_ORDER => Self::string(),
                NBT_LIST_ORDER => Self::list(),
                NBT_COMPOUND_ORDER => Self::compound(),
                NBT_INT_ARRAY_ORDER => Self::int_array(),
                NBT_LONG_ARRAY_ORDER => Self::long_array(),
                _ => Self::end(),
            })
        }
    }

    /// 创建 End 类型 NBT 标签
    pub fn end() -> NbtType {
        NbtType::End(Default::default())
    }

    /// 创建 Byte 类型 NBT 标签
    pub fn byte() -> NbtType {
        NbtType::Byte(Default::default())
    }

    /// 创建 Short 类型 NBT 标签
    pub fn short() -> NbtType {
        NbtType::Short(Default::default())
    }

    /// 创建 Int 类型 NBT 标签
    pub fn int() -> NbtType {
        NbtType::Int(Default::default())
    }

    /// 创建 Long 类型 NBT 标签
    pub fn long() -> NbtType {
        NbtType::Long(Default::default())
    }

    /// 创建 Float 类型 NBT 标签
    pub fn float() -> NbtType {
        NbtType::Float(Default::default())
    }

    /// 创建 Double 类型 NBT 标签
    pub fn double() -> NbtType {
        NbtType::Double(Default::default())
    }

    /// 创建 ByteArray 类型 NBT 标签
    pub fn byte_array() -> NbtType {
        NbtType::ByteArray(Default::default())
    }

    /// 创建 String 类型 NBT 标签
    pub fn string() -> NbtType {
        NbtType::String(Default::default())
    }

    /// 创建 List 类型 NBT 标签
    pub fn list() -> NbtType {
        NbtType::List(Default::default())
    }

    /// 创建 Compound 类型 NBT 标签
    pub fn compound() -> NbtType {
        NbtType::Compound(Default::default())
    }

    /// 创建 IntArray 类型 NBT 标签
    pub fn int_array() -> NbtType {
        NbtType::IntArray(Default::default())
    }

    /// 创建 LongArray 类型 NBT 标签
    pub fn long_array() -> NbtType {
        NbtType::LongArray(Default::default())
    }

    /// 从NBT标签读对应的数字序号
    pub fn get_num(&self) -> u8 {
        match self {
            NbtType::End(_) => NBT_END_ORDER,
            NbtType::Byte(_) => NBT_BYTE_ORDER,
            NbtType::Short(_) => NBT_SHORT_ORDER,
            NbtType::Int(_) => NBT_INT_ORDER,
            NbtType::Long(_) => NBT_LONG_ORDER,
            NbtType::Float(_) => NBT_FLOAT_ORDER,
            NbtType::Double(_) => NBT_DOUBLE_ORDER,
            NbtType::ByteArray(_) => NBT_BYTE_ARRAY_ORDER,
            NbtType::String(_) => NBT_STRING_ORDER,
            NbtType::List(_) => NBT_LIST_ORDER,
            NbtType::Compound(_) => NBT_COMPOUND_ORDER,
            NbtType::IntArray(_) => NBT_INT_ARRAY_ORDER,
            NbtType::LongArray(_) => NBT_LONG_ARRAY_ORDER,
        }
    }

    /// 获取 End 标签的引用
    pub fn as_end(&self) -> Option<&NbtEnd> {
        if let NbtType::End(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 End 标签的可变引用
    pub fn as_end_mut(&mut self) -> Option<&mut NbtEnd> {
        if let NbtType::End(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Byte 标签的引用
    pub fn as_byte(&self) -> Option<&NbtByte> {
        if let NbtType::Byte(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Byte 标签的可变引用
    pub fn as_byte_mut(&mut self) -> Option<&mut NbtByte> {
        if let NbtType::Byte(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Short 标签的引用
    pub fn as_short(&self) -> Option<&NbtShort> {
        if let NbtType::Short(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Short 标签的可变引用
    pub fn as_short_mut(&mut self) -> Option<&mut NbtShort> {
        if let NbtType::Short(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Int 标签的引用
    pub fn as_int(&self) -> Option<&NbtInt> {
        if let NbtType::Int(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Int 标签的可变引用
    pub fn as_int_mut(&mut self) -> Option<&mut NbtInt> {
        if let NbtType::Int(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Long 标签的引用
    pub fn as_long(&self) -> Option<&NbtLong> {
        if let NbtType::Long(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Long 标签的可变引用
    pub fn as_long_mut(&mut self) -> Option<&mut NbtLong> {
        if let NbtType::Long(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Float 标签的引用
    pub fn as_float(&self) -> Option<&NbtFloat> {
        if let NbtType::Float(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Float 标签的可变引用
    pub fn as_float_mut(&mut self) -> Option<&mut NbtFloat> {
        if let NbtType::Float(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Double 标签的引用
    pub fn as_double(&self) -> Option<&NbtDouble> {
        if let NbtType::Double(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Double 标签的可变引用
    pub fn as_double_mut(&mut self) -> Option<&mut NbtDouble> {
        if let NbtType::Double(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 ByteArray 标签的引用
    pub fn as_byte_array(&self) -> Option<&NbtByteArray> {
        if let NbtType::ByteArray(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 ByteArray 标签的可变引用
    pub fn as_byte_array_mut(&mut self) -> Option<&mut NbtByteArray> {
        if let NbtType::ByteArray(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 String 标签的引用
    pub fn as_string(&self) -> Option<&NbtString> {
        if let NbtType::String(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 String 标签的可变引用
    pub fn as_string_mut(&mut self) -> Option<&mut NbtString> {
        if let NbtType::String(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 List 标签的引用
    pub fn as_list(&self) -> Option<&NbtList> {
        if let NbtType::List(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 List 标签的可变引用
    pub fn as_list_mut(&mut self) -> Option<&mut NbtList> {
        if let NbtType::List(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Compound 标签的引用
    pub fn as_compound(&self) -> Option<&NbtCompound> {
        if let NbtType::Compound(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Compound 标签的可变引用
    pub fn as_compound_mut(&mut self) -> Option<&mut NbtCompound> {
        if let NbtType::Compound(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Compound 标签的所有权
    pub fn get_compound(self) -> Option<NbtCompound> {
        if let NbtType::Compound(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 IntArray 标签的引用
    pub fn as_int_array(&self) -> Option<&NbtIntArray> {
        if let NbtType::IntArray(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 IntArray 标签的可变引用
    pub fn as_int_array_mut(&mut self) -> Option<&mut NbtIntArray> {
        if let NbtType::IntArray(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 LongArray 标签的引用
    pub fn as_long_array(&self) -> Option<&NbtLongArray> {
        if let NbtType::LongArray(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 LongArray 标签的可变引用
    pub fn as_long_array_mut(&mut self) -> Option<&mut NbtLongArray> {
        if let NbtType::LongArray(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 判断两个 NBT 标签是否类型与内容都一致
    ///
    /// - `nbt`: 待比较的另一个标签
    pub fn eq(&self, nbt: &NbtType) -> bool {
        match self {
            NbtType::End(nbt_end) => nbt_end.eq(nbt),
            NbtType::Byte(nbt_byte) => nbt_byte.eq(nbt),
            NbtType::Short(nbt_short) => nbt_short.eq(nbt),
            NbtType::Int(nbt_int) => nbt_int.eq(nbt),
            NbtType::Long(nbt_long) => nbt_long.eq(nbt),
            NbtType::Float(nbt_float) => nbt_float.eq(nbt),
            NbtType::Double(nbt_double) => nbt_double.eq(nbt),
            NbtType::ByteArray(nbt_byte_array) => nbt_byte_array.eq(nbt),
            NbtType::String(nbt_string) => nbt_string.eq(nbt),
            NbtType::List(nbt_list) => nbt_list.eq(nbt),
            NbtType::Compound(nbt_compound) => nbt_compound.eq(nbt),
            NbtType::IntArray(nbt_int_array) => nbt_int_array.eq(nbt),
            NbtType::LongArray(nbt_long_array) => nbt_long_array.eq(nbt),
        }
    }

    /// 按变体分派到具体标签类型的读取实现
    ///
    /// - `stream`: 数据来源流
    pub(crate) fn read<R: Read>(&mut self, stream: &mut R) -> CoreResult<()> {
        match self {
            NbtType::End(nbt_end) => nbt_end.read(stream),
            NbtType::Byte(nbt_byte) => nbt_byte.read(stream),
            NbtType::Short(nbt_short) => nbt_short.read(stream),
            NbtType::Int(nbt_int) => nbt_int.read(stream),
            NbtType::Long(nbt_long) => nbt_long.read(stream),
            NbtType::Float(nbt_float) => nbt_float.read(stream),
            NbtType::Double(nbt_double) => nbt_double.read(stream),
            NbtType::ByteArray(nbt_byte_array) => nbt_byte_array.read(stream),
            NbtType::String(nbt_string) => nbt_string.read(stream),
            NbtType::List(nbt_list) => nbt_list.read(stream),
            NbtType::Compound(nbt_compound) => nbt_compound.read(stream),
            NbtType::IntArray(nbt_int_array) => nbt_int_array.read(stream),
            NbtType::LongArray(nbt_long_array) => nbt_long_array.read(stream),
        }?;

        Ok(())
    }

    /// 按变体分派到具体标签类型的写入实现
    ///
    /// - `stream`: 目标流
    pub(crate) fn write<W: Write>(&self, stream: &mut W) -> CoreResult<()> {
        match self {
            NbtType::End(nbt_end) => nbt_end.write(stream),
            NbtType::Byte(nbt_byte) => nbt_byte.write(stream),
            NbtType::Short(nbt_short) => nbt_short.write(stream),
            NbtType::Int(nbt_int) => nbt_int.write(stream),
            NbtType::Long(nbt_long) => nbt_long.write(stream),
            NbtType::Float(nbt_float) => nbt_float.write(stream),
            NbtType::Double(nbt_double) => nbt_double.write(stream),
            NbtType::ByteArray(nbt_byte_array) => nbt_byte_array.write(stream),
            NbtType::String(nbt_string) => nbt_string.write(stream),
            NbtType::List(nbt_list) => nbt_list.write(stream),
            NbtType::Compound(nbt_compound) => nbt_compound.write(stream),
            NbtType::IntArray(nbt_int_array) => nbt_int_array.write(stream),
            NbtType::LongArray(nbt_long_array) => nbt_long_array.write(stream),
        }?;

        Ok(())
    }
}

/// 按 Minecraft SNBT 格式输出标签内容
///
/// 数值带类型后缀，如 `1b`、`2s`、`3L`、`4.0f`、`5.0d`；字符串加双引号并转义；
/// 数组写成 `[B;...]`、`[I;...]`、`[L;...]`；Compound 写成 `{key: value, ...}`。
/// 仅供调试与日志使用，输出不保证能重新解析回 NBT。
impl fmt::Display for NbtType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NbtType::End(_) => write!(f, "END"),
            NbtType::Byte(nbt) => write!(f, "{}b", nbt.data),
            NbtType::Short(nbt) => write!(f, "{}s", nbt.data),
            NbtType::Int(nbt) => write!(f, "{}", nbt.data),
            NbtType::Long(nbt) => write!(f, "{}L", nbt.data),
            NbtType::Float(nbt) => write!(f, "{}f", nbt.data),
            NbtType::Double(nbt) => write!(f, "{}d", nbt.data),
            NbtType::ByteArray(nbt) => {
                write!(f, "[B;")?;
                for (i, b) in nbt.data.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}B", b)?;
                }
                write!(f, "]")
            }
            NbtType::String(nbt) => write!(f, "\"{}\"", nbt.data.escape_default()),
            NbtType::List(nbt) => {
                write!(f, "[")?;
                for (i, item) in nbt.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            NbtType::Compound(nbt) => {
                write!(f, "{{")?;
                let mut first = true;
                for (key, value) in &nbt.data {
                    if !first {
                        write!(f, ", ")?;
                    }
                    first = false;
                    write!(f, "{}: {}", key, value)?;
                }
                write!(f, "}}")
            }
            NbtType::IntArray(nbt) => {
                write!(f, "[I;")?;
                for (i, v) in nbt.data.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            NbtType::LongArray(nbt) => {
                write!(f, "[L;")?;
                for (i, v) in nbt.data.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}L", v)?;
                }
                write!(f, "]")
            }
        }
    }
}

/// 判断给定的序号是否为合法的 NBT 标签类型 ID
///
/// - `nbt_type`: 待校验的 NBT 标签类型序号
///
/// # 返回值
///
/// 合法范围是 0 到 12，含两端
pub fn is_nbt_num(nbt_type: u8) -> bool {
    nbt_type >= NBT_END_ORDER && nbt_type <= NBT_LONG_ARRAY_ORDER
}

/// 把标准库 IO 错误转换成项目统一的错误类型
///
/// 只保留错误的文本描述，NBT 读写全程靠它统一报错。
///
/// - `e`: 来自标准库的 IO 错误
///
/// # 返回值
///
/// 携带原始错误描述的 `ErrorType::StreamError`
pub(crate) fn io_error(e: std::io::Error) -> ErrorType {
    ErrorType::StreamError(ErrorData {
        error: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
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
}
