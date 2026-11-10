//! 标量标签
//!
//! End、Byte、Short、Int、Long、Float、Double 七种定长标签。

use std::io::{Read, Write};

use crate::{NbtStream, NbtType, io_error};
use mml_names::i18_items::error_type::CoreResult;

/// NBT 结束标记，对应 TAG_End
///
/// 无数据负载，只用于标识 Compound 标签中键值对列表的结束。
/// 自身不占数据字节，前面那 1 字节类型序号由调用方读写。
pub struct NbtEnd {}

impl Default for NbtEnd {
    fn default() -> Self {
        Self::new()
    }
}

impl NbtEnd {
    /// 创建空的 End 标签
    ///
    /// # 返回值
    ///
    /// 新建的空 `NbtEnd` 实例
    pub fn new() -> Self {
        Self {}
    }

    /// 判断另一个 NBT 标签是否也是 End 类型
    ///
    /// - `nbt`: 待比较的 NBT 标签
    ///
    /// # 返回值
    ///
    /// 一致时返回 `true`，否则返回 `false`
    pub fn eq(&self, nbt: &NbtType) -> bool {
        matches!(nbt, NbtType::End(_))
    }

    /// 将自身转换为 `NbtType` 枚举变体
    ///
    /// # 返回值
    ///
    /// 对应的 `NbtType` 变体
    pub fn to_nbt(self) -> NbtType {
        NbtType::End(self)
    }
}

/// End 标签无任何数据需要读写
impl NbtStream for NbtEnd {
    /// 读取 End 标签：本身无内容，不消费流中任何字节
    fn read<R: Read>(&mut self, _stream: &mut R) -> CoreResult<()> {
        Ok(())
    }

    /// 写入 End 标签：本身无内容，不向流中写出任何字节
    fn write<W: Write>(&self, _stream: &mut W) -> CoreResult<()> {
        Ok(())
    }
}

/// NBT 字节类型，对应 TAG_Byte
///
/// 存储一个 8 位整数，Rust 侧类型为 `u8`，在二进制流中占用 1 字节。
/// 这是 Minecraft NBT 中最小的数值类型。
#[derive(Default)]
pub struct NbtByte {
    /// 字节数据，按无符号 `u8` 存取，取值 0–255
    pub data: u8,
}

impl NbtByte {
    /// 创建 Byte 标签
    ///
    /// - `data`: 字节数据
    ///
    /// # 返回值
    ///
    /// 新建的 `NbtByte` 实例
    pub fn new(data: u8) -> Self {
        Self { data }
    }

    /// 判断另一个 NBT 标签是否为 Byte 类型且值相等
    ///
    /// - `nbt`: 待比较的 NBT 标签
    ///
    /// # 返回值
    ///
    /// 一致时返回 `true`，否则返回 `false`
    pub fn eq(&self, nbt: &NbtType) -> bool {
        match nbt {
            NbtType::Byte(nbt) => nbt.data == self.data,
            _ => false,
        }
    }

    /// 将自身转换为 `NbtType` 枚举变体
    ///
    /// # 返回值
    ///
    /// 对应的 `NbtType` 变体
    pub fn to_nbt(self) -> NbtType {
        NbtType::Byte(self)
    }
}

impl NbtStream for NbtByte {
    /// 从流中读取 1 字节
    fn read<R: Read>(&mut self, stream: &mut R) -> CoreResult<()> {
        let mut temp = [0u8; 1];
        stream.read_exact(&mut temp).map_err(|err| io_error(err))?;

        self.data = temp[0];

        Ok(())
    }

    /// 将 1 字节写入流
    fn write<W: Write>(&self, stream: &mut W) -> CoreResult<()> {
        let temp = [self.data];
        stream.write_all(&temp).map_err(|err| io_error(err))?;

        Ok(())
    }
}

/// NBT 短整型，对应 TAG_Short
///
/// 存储一个有符号 16 位整数，在二进制流中占用 2 字节，大端序。
#[derive(Default)]
pub struct NbtShort {
    /// 短整型数据，取值 -32768 到 32767
    pub data: i16,
}

impl NbtShort {
    /// 创建 Short 标签
    ///
    /// - `data`: 短整型数据
    ///
    /// # 返回值
    ///
    /// 新建的 `NbtShort` 实例
    pub fn new(data: i16) -> Self {
        Self { data }
    }

