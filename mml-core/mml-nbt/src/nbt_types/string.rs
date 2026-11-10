//! 字符串标签
//!
//! 长度前缀加 CESU-8 文本，读入时兼容标准 UTF-8。

use std::io::{Read, Write};

use crate::{NbtStream, NbtType, io_error};
use mml_names::i18_items::error_type::{CoreResult, ErrorType};

/// NBT 字符串类型，对应 TAG_String
///
/// 二进制流格式为 2 字节长度，大端序 u16，后跟 UTF-8 字节序列。
/// 长度字段记录的是**字节数**，不是字符数。
#[derive(Default)]
pub struct NbtString {
    /// 字符串数据，按 UTF-8 编码写出
    pub data: String,
}

impl NbtString {
    /// 创建 String 标签
    ///
    /// - `data`: 字符串数据
    ///
    /// # 返回值
    ///
    /// 新建的 `NbtString` 实例
    pub fn new(data: String) -> Self {
        Self { data }
    }

    /// 判断另一个 NBT 标签是否为 String 类型且值相等
    ///
    /// - `nbt`: 待比较的 NBT 标签
    ///
    /// # 返回值
    ///
    /// 一致时返回 `true`，否则返回 `false`
    pub fn eq(&self, nbt: &NbtType) -> bool {
        match nbt {
            NbtType::String(nbt) => nbt.data == self.data,
            _ => false,
        }
    }

    /// 将自身转换为 `NbtType` 枚举变体
    ///
    /// # 返回值
    ///
    /// 对应的 `NbtType` 变体
    pub fn to_nbt(self) -> NbtType {
        NbtType::String(self)
    }
}

/// 解码 NBT 里的字符串字节
///
/// 标准 UTF-8 直接收；不合法时再按 **CESU-8** 试一次：Java 写的 NBT 会把 UTF-16 代理对
/// 拆成两个 3 字节序列（`ED A0 BE ED BA 93` 之类），单看每个序列是合法编码单元、整体却
/// 不是合法 UTF-8，`from_utf8` 一律拒绝。按代理对合回去才是作者想要的那个字符。
/// 仍解不出来就按替换符处理，不因为一个字符串让整棵树读失败。
///
/// - `bytes`: 字符串原始字节
///
/// # 返回值
///
/// 解码后的字符串
pub(crate) fn decode_string(bytes: Vec<u8>) -> String {
    match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(err) => {
            let bytes = err.into_bytes();
            match decode_cesu8(&bytes) {
                Some(text) => text,
                None => String::from_utf8_lossy(&bytes).into_owned(),
            }
        }
    }
}

/// 把 CESU-8 解成字符串，遇到非法序列返回 `None`
fn decode_cesu8(bytes: &[u8]) -> Option<String> {
    let mut units: Vec<u16> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let head = bytes[i];
        let (unit, size) = if head < 0x80 {
            (u16::from(head), 1)
        } else if head & 0xE0 == 0xC0 {
            (decode_unit_2(bytes, i)?, 2)
        } else if head & 0xF0 == 0xE0 {
            (decode_unit_3(bytes, i)?, 3)
        } else {
            return None;
        };
        units.push(unit);
        i += size;
    }
    // 落单的代理项这里会失败，交给调用方退回替换符
    String::from_utf16(&units).ok()
}

/// 解码 2 字节的 UTF-8 序列，越界返回 `None`
fn decode_unit_2(bytes: &[u8], i: usize) -> Option<u16> {
    let tail = *bytes.get(i + 1)?;
    Some((u16::from(bytes[i] & 0x1F) << 6) | u16::from(tail & 0x3F))
}

/// 解码 3 字节的 UTF-8 序列，越界返回 `None`
fn decode_unit_3(bytes: &[u8], i: usize) -> Option<u16> {
    let mid = *bytes.get(i + 1)?;
    let tail = *bytes.get(i + 2)?;
    Some((u16::from(bytes[i] & 0x0F) << 12) | (u16::from(mid & 0x3F) << 6) | u16::from(tail & 0x3F))
}

impl NbtStream for NbtString {
    /// 先读 2 字节长度，再读对应数量的 UTF-8 字节并解码
    fn read<R: Read>(&mut self, stream: &mut R) -> CoreResult<()> {
        let mut temp = [0u8; 2];
        stream.read_exact(&mut temp).map_err(|err| io_error(err))?;

        let len = u16::from_be_bytes(temp);

        let mut temp = vec![0; len as usize];
        stream.read_exact(&mut temp).map_err(|err| io_error(err))?;

        self.data = decode_string(temp);

        Ok(())
    }

    /// 先写 2 字节的字节数，再写 UTF-8 字节
    fn write<W: Write>(&self, stream: &mut W) -> CoreResult<()> {
        let len = u16::try_from(self.data.len())
            .map_err(|_| ErrorType::NbtStringTooLong(self.data.len()))?;
        let temp = len.to_be_bytes();
        stream.write_all(&temp).map_err(|err| io_error(err))?;
        stream
            .write_all(&self.data.as_bytes())
            .map_err(|err| io_error(err))?;

        Ok(())
    }
}
