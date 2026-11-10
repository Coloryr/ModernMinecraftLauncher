use std::fmt;

use mml_names::i18_items::error_type::{ErrorData, ErrorType};

use crate::*;

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
/// - `nbt_type`: 待校验的 NBT 标签类型序号，合法范围 0 到 12，含两端
///
/// # 返回值
///
/// 在合法范围内返回 `true`
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
    ErrorType::StreamError(ErrorData::from_error(e))
}
