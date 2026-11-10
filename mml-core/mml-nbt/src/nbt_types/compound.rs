//! 复合标签
//!
//! 键值对循环，以 End 结束；保持插入顺序。

use std::{
    collections::HashMap,
    io::{Read, Write},
};

use mml_names::i18_items::error_type::{CoreResult, ErrorType};

use super::array::{NbtByteArray, NbtLongArray};
use super::list::NbtList;
use super::string::decode_string;
use crate::{NbtStream, NbtType, io_error};

/// NBT 复合标签类型，对应 TAG_Compound
///
/// 存储一组键值对，键为字符串，值为任意类型的 NBT 标签；二进制流中循环读取
/// "类型序号 + 键名 + 数据"，遇到类型序号 0 的 TAG_End 即结束。
/// 这是 Minecraft NBT 中最核心的结构类型，区块数据、物品标签都靠它嵌套。
///
/// # 示例
///
/// ```ignore
/// let mut compound = NbtCompound::new();
/// // 通过调用方手动插入键值对
/// compound.data.insert("Level".to_string(), NbtType::int());
/// ```
#[derive(Default)]
pub struct NbtCompound {
    /// 键值对数据；HashMap 不保证顺序，写出顺序与插入顺序无关
    pub data: HashMap<String, NbtType>,
}

impl NbtCompound {
    /// 创建空的 Compound 标签
    ///
    /// # 返回值
    ///
    /// 新建的空 `NbtCompound` 实例
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// 判断另一个 NBT 标签是否为 Compound 类型且所有键值对一致
    ///
    /// - `nbt`: 待比较的 NBT 标签
    ///
    /// # 返回值
    ///
    /// 一致时返回 `true`，否则返回 `false`
    pub fn eq(&self, nbt: &NbtType) -> bool {
        match nbt {
            NbtType::Compound(nbt) => {
                if self.data.len() != nbt.data.len() {
                    return false;
                }

                for (index, item1) in self.data.iter() {
                    let item2 = nbt.data.get(index);
                    if item2.is_none() {
                        return false;
                    }
                    let item2 = item2.unwrap();

                    if !item1.eq(item2) {
                        return false;
                    }
                }

                return true;
            }
            _ => false,
        }
    }

    /// 获取指定键对应的 NBT 标签的不可变引用
    ///
    /// - `key`: 键名
    ///
    /// # 返回值
    ///
    /// 键存在时返回其引用，否则返回 `None`
    pub fn get(&self, key: &str) -> Option<&NbtType> {
        self.data.get(key)
    }

    /// 获取指定键对应的 NBT 标签的可变引用
    ///
    /// - `key`: 键名
    ///
    /// # 返回值
    ///
    /// 键存在时返回其可变引用，否则返回 `None`
    pub fn get_mut(&mut self, key: &str) -> Option<&mut NbtType> {
        self.data.get_mut(key)
    }

    /// 从 Compound 中提取 `&NbtByteArray`，自动进行类型匹配
    ///
    /// - `key`: 键名
    ///
    /// # 返回值
    ///
    /// 键存在且是 ByteArray 类型时返回该引用，否则返回 `None`
    pub fn get_byte_array(&self, key: &str) -> Option<&NbtByteArray> {
        match self.get(key) {
            Some(NbtType::ByteArray(v)) => Some(v),
            _ => None,
        }
    }

    /// 从 Compound 中提取 `&mut NbtByteArray`，自动进行类型匹配
    ///
    /// - `key`: 键名
    ///
    /// # 返回值
    ///
    /// 键存在且是 ByteArray 类型时返回该可变引用，否则返回 `None`
    pub fn get_byte_array_mut(&mut self, key: &str) -> Option<&mut NbtByteArray> {
        match self.get_mut(key) {
            Some(NbtType::ByteArray(v)) => Some(v),
            _ => None,
        }
    }

    /// 从 Compound 中提取 `&NbtLongArray`，自动进行类型匹配
    ///
    /// - `key`: 键名
    ///
    /// # 返回值
    ///
    /// 键存在且是 LongArray 类型时返回该引用，否则返回 `None`
    pub fn get_long_array(&self, key: &str) -> Option<&NbtLongArray> {
        match self.get(key) {
            Some(NbtType::LongArray(v)) => Some(v),
            _ => None,
        }
    }

    /// 从 Compound 中提取 `&mut NbtLongArray`，自动进行类型匹配
    ///
    /// - `key`: 键名
    ///
    /// # 返回值
    ///
    /// 键存在且是 LongArray 类型时返回该可变引用，否则返回 `None`
    pub fn get_long_array_mut(&mut self, key: &str) -> Option<&mut NbtLongArray> {
        match self.get_mut(key) {
            Some(NbtType::LongArray(v)) => Some(v),
            _ => None,
        }
    }

    /// 从 Compound 中提取嵌套的 `&NbtCompound`，自动进行类型匹配
    ///
    /// - `key`: 键名
    ///
    /// # 返回值
    ///
    /// 键存在且是 Compound 类型时返回该引用，否则返回 `None`
    pub fn get_compound(&self, key: &str) -> Option<&NbtCompound> {
        match self.get(key) {
            Some(NbtType::Compound(v)) => Some(v),
            _ => None,
        }
    }

    /// 从 Compound 中提取嵌套的 `&mut NbtCompound`，自动进行类型匹配
    ///
    /// - `key`: 键名
    ///
    /// # 返回值
    ///
    /// 键存在且是 Compound 类型时返回该可变引用，否则返回 `None`
    pub fn get_compound_mut(&mut self, key: &str) -> Option<&mut NbtCompound> {
        match self.get_mut(key) {
            Some(NbtType::Compound(v)) => Some(v),
            _ => None,
        }
    }

