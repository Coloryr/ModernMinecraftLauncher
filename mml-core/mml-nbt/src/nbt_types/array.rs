//! 数组标签
//!
//! Byte / Int / Long 三种定宽数组，长度前缀加元素数据。

use std::io::{Read, Write};

use mml_names::i18_items::error_type::CoreResult;

use super::read_array_bytes;
use crate::{NbtStream, NbtType, io_error};

/// NBT 字节数组类型，对应 TAG_Byte_Array
///
/// 二进制流格式为 4 字节元素个数，大端序 i32，后跟原始字节数据。
#[derive(Default)]
pub struct NbtByteArray {
    /// 字节数组数据，元素按无符号字节存取
    pub data: Vec<u8>,
}

impl NbtByteArray {
    /// 创建 ByteArray 标签
    ///
    /// - `data`: 字节数组数据
    ///
    /// # 返回值
    ///
    /// 新建的 `NbtByteArray` 实例
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// 判断另一个 NBT 标签是否为 ByteArray 类型且数据相等
    ///
    /// - `nbt`: 待比较的 NBT 标签
    ///
    /// # 返回值
    ///
    /// 一致时返回 `true`，否则返回 `false`
    pub fn eq(&self, nbt: &NbtType) -> bool {
        match nbt {
            NbtType::ByteArray(nbt) => nbt.data == self.data,
            _ => false,
        }
    }

    /// 将自身转换为 `NbtType` 枚举变体
    ///
    /// # 返回值
    ///
    /// 对应的 `NbtType` 变体
    pub fn to_nbt(self) -> NbtType {
        NbtType::ByteArray(self)
    }
}

impl NbtStream for NbtByteArray {
    /// 先读 4 字节元素个数，再读对应数量的字节
    fn read<R: Read>(&mut self, stream: &mut R) -> CoreResult<()> {
        let mut temp = [0u8; 4];
        stream.read_exact(&mut temp).map_err(|err| io_error(err))?;

        self.data = read_array_bytes(stream, i32::from_be_bytes(temp), 1)?;

        Ok(())
    }

    /// 先写 4 字节元素个数，再写字节数据
    fn write<W: Write>(&self, stream: &mut W) -> CoreResult<()> {
        let temp = i32::to_be_bytes(self.data.len() as i32);
        stream.write_all(&temp).map_err(|err| io_error(err))?;
        stream.write_all(&self.data).map_err(|err| io_error(err))?;

        Ok(())
    }
}

/// NBT 整型数组类型，对应 TAG_Int_Array
///
/// 二进制流格式为 4 字节元素个数，大端序 i32，后跟每个元素 4 字节，大端序。
#[derive(Default)]
pub struct NbtIntArray {
    /// 32 位整数数组数据，元素按大端序写出
    pub data: Vec<i32>,
}

impl NbtIntArray {
    /// 创建 IntArray 标签
    ///
    /// - `data`: 32 位整数数组数据
    ///
    /// # 返回值
    ///
    /// 新建的 `NbtIntArray` 实例
    pub fn new(data: Vec<i32>) -> Self {
        Self { data }
    }

    /// 判断另一个 NBT 标签是否为 IntArray 类型且数据相等
    ///
    /// - `nbt`: 待比较的 NBT 标签
    ///
    /// # 返回值
    ///
    /// 一致时返回 `true`，否则返回 `false`
    pub fn eq(&self, nbt: &NbtType) -> bool {
        match nbt {
            NbtType::IntArray(nbt) => nbt.data == self.data,
            _ => false,
        }
    }

    /// 将自身转换为 `NbtType` 枚举变体
    ///
    /// # 返回值
    ///
    /// 对应的 `NbtType` 变体
    pub fn to_nbt(self) -> NbtType {
        NbtType::IntArray(self)
    }
}

impl NbtStream for NbtIntArray {
    /// 先读 4 字节元素个数，再按每元素 4 字节读出
    fn read<R: Read>(&mut self, stream: &mut R) -> CoreResult<()> {
        let mut temp = [0u8; 4];
        stream.read_exact(&mut temp).map_err(|err| io_error(err))?;

        let temp = read_array_bytes(stream, i32::from_be_bytes(temp), 4)?;

        self.data = temp
            .chunks_exact(4)
            .map(|chunk| i32::from_be_bytes(chunk.try_into().unwrap()))
            .collect();

        Ok(())
    }

    /// 先写 4 字节元素个数，再按每元素 4 字节写出
    fn write<W: Write>(&self, stream: &mut W) -> CoreResult<()> {
        let temp = i32::to_be_bytes(self.data.len() as i32);
        stream.write_all(&temp).map_err(|err| io_error(err))?;
        let temp: Vec<u8> = self
            .data
            .iter()
            .flat_map(|&value| value.to_be_bytes())
            .collect();
        stream.write_all(&temp).map_err(|err| io_error(err))?;

        Ok(())
    }
}

/// NBT 长整型数组类型，对应 TAG_Long_Array
///
/// 二进制流格式为 4 字节元素个数，大端序 i32，后跟每个元素 8 字节，大端序。
/// Minecraft 1.12 起引入，区块高度图等大数据结构用它存放。
#[derive(Default)]
pub struct NbtLongArray {
    /// 64 位整数数组数据，元素按大端序写出
    pub data: Vec<i64>,
}

impl NbtLongArray {
    /// 创建 LongArray 标签
    ///
    /// - `data`: 64 位整数数组数据
    ///
    /// # 返回值
    ///
    /// 新建的 `NbtLongArray` 实例
    pub fn new(data: Vec<i64>) -> Self {
        Self { data }
    }

    /// 判断另一个 NBT 标签是否为 LongArray 类型且数据相等
    ///
    /// - `nbt`: 待比较的 NBT 标签
    ///
    /// # 返回值
    ///
    /// 一致时返回 `true`，否则返回 `false`
    pub fn eq(&self, nbt: &NbtType) -> bool {
        match nbt {
            NbtType::LongArray(nbt) => nbt.data == self.data,
            _ => false,
        }
    }

    /// 将自身转换为 `NbtType` 枚举变体
    ///
    /// # 返回值
    ///
    /// 对应的 `NbtType` 变体
    pub fn to_nbt(self) -> NbtType {
        NbtType::LongArray(self)
    }
}

impl NbtStream for NbtLongArray {
    /// 先读 4 字节元素个数，再按每元素 8 字节读出
    fn read<R: Read>(&mut self, stream: &mut R) -> CoreResult<()> {
        let mut temp = [0u8; 4];
        stream.read_exact(&mut temp).map_err(|err| io_error(err))?;

        let temp = read_array_bytes(stream, i32::from_be_bytes(temp), 8)?;

        self.data = temp
            .chunks_exact(8)
            .map(|chunk| i64::from_be_bytes(chunk.try_into().unwrap()))
            .collect();

        Ok(())
    }

    /// 先写 4 字节元素个数，再按每元素 8 字节写出
    fn write<W: Write>(&self, stream: &mut W) -> CoreResult<()> {
        let temp = i32::to_be_bytes(self.data.len() as i32);
        stream.write_all(&temp).map_err(|err| io_error(err))?;
        let temp: Vec<u8> = self
            .data
            .iter()
            .flat_map(|&value| value.to_be_bytes())
            .collect();
        stream.write_all(&temp).map_err(|err| io_error(err))?;

        Ok(())
    }
}