    /// 判断另一个 NBT 标签是否为 Short 类型且值相等
    ///
    /// - `nbt`: 待比较的 NBT 标签
    ///
    /// # 返回值
    ///
    /// 一致时返回 `true`，否则返回 `false`
    pub fn eq(&self, nbt: &NbtType) -> bool {
        match nbt {
            NbtType::Short(nbt) => nbt.data == self.data,
            _ => false,
        }
    }

    /// 将自身转换为 `NbtType` 枚举变体
    ///
    /// # 返回值
    ///
    /// 对应的 `NbtType` 变体
    pub fn to_nbt(self) -> NbtType {
        NbtType::Short(self)
    }
}

impl NbtStream for NbtShort {
    /// 从流中按大端序读取 2 字节
    fn read<R: Read>(&mut self, stream: &mut R) -> CoreResult<()> {
        let mut temp = [0u8; 2];
        stream.read_exact(&mut temp).map_err(|err| io_error(err))?;

        self.data = i16::from_be_bytes(temp);

        Ok(())
    }

    /// 将 2 字节按大端序写入流
    fn write<W: Write>(&self, stream: &mut W) -> CoreResult<()> {
        let temp = i16::to_be_bytes(self.data);
        stream.write_all(&temp).map_err(|err| io_error(err))?;

        Ok(())
    }
}

/// NBT 整型，对应 TAG_Int
///
/// 存储一个有符号 32 位整数，在二进制流中占用 4 字节，大端序。
#[derive(Default)]
pub struct NbtInt {
    /// 整型数据，取值 -2^31 到 2^31-1
    pub data: i32,
}

impl NbtInt {
    /// 创建 Int 标签
    ///
    /// - `data`: 整型数据
    ///
    /// # 返回值
    ///
    /// 新建的 `NbtInt` 实例
    pub fn new(data: i32) -> Self {
        Self { data }
    }

    /// 判断另一个 NBT 标签是否为 Int 类型且值相等
    ///
    /// - `nbt`: 待比较的 NBT 标签
    ///
    /// # 返回值
    ///
    /// 一致时返回 `true`，否则返回 `false`
    pub fn eq(&self, nbt: &NbtType) -> bool {
        match nbt {
            NbtType::Int(nbt) => nbt.data == self.data,
            _ => false,
        }
    }

    /// 将自身转换为 `NbtType` 枚举变体
    ///
    /// # 返回值
    ///
    /// 对应的 `NbtType` 变体
    pub fn to_nbt(self) -> NbtType {
        NbtType::Int(self)
    }
}

impl NbtStream for NbtInt {
    /// 从流中按大端序读取 4 字节
    fn read<R: Read>(&mut self, stream: &mut R) -> CoreResult<()> {
        let mut temp = [0u8; 4];
        stream.read_exact(&mut temp).map_err(|err| io_error(err))?;

        self.data = i32::from_be_bytes(temp);

        Ok(())
    }

    /// 将 4 字节按大端序写入流
    fn write<W: Write>(&self, stream: &mut W) -> CoreResult<()> {
        let temp = i32::to_be_bytes(self.data);
        stream.write_all(&temp).map_err(|err| io_error(err))?;

        Ok(())
    }
}

/// NBT 长整型，对应 TAG_Long
///
/// 存储一个有符号 64 位整数，在二进制流中占用 8 字节，大端序。
#[derive(Default)]
pub struct NbtLong {
    /// 长整型数据，取值 -2^63 到 2^63-1
    pub data: i64,
}

impl NbtLong {
    /// 创建 Long 标签
    ///
    /// - `data`: 长整型数据
    ///
    /// # 返回值
    ///
    /// 新建的 `NbtLong` 实例
    pub fn new(data: i64) -> Self {
        Self { data }
    }

    /// 判断另一个 NBT 标签是否为 Long 类型且值相等
    ///
    /// - `nbt`: 待比较的 NBT 标签
    ///
    /// # 返回值
    ///
    /// 一致时返回 `true`，否则返回 `false`
    pub fn eq(&self, nbt: &NbtType) -> bool {
        match nbt {
            NbtType::Long(nbt) => nbt.data == self.data,
            _ => false,
        }
    }

    /// 将自身转换为 `NbtType` 枚举变体
    ///
    /// # 返回值
    ///
    /// 对应的 `NbtType` 变体
    pub fn to_nbt(self) -> NbtType {
        NbtType::Long(self)
    }
}

