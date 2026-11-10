use crate::nbt_types::*;
use crate::*;

impl NbtType {
    /// 获取 End 标签的引用
    ///
    /// # 返回值
    ///
    /// 是 End 类型时返回该引用，否则返回 `None`
    pub fn as_end(&self) -> Option<&NbtEnd> {
        if let NbtType::End(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 End 标签的可变引用
    ///
    /// # 返回值
    ///
    /// 是 End 类型时返回该可变引用，否则返回 `None`
    pub fn as_end_mut(&mut self) -> Option<&mut NbtEnd> {
        if let NbtType::End(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Byte 标签的引用
    ///
    /// # 返回值
    ///
    /// 是 Byte 类型时返回该引用，否则返回 `None`
    pub fn as_byte(&self) -> Option<&NbtByte> {
        if let NbtType::Byte(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Byte 标签的可变引用
    ///
    /// # 返回值
    ///
    /// 是 Byte 类型时返回该可变引用，否则返回 `None`
    pub fn as_byte_mut(&mut self) -> Option<&mut NbtByte> {
        if let NbtType::Byte(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Short 标签的引用
    ///
    /// # 返回值
    ///
    /// 是 Short 类型时返回该引用，否则返回 `None`
    pub fn as_short(&self) -> Option<&NbtShort> {
        if let NbtType::Short(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Short 标签的可变引用
    ///
    /// # 返回值
    ///
    /// 是 Short 类型时返回该可变引用，否则返回 `None`
    pub fn as_short_mut(&mut self) -> Option<&mut NbtShort> {
        if let NbtType::Short(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Int 标签的引用
    ///
    /// # 返回值
    ///
    /// 是 Int 类型时返回该引用，否则返回 `None`
    pub fn as_int(&self) -> Option<&NbtInt> {
        if let NbtType::Int(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Int 标签的可变引用
    ///
    /// # 返回值
    ///
    /// 是 Int 类型时返回该可变引用，否则返回 `None`
    pub fn as_int_mut(&mut self) -> Option<&mut NbtInt> {
        if let NbtType::Int(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Long 标签的引用
    ///
    /// # 返回值
    ///
    /// 是 Long 类型时返回该引用，否则返回 `None`
    pub fn as_long(&self) -> Option<&NbtLong> {
        if let NbtType::Long(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Long 标签的可变引用
    ///
    /// # 返回值
    ///
    /// 是 Long 类型时返回该可变引用，否则返回 `None`
    pub fn as_long_mut(&mut self) -> Option<&mut NbtLong> {
        if let NbtType::Long(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Float 标签的引用
    ///
    /// # 返回值
    ///
    /// 是 Float 类型时返回该引用，否则返回 `None`
    pub fn as_float(&self) -> Option<&NbtFloat> {
        if let NbtType::Float(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Float 标签的可变引用
    ///
    /// # 返回值
    ///
    /// 是 Float 类型时返回该可变引用，否则返回 `None`
    pub fn as_float_mut(&mut self) -> Option<&mut NbtFloat> {
        if let NbtType::Float(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Double 标签的引用
    ///
    /// # 返回值
    ///
    /// 是 Double 类型时返回该引用，否则返回 `None`
    pub fn as_double(&self) -> Option<&NbtDouble> {
        if let NbtType::Double(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Double 标签的可变引用
    ///
    /// # 返回值
    ///
    /// 是 Double 类型时返回该可变引用，否则返回 `None`
    pub fn as_double_mut(&mut self) -> Option<&mut NbtDouble> {
        if let NbtType::Double(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 ByteArray 标签的引用
    ///
    /// # 返回值
    ///
    /// 是 ByteArray 类型时返回该引用，否则返回 `None`
    pub fn as_byte_array(&self) -> Option<&NbtByteArray> {
        if let NbtType::ByteArray(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 ByteArray 标签的可变引用
    ///
    /// # 返回值
    ///
    /// 是 ByteArray 类型时返回该可变引用，否则返回 `None`
    pub fn as_byte_array_mut(&mut self) -> Option<&mut NbtByteArray> {
        if let NbtType::ByteArray(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 String 标签的引用
    ///
    /// # 返回值
    ///
    /// 是 String 类型时返回该引用，否则返回 `None`
    pub fn as_string(&self) -> Option<&NbtString> {
        if let NbtType::String(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 String 标签的可变引用
    ///
    /// # 返回值
    ///
    /// 是 String 类型时返回该可变引用，否则返回 `None`
    pub fn as_string_mut(&mut self) -> Option<&mut NbtString> {
        if let NbtType::String(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 List 标签的引用
    ///
    /// # 返回值
    ///
    /// 是 List 类型时返回该引用，否则返回 `None`
    pub fn as_list(&self) -> Option<&NbtList> {
        if let NbtType::List(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 List 标签的可变引用
    ///
    /// # 返回值
    ///
    /// 是 List 类型时返回该可变引用，否则返回 `None`
    pub fn as_list_mut(&mut self) -> Option<&mut NbtList> {
        if let NbtType::List(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Compound 标签的引用
    ///
    /// # 返回值
    ///
    /// 是 Compound 类型时返回该引用，否则返回 `None`
    pub fn as_compound(&self) -> Option<&NbtCompound> {
        if let NbtType::Compound(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Compound 标签的可变引用
    ///
    /// # 返回值
    ///
    /// 是 Compound 类型时返回该可变引用，否则返回 `None`
    pub fn as_compound_mut(&mut self) -> Option<&mut NbtCompound> {
        if let NbtType::Compound(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 Compound 标签的所有权
    ///
    /// # 返回值
    ///
    /// 是 Compound 类型时返回它，否则返回 `None`
    pub fn get_compound(self) -> Option<NbtCompound> {
        if let NbtType::Compound(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 IntArray 标签的引用
    ///
    /// # 返回值
    ///
    /// 是 IntArray 类型时返回该引用，否则返回 `None`
    pub fn as_int_array(&self) -> Option<&NbtIntArray> {
        if let NbtType::IntArray(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 IntArray 标签的可变引用
    ///
    /// # 返回值
    ///
    /// 是 IntArray 类型时返回该可变引用，否则返回 `None`
    pub fn as_int_array_mut(&mut self) -> Option<&mut NbtIntArray> {
        if let NbtType::IntArray(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 LongArray 标签的引用
    ///
    /// # 返回值
    ///
    /// 是 LongArray 类型时返回该引用，否则返回 `None`
    pub fn as_long_array(&self) -> Option<&NbtLongArray> {
        if let NbtType::LongArray(nbt) = self {
            Some(nbt)
        } else {
            None
        }
    }

    /// 获取 LongArray 标签的可变引用
    ///
    /// # 返回值
    ///
    /// 是 LongArray 类型时返回该可变引用，否则返回 `None`
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
    ///
    /// # 返回值
    ///
    /// 一致时返回 `true`，否则返回 `false`
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
    ///
    /// # 返回值
    ///
    /// 读取成功返回 `Ok(())`，否则返回相应错误
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
    ///
    /// # 返回值
    ///
    /// 写入成功返回 `Ok(())`，否则返回相应错误
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
