use crate::*;

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
    ///
    /// # 返回值
    ///
    /// 新建的 `NbtType::End` 变体
    pub fn end() -> NbtType {
        NbtType::End(Default::default())
    }

    /// 创建 Byte 类型 NBT 标签
    ///
    /// # 返回值
    ///
    /// 新建的 `NbtType::Byte` 变体
    pub fn byte() -> NbtType {
        NbtType::Byte(Default::default())
    }

    /// 创建 Short 类型 NBT 标签
    ///
    /// # 返回值
    ///
    /// 新建的 `NbtType::Short` 变体
    pub fn short() -> NbtType {
        NbtType::Short(Default::default())
    }

    /// 创建 Int 类型 NBT 标签
    ///
    /// # 返回值
    ///
    /// 新建的 `NbtType::Int` 变体
    pub fn int() -> NbtType {
        NbtType::Int(Default::default())
    }

    /// 创建 Long 类型 NBT 标签
    ///
    /// # 返回值
    ///
    /// 新建的 `NbtType::Long` 变体
    pub fn long() -> NbtType {
        NbtType::Long(Default::default())
    }

    /// 创建 Float 类型 NBT 标签
    ///
    /// # 返回值
    ///
    /// 新建的 `NbtType::Float` 变体
    pub fn float() -> NbtType {
        NbtType::Float(Default::default())
    }

    /// 创建 Double 类型 NBT 标签
    ///
    /// # 返回值
    ///
    /// 新建的 `NbtType::Double` 变体
    pub fn double() -> NbtType {
        NbtType::Double(Default::default())
    }

    /// 创建 ByteArray 类型 NBT 标签
    ///
    /// # 返回值
    ///
    /// 新建的 `NbtType::ByteArray` 变体
    pub fn byte_array() -> NbtType {
        NbtType::ByteArray(Default::default())
    }

    /// 创建 String 类型 NBT 标签
    ///
    /// # 返回值
    ///
    /// 新建的 `NbtType::String` 变体
    pub fn string() -> NbtType {
        NbtType::String(Default::default())
    }

    /// 创建 List 类型 NBT 标签
    ///
    /// # 返回值
    ///
    /// 新建的 `NbtType::List` 变体
    pub fn list() -> NbtType {
        NbtType::List(Default::default())
    }

    /// 创建 Compound 类型 NBT 标签
    ///
    /// # 返回值
    ///
    /// 新建的 `NbtType::Compound` 变体
    pub fn compound() -> NbtType {
        NbtType::Compound(Default::default())
    }

    /// 创建 IntArray 类型 NBT 标签
    ///
    /// # 返回值
    ///
    /// 新建的 `NbtType::IntArray` 变体
    pub fn int_array() -> NbtType {
        NbtType::IntArray(Default::default())
    }

    /// 创建 LongArray 类型 NBT 标签
    ///
    /// # 返回值
    ///
    /// 新建的 `NbtType::LongArray` 变体
    pub fn long_array() -> NbtType {
        NbtType::LongArray(Default::default())
    }

    /// 从NBT标签读对应的数字序号
    ///
    /// # 返回值
    ///
    /// 该标签的类型序号，取值 0–12
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
}