impl NbtStream for NbtLong {
    /// 从流中按大端序读取 8 字节
    fn read<R: Read>(&mut self, stream: &mut R) -> CoreResult<()> {
        let mut temp = [0u8; 8];
        stream.read_exact(&mut temp).map_err(|err| io_error(err))?;

        self.data = i64::from_be_bytes(temp);

        Ok(())
    }

    /// 将 8 字节按大端序写入流
    fn write<W: Write>(&self, stream: &mut W) -> CoreResult<()> {
        let temp = i64::to_be_bytes(self.data);
        stream.write_all(&temp).map_err(|err| io_error(err))?;

        Ok(())
    }
}

/// NBT 浮点型，对应 TAG_Float
///
/// 存储一个 32 位 IEEE 754 浮点数，在二进制流中占用 4 字节，大端序。
#[derive(Default)]
pub struct NbtFloat {
    /// 浮点数据，IEEE 754 单精度
    pub data: f32,
}

impl NbtFloat {
    /// 创建 Float 标签
    ///
    /// - `data`: 浮点数据
    ///
    /// # 返回值
    ///
    /// 新建的 `NbtFloat` 实例
    pub fn new(data: f32) -> Self {
        Self { data }
    }

    /// 判断另一个 NBT 标签是否为 Float 类型且值相等
    ///
    /// - `nbt`: 待比较的 NBT 标签
    ///
    /// # 返回值
    ///
    /// 一致时返回 `true`，否则返回 `false`
    pub fn eq(&self, nbt: &NbtType) -> bool {
        match nbt {
            NbtType::Float(nbt) => nbt.data == self.data,
            _ => false,
        }
    }

    /// 将自身转换为 `NbtType` 枚举变体
    ///
    /// # 返回值
    ///
    /// 对应的 `NbtType` 变体
    pub fn to_nbt(self) -> NbtType {
        NbtType::Float(self)
    }
}

impl NbtStream for NbtFloat {
    /// 从流中按大端序读取 4 字节
    fn read<R: Read>(&mut self, stream: &mut R) -> CoreResult<()> {
        let mut temp = [0u8; 4];
        stream.read_exact(&mut temp).map_err(|err| io_error(err))?;

        self.data = f32::from_be_bytes(temp);

        Ok(())
    }

    /// 将 4 字节按大端序写入流
    fn write<W: Write>(&self, stream: &mut W) -> CoreResult<()> {
        let temp = f32::to_be_bytes(self.data);
        stream.write_all(&temp).map_err(|err| io_error(err))?;

        Ok(())
    }
}

/// NBT 双精度浮点型，对应 TAG_Double
///
/// 存储一个 64 位 IEEE 754 浮点数，在二进制流中占用 8 字节，大端序。
#[derive(Default)]
pub struct NbtDouble {
    /// 双精度浮点数据，IEEE 754 双精度
    pub data: f64,
}

impl NbtDouble {
    /// 创建 Double 标签
    ///
    /// - `data`: 双精度浮点数据
    ///
    /// # 返回值
    ///
    /// 新建的 `NbtDouble` 实例
    pub fn new(data: f64) -> Self {
        Self { data }
    }

    /// 判断另一个 NBT 标签是否为 Double 类型且值相等
    ///
    /// - `nbt`: 待比较的 NBT 标签
    ///
    /// # 返回值
    ///
    /// 一致时返回 `true`，否则返回 `false`
    pub fn eq(&self, nbt: &NbtType) -> bool {
        match nbt {
            NbtType::Double(nbt) => nbt.data == self.data,
            _ => false,
        }
    }

    /// 将自身转换为 `NbtType` 枚举变体
    ///
    /// # 返回值
    ///
    /// 对应的 `NbtType` 变体
    pub fn to_nbt(self) -> NbtType {
        NbtType::Double(self)
    }
}

impl NbtStream for NbtDouble {
    /// 从流中按大端序读取 8 字节
    fn read<R: Read>(&mut self, stream: &mut R) -> CoreResult<()> {
        let mut temp = [0u8; 8];
        stream.read_exact(&mut temp).map_err(|err| io_error(err))?;

        self.data = f64::from_be_bytes(temp);

        Ok(())
    }

    /// 将 8 字节按大端序写入流
    fn write<W: Write>(&self, stream: &mut W) -> CoreResult<()> {
        let temp = f64::to_be_bytes(self.data);
        stream.write_all(&temp).map_err(|err| io_error(err))?;

        Ok(())
    }
}