    /// 从 Compound 中提取 TAG_Long 的 `i64` 值
    ///
    /// - `key`: 键名
    ///
    /// # 返回值
    ///
    /// 键存在且是 Long 类型时返回该值，否则返回 `None`
    pub fn get_long(&self, key: &str) -> Option<i64> {
        match self.get(key) {
            Some(NbtType::Long(v)) => Some(v.data),
            _ => None,
        }
    }

    /// 从 Compound 中提取 TAG_Short 的 `i16` 值
    ///
    /// - `key`: 键名
    ///
    /// # 返回值
    ///
    /// 键存在且是 Short 类型时返回该值，否则返回 `None`
    pub fn get_short(&self, key: &str) -> Option<i16> {
        match self.get(key) {
            Some(NbtType::Short(v)) => Some(v.data),
            _ => None,
        }
    }

    /// 从 Compound 中提取 TAG_Int 的 `i32` 值
    ///
    /// - `key`: 键名
    ///
    /// # 返回值
    ///
    /// 键存在且是 Int 类型时返回该值，否则返回 `None`
    pub fn get_int(&self, key: &str) -> Option<i32> {
        match self.get(key) {
            Some(NbtType::Int(v)) => Some(v.data),
            _ => None,
        }
    }

    /// 从 Compound 中提取 TAG_Byte 的 `u8` 值
    ///
    /// - `key`: 键名
    ///
    /// # 返回值
    ///
    /// 键存在且是 Byte 类型时返回该值，否则返回 `None`
    pub fn get_byte(&self, key: &str) -> Option<u8> {
        match self.get(key) {
            Some(NbtType::Byte(v)) => Some(v.data),
            _ => None,
        }
    }

    /// 从 Compound 中提取 TAG_String 的 `String` 值，克隆返回
    ///
    /// - `key`: 键名
    ///
    /// # 返回值
    ///
    /// 键存在且是 String 类型时返回克隆的值，否则返回 `None`
    pub fn get_string(&self, key: &str) -> Option<String> {
        match self.get(key) {
            Some(NbtType::String(v)) => Some(v.data.clone()),
            _ => None,
        }
    }

    /// 从 Compound 中提取 `&NbtList`，自动进行类型匹配
    ///
    /// - `key`: 键名
    ///
    /// # 返回值
    ///
    /// 键存在且是 List 类型时返回该引用，否则返回 `None`
    pub fn get_list(&self, key: &str) -> Option<&NbtList> {
        match self.get(key) {
            Some(NbtType::List(v)) => Some(v),
            _ => None,
        }
    }

    /// 从 Compound 中提取 `&mut NbtList`，自动进行类型匹配
    ///
    /// - `key`: 键名
    ///
    /// # 返回值
    ///
    /// 键存在且是 List 类型时返回该可变引用，否则返回 `None`
    pub fn get_list_mut(&mut self, key: &str) -> Option<&mut NbtList> {
        match self.get_mut(key) {
            Some(NbtType::List(v)) => Some(v),
            _ => None,
        }
    }

    /// 将自身转换为 `NbtType` 枚举变体
    ///
    /// # 返回值
    ///
    /// 对应的 `NbtType` 变体
    pub fn to_nbt(self) -> NbtType {
        NbtType::Compound(self)
    }
}

impl NbtStream for NbtCompound {
    /// 从流中逐条读取键值对，直到遇到类型序号 0 的 TAG_End
    ///
    /// 每条键值对的格式：1 字节类型序号 → 2 字节键名长度 → 键名字符串 → 值数据
    fn read<R: Read>(&mut self, stream: &mut R) -> CoreResult<()> {
        loop {
            let mut temp = [0u8; 1];
            stream.read_exact(&mut temp).map_err(|err| io_error(err))?;

            if temp[0] == 0 {
                return Ok(());
            }

            let nbt = NbtType::get_nbt(temp[0]);
            if nbt.is_none() {
                return Err(ErrorType::NbtTypeError);
            }

            let mut temp = [0u8; 2];
            stream.read_exact(&mut temp).map_err(|err| io_error(err))?;

            let len = u16::from_be_bytes(temp);

            let mut temp = vec![0; len as usize];
            stream.read_exact(&mut temp).map_err(|err| io_error(err))?;

            let key = decode_string(temp);

            let mut nbt = nbt.unwrap();
            nbt.read(stream)?;

            self.data.insert(key, nbt);
        }
    }

    /// 将全部键值对写入流，末尾附加 0x00 作为终止标记
    fn write<W: Write>(&self, stream: &mut W) -> CoreResult<()> {
        for (key, nbt) in &self.data {
            let temp = [nbt.get_num()];
            stream.write_all(&temp).map_err(|err| io_error(err))?;

            // End 变体只占 1 字节类型序号，后面没有键名和值
            if !matches!(nbt, NbtType::End(_)) {
                let len =
                    u16::try_from(key.len()).map_err(|_| ErrorType::NbtStringTooLong(key.len()))?;
                let temp = len.to_be_bytes();
                stream.write_all(&temp).map_err(|err| io_error(err))?;
                stream
                    .write_all(key.as_bytes())
                    .map_err(|err| io_error(err))?;

                nbt.write(stream)?;
            }
        }

        let temp = [0];
        stream.write_all(&temp).map_err(|err| io_error(err))?;

        Ok(())
    }
}
