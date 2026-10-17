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

use std::{
    collections::HashMap,
    io::{Read, Write},
};

use mml_names::i18_items::error_type::{CoreResult, ErrorType};

use crate::{NbtStream, NbtType, io_error, is_nbt_num};

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
    pub fn new() -> Self {
        Self {}
    }

    /// 判断另一个 NBT 标签是否也是 End 类型
    ///
    /// - `nbt`: 待比较的 NBT 标签
    pub fn eq(&self, nbt: &NbtType) -> bool {
        matches!(nbt, NbtType::End(_))
    }

    /// 将自身转换为 `NbtType` 枚举变体
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
pub struct NbtByte {
    /// 字节数据，按无符号 `u8` 存取，取值 0–255
    pub data: u8,
}

impl Default for NbtByte {
    fn default() -> Self {
        Self {
            data: Default::default(),
        }
    }
}

impl NbtByte {
    /// 创建 Byte 标签
    ///
    /// - `data`: 字节数据
    pub fn new(data: u8) -> Self {
        Self { data }
    }

    /// 判断另一个 NBT 标签是否为 Byte 类型且值相等
    ///
    /// - `nbt`: 待比较的 NBT 标签
    pub fn eq(&self, nbt: &NbtType) -> bool {
        match nbt {
            NbtType::Byte(nbt) => nbt.data == self.data,
            _ => false,
        }
    }

    /// 将自身转换为 `NbtType` 枚举变体
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
pub struct NbtShort {
    /// 短整型数据，取值 -32768 到 32767
    pub data: i16,
}

impl Default for NbtShort {
    fn default() -> Self {
        Self {
            data: Default::default(),
        }
    }
}

impl NbtShort {
    /// 创建 Short 标签
    ///
    /// - `data`: 短整型数据
    pub fn new(data: i16) -> Self {
        Self { data }
    }

    /// 判断另一个 NBT 标签是否为 Short 类型且值相等
    ///
    /// - `nbt`: 待比较的 NBT 标签
    pub fn eq(&self, nbt: &NbtType) -> bool {
        match nbt {
            NbtType::Short(nbt) => nbt.data == self.data,
            _ => false,
        }
    }

    /// 将自身转换为 `NbtType` 枚举变体
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
pub struct NbtInt {
    /// 整型数据，取值 -2^31 到 2^31-1
    pub data: i32,
}

impl Default for NbtInt {
    fn default() -> Self {
        Self {
            data: Default::default(),
        }
    }
}

impl NbtInt {
    /// 创建 Int 标签
    ///
    /// - `data`: 整型数据
    pub fn new(data: i32) -> Self {
        Self { data }
    }

    /// 判断另一个 NBT 标签是否为 Int 类型且值相等
    ///
    /// - `nbt`: 待比较的 NBT 标签
    pub fn eq(&self, nbt: &NbtType) -> bool {
        match nbt {
            NbtType::Int(nbt) => nbt.data == self.data,
            _ => false,
        }
    }

    /// 将自身转换为 `NbtType` 枚举变体
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
pub struct NbtLong {
    /// 长整型数据，取值 -2^63 到 2^63-1
    pub data: i64,
}

impl Default for NbtLong {
    fn default() -> Self {
        Self {
            data: Default::default(),
        }
    }
}

impl NbtLong {
    /// 创建 Long 标签
    ///
    /// - `data`: 长整型数据
    pub fn new(data: i64) -> Self {
        Self { data }
    }

    /// 判断另一个 NBT 标签是否为 Long 类型且值相等
    ///
    /// - `nbt`: 待比较的 NBT 标签
    pub fn eq(&self, nbt: &NbtType) -> bool {
        match nbt {
            NbtType::Long(nbt) => nbt.data == self.data,
            _ => false,
        }
    }

    /// 将自身转换为 `NbtType` 枚举变体
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
pub struct NbtFloat {
    /// 浮点数据，IEEE 754 单精度
    pub data: f32,
}

impl Default for NbtFloat {
    fn default() -> Self {
        Self {
            data: Default::default(),
        }
    }
}

impl NbtFloat {
    /// 创建 Float 标签
    ///
    /// - `data`: 浮点数据
    pub fn new(data: f32) -> Self {
        Self { data }
    }

    /// 判断另一个 NBT 标签是否为 Float 类型且值相等
    ///
    /// - `nbt`: 待比较的 NBT 标签
    pub fn eq(&self, nbt: &NbtType) -> bool {
        match nbt {
            NbtType::Float(nbt) => nbt.data == self.data,
            _ => false,
        }
    }

    /// 将自身转换为 `NbtType` 枚举变体
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
pub struct NbtDouble {
    /// 双精度浮点数据，IEEE 754 双精度
    pub data: f64,
}

impl Default for NbtDouble {
    fn default() -> Self {
        Self {
            data: Default::default(),
        }
    }
}

impl NbtDouble {
    /// 创建 Double 标签
    ///
    /// - `data`: 双精度浮点数据
    pub fn new(data: f64) -> Self {
        Self { data }
    }

    /// 判断另一个 NBT 标签是否为 Double 类型且值相等
    ///
    /// - `nbt`: 待比较的 NBT 标签
    pub fn eq(&self, nbt: &NbtType) -> bool {
        match nbt {
            NbtType::Double(nbt) => nbt.data == self.data,
            _ => false,
        }
    }

    /// 将自身转换为 `NbtType` 枚举变体
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

/// NBT 字节数组类型，对应 TAG_Byte_Array
///
/// 二进制流格式为 4 字节元素个数，大端序 i32，后跟原始字节数据。
pub struct NbtByteArray {
    /// 字节数组数据，元素按无符号字节存取
    pub data: Vec<u8>,
}

impl Default for NbtByteArray {
    fn default() -> Self {
        Self {
            data: Default::default(),
        }
    }
}

impl NbtByteArray {
    /// 创建 ByteArray 标签
    ///
    /// - `data`: 字节数组数据
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// 判断另一个 NBT 标签是否为 ByteArray 类型且数据相等
    ///
    /// - `nbt`: 待比较的 NBT 标签
    pub fn eq(&self, nbt: &NbtType) -> bool {
        match nbt {
            NbtType::ByteArray(nbt) => nbt.data == self.data,
            _ => false,
        }
    }

    /// 将自身转换为 `NbtType` 枚举变体
    pub fn to_nbt(self) -> NbtType {
        NbtType::ByteArray(self)
    }
}

impl NbtStream for NbtByteArray {
    /// 先读 4 字节元素个数，再读对应数量的字节
    fn read<R: Read>(&mut self, stream: &mut R) -> CoreResult<()> {
        let mut temp = [0u8; 4];
        stream.read_exact(&mut temp).map_err(|err| io_error(err))?;

        let len = i32::from_be_bytes(temp);

        let mut temp = vec![0; len as usize];
        stream.read_exact(&mut temp).map_err(|err| io_error(err))?;

        self.data = temp;

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

/// NBT 字符串类型，对应 TAG_String
///
/// 二进制流格式为 2 字节长度，大端序 i16，后跟 UTF-8 字节序列。
/// 长度字段记录的是**字节数**，不是字符数。
pub struct NbtString {
    /// 字符串数据，按 UTF-8 编码写出
    pub data: String,
}

impl Default for NbtString {
    fn default() -> Self {
        Self {
            data: Default::default(),
        }
    }
}

impl NbtString {
    /// 创建 String 标签
    ///
    /// - `data`: 字符串数据
    pub fn new(data: String) -> Self {
        Self { data }
    }

    /// 判断另一个 NBT 标签是否为 String 类型且值相等
    ///
    /// - `nbt`: 待比较的 NBT 标签
    pub fn eq(&self, nbt: &NbtType) -> bool {
        match nbt {
            NbtType::String(nbt) => nbt.data == self.data,
            _ => false,
        }
    }

    /// 将自身转换为 `NbtType` 枚举变体
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

fn decode_unit_2(bytes: &[u8], i: usize) -> Option<u16> {
    let tail = *bytes.get(i + 1)?;
    Some((u16::from(bytes[i] & 0x1F) << 6) | u16::from(tail & 0x3F))
}

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

        let len = i16::from_be_bytes(temp);

        let mut temp = vec![0; len as usize];
        stream.read_exact(&mut temp).map_err(|err| io_error(err))?;

        self.data = decode_string(temp);

        Ok(())
    }

    /// 先写 2 字节的字节数，再写 UTF-8 字节
    fn write<W: Write>(&self, stream: &mut W) -> CoreResult<()> {
        let temp = i16::to_be_bytes(self.data.len() as i16);
        stream.write_all(&temp).map_err(|err| io_error(err))?;
        stream
            .write_all(&self.data.as_bytes())
            .map_err(|err| io_error(err))?;

        Ok(())
    }
}

/// NBT 列表类型，对应 TAG_List
///
/// 存储一组同类型的 NBT 元素，二进制流格式为 1 字节元素类型序号、
/// 4 字节元素个数，大端序 i32，后跟各元素的 NBT 数据；元素自身不带类型序号。
///
/// # 注意
///
/// 不允许混合不同类型的元素，修改列表类型时会清空已有数据。
pub struct NbtList {
    /// 元素列表，每个元素的类型序号都必须等于 `nbt_num`
    data: Vec<NbtType>,
    /// 元素类型序号，取值 0–12；空列表写出时会临时改写成 0
    nbt_num: u8,
}

impl Default for NbtList {
    fn default() -> Self {
        Self {
            data: Default::default(),
            nbt_num: Default::default(),
        }
    }
}

impl NbtList {
    /// 创建指定元素类型的空 List 标签
    ///
    /// 序号不做校验，非法序号要到读回时才会报 `NbtTypeError`。
    ///
    /// - `nbt_num`: 元素类型序号，取值 0–12
    pub fn new(nbt_num: u8) -> Self {
        Self {
            nbt_num,
            data: Vec::new(),
        }
    }

    /// 按实例设置列表的元素类型，同时清空已有数据
    ///
    /// - `nbt_type`: 元素类型实例，取其类型序号
    pub fn set_type(&mut self, nbt_type: NbtType) {
        self.nbt_num = nbt_type.get_num();
        self.data.clear();
    }

    /// 按类型序号设置列表的元素类型，同时清空已有数据
    ///
    /// 序号不在合法范围 0–12 内时忽略本次调用。
    ///
    /// - `nbt_num`: 元素类型序号，取值 0–12
    pub fn set_num(&mut self, nbt_num: u8) {
        if is_nbt_num(nbt_num) {
            self.nbt_num = nbt_num;
            self.data.clear();
        }
    }

    /// 向列表末尾追加一个元素
    ///
    /// - `nbt`: 待添加的元素
    ///
    /// # 返回值
    ///
    /// 元素类型序号与列表一致时追加并返回 `true`，否则返回 `false` 且不改动列表
    pub fn add_item(&mut self, nbt: NbtType) -> bool {
        if nbt.get_num() != self.nbt_num {
            false
        } else {
            self.data.push(nbt);

            true
        }
    }

    /// 在指定位置插入一个元素
    ///
    /// 元素类型序号必须与列表一致：列表是同类型容器，混进别的类型就写不出合法 NBT。
    /// 空列表（元素类型还是 `END`）要先由调用方用 [`NbtList::set_num`] 定好类型。
    ///
    /// - `index`: 插入位置，从 0 开始；等于元素个数就是追加
    /// - `nbt`: 待插入的元素
    ///
    /// # 返回值
    ///
    /// 越界或类型不一致时返回 `false` 且不改动列表
    pub fn insert_item(&mut self, index: usize, nbt: NbtType) -> bool {
        if index > self.data.len() || nbt.get_num() != self.nbt_num {
            false
        } else {
            self.data.insert(index, nbt);

            true
        }
    }

    /// 移除并返回指定索引的元素
    ///
    /// - `index`: 元素索引，从 0 开始
    ///
    /// # 返回值
    ///
    /// 被移除的元素；`index` 越界会 panic
    pub fn remove(&mut self, index: usize) -> NbtType {
        self.data.remove(index)
    }

    /// 获取指定索引元素的不可变引用
    ///
    /// - `index`: 元素索引，从 0 开始
    ///
    /// # 返回值
    ///
    /// 索引越界时返回 `None`
    pub fn get_item(&self, index: usize) -> Option<&NbtType> {
        self.data.get(index)
    }

    /// 获取指定索引元素的可变引用，就地改某个元素的值
    ///
    /// 只改元素**内容**，不动元素类型：类型序号仍由列表的 `nbt_num` 统一决定，
    /// 调用方不要在这里把元素换成别的类型。
    ///
    /// - `index`: 元素索引，从 0 开始
    ///
    /// # 返回值
    ///
    /// 索引越界时返回 `None`
    pub fn get_item_mut(&mut self, index: usize) -> Option<&mut NbtType> {
        self.data.get_mut(index)
    }

    /// 返回列表中的元素数量
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// 返回列表元素的迭代器
    ///
    /// # 返回值
    ///
    /// 按列表内的存储顺序遍历全部元素
    pub fn iter(&self) -> impl Iterator<Item = &NbtType> {
        self.data.iter()
    }

    /// 判断另一个 NBT 标签是否为 List 类型且元素类型、数量和值均相等
    ///
    /// - `nbt`: 待比较的 NBT 标签
    pub fn eq(&self, nbt: &NbtType) -> bool {
        match nbt {
            NbtType::List(nbt) => {
                if self.nbt_num != nbt.nbt_num {
                    return false;
                }
                if self.data.len() != nbt.data.len() {
                    return false;
                }

                for index in 0..self.data.len() {
                    let item1 = self.data.get(index).unwrap();
                    let item2 = nbt.data.get(index).unwrap();

                    if !item1.eq(item2) {
                        return false;
                    }
                }

                return true;
            }
            _ => false,
        }
    }

    /// 将自身转换为 `NbtType` 枚举变体
    pub fn to_nbt(self) -> NbtType {
        NbtType::List(self)
    }
}

impl NbtStream for NbtList {
    /// 依次读元素类型序号、元素个数和每个元素的 NBT 数据
    fn read<R: Read>(&mut self, stream: &mut R) -> CoreResult<()> {
        let mut temp = [0u8; 1];
        stream.read_exact(&mut temp).map_err(|err| io_error(err))?;

        self.nbt_num = temp[0];
        if !is_nbt_num(self.nbt_num) {
            return Err(ErrorType::NbtTypeError);
        }

        let mut temp = [0u8; 4];
        stream.read_exact(&mut temp).map_err(|err| io_error(err))?;

        let len = i32::from_be_bytes(temp);

        for _i in 0..len {
            let mut nbt = NbtType::get_nbt(self.nbt_num).unwrap();
            nbt.read(stream)?;
            self.data.push(nbt);
        }

        Ok(())
    }

    /// 依次写元素类型序号、元素个数和每个元素的 NBT 数据
    fn write<W: Write>(&self, stream: &mut W) -> CoreResult<()> {
        // 空列表按规范写元素类型 0，即 TAG_End
        let nbt_type = if self.data.len() == 0 {
            0
        } else {
            self.nbt_num
        };

        let temp = [nbt_type];
        stream.write_all(&temp).map_err(|err| io_error(err))?;

        let temp = i32::to_be_bytes(self.data.len() as i32);
        stream.write_all(&temp).map_err(|err| io_error(err))?;

        for nbt in &self.data {
            nbt.write(stream)?;
        }

        Ok(())
    }
}

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
pub struct NbtCompound {
    /// 键值对数据；HashMap 不保证顺序，写出顺序与插入顺序无关
    pub data: HashMap<String, NbtType>,
}

impl Default for NbtCompound {
    fn default() -> Self {
        Self {
            data: Default::default(),
        }
    }
}

impl NbtCompound {
    /// 创建空的 Compound 标签
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// 判断另一个 NBT 标签是否为 Compound 类型且所有键值对一致
    ///
    /// - `nbt`: 待比较的 NBT 标签
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
    pub fn get(&self, key: &str) -> Option<&NbtType> {
        self.data.get(key)
    }

    /// 获取指定键对应的 NBT 标签的可变引用
    ///
    /// - `key`: 键名
    pub fn get_mut(&mut self, key: &str) -> Option<&mut NbtType> {
        self.data.get_mut(key)
    }

    /// 从 Compound 中提取 `&NbtByteArray`，自动进行类型匹配
    ///
    /// - `key`: 键名
    pub fn get_byte_array(&self, key: &str) -> Option<&NbtByteArray> {
        match self.get(key) {
            Some(NbtType::ByteArray(v)) => Some(v),
            _ => None,
        }
    }

    /// 从 Compound 中提取 `&mut NbtByteArray`，自动进行类型匹配
    ///
    /// - `key`: 键名
    pub fn get_byte_array_mut(&mut self, key: &str) -> Option<&mut NbtByteArray> {
        match self.get_mut(key) {
            Some(NbtType::ByteArray(v)) => Some(v),
            _ => None,
        }
    }

    /// 从 Compound 中提取 `&NbtLongArray`，自动进行类型匹配
    ///
    /// - `key`: 键名
    pub fn get_long_array(&self, key: &str) -> Option<&NbtLongArray> {
        match self.get(key) {
            Some(NbtType::LongArray(v)) => Some(v),
            _ => None,
        }
    }

    /// 从 Compound 中提取 `&mut NbtLongArray`，自动进行类型匹配
    ///
    /// - `key`: 键名
    pub fn get_long_array_mut(&mut self, key: &str) -> Option<&mut NbtLongArray> {
        match self.get_mut(key) {
            Some(NbtType::LongArray(v)) => Some(v),
            _ => None,
        }
    }

    /// 从 Compound 中提取嵌套的 `&NbtCompound`，自动进行类型匹配
    ///
    /// - `key`: 键名
    pub fn get_compound(&self, key: &str) -> Option<&NbtCompound> {
        match self.get(key) {
            Some(NbtType::Compound(v)) => Some(v),
            _ => None,
        }
    }

    /// 从 Compound 中提取嵌套的 `&mut NbtCompound`，自动进行类型匹配
    ///
    /// - `key`: 键名
    pub fn get_compound_mut(&mut self, key: &str) -> Option<&mut NbtCompound> {
        match self.get_mut(key) {
            Some(NbtType::Compound(v)) => Some(v),
            _ => None,
        }
    }

    /// 从 Compound 中提取 TAG_Long 的 `i64` 值
    ///
    /// - `key`: 键名
    pub fn get_long(&self, key: &str) -> Option<i64> {
        match self.get(key) {
            Some(NbtType::Long(v)) => Some(v.data),
            _ => None,
        }
    }

    /// 从 Compound 中提取 TAG_Short 的 `i16` 值
    ///
    /// - `key`: 键名
    pub fn get_short(&self, key: &str) -> Option<i16> {
        match self.get(key) {
            Some(NbtType::Short(v)) => Some(v.data),
            _ => None,
        }
    }

    /// 从 Compound 中提取 TAG_Int 的 `i32` 值
    ///
    /// - `key`: 键名
    pub fn get_int(&self, key: &str) -> Option<i32> {
        match self.get(key) {
            Some(NbtType::Int(v)) => Some(v.data),
            _ => None,
        }
    }

    /// 从 Compound 中提取 TAG_Byte 的 `u8` 值
    ///
    /// - `key`: 键名
    pub fn get_byte(&self, key: &str) -> Option<u8> {
        match self.get(key) {
            Some(NbtType::Byte(v)) => Some(v.data),
            _ => None,
        }
    }

    /// 从 Compound 中提取 TAG_String 的 `String` 值，克隆返回
    ///
    /// - `key`: 键名
    pub fn get_string(&self, key: &str) -> Option<String> {
        match self.get(key) {
            Some(NbtType::String(v)) => Some(v.data.clone()),
            _ => None,
        }
    }

    /// 从 Compound 中提取 `&NbtList`，自动进行类型匹配
    ///
    /// - `key`: 键名
    pub fn get_list(&self, key: &str) -> Option<&NbtList> {
        match self.get(key) {
            Some(NbtType::List(v)) => Some(v),
            _ => None,
        }
    }

    /// 从 Compound 中提取 `&mut NbtList`，自动进行类型匹配
    ///
    /// - `key`: 键名
    pub fn get_list_mut(&mut self, key: &str) -> Option<&mut NbtList> {
        match self.get_mut(key) {
            Some(NbtType::List(v)) => Some(v),
            _ => None,
        }
    }

    /// 将自身转换为 `NbtType` 枚举变体
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

            let len = i16::from_be_bytes(temp);

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
                let temp = i16::to_be_bytes(key.len() as i16);
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

/// NBT 整型数组类型，对应 TAG_Int_Array
///
/// 二进制流格式为 4 字节元素个数，大端序 i32，后跟每个元素 4 字节，大端序。
pub struct NbtIntArray {
    /// 32 位整数数组数据，元素按大端序写出
    pub data: Vec<i32>,
}

impl Default for NbtIntArray {
    fn default() -> Self {
        Self {
            data: Default::default(),
        }
    }
}

impl NbtIntArray {
    /// 创建 IntArray 标签
    ///
    /// - `data`: 32 位整数数组数据
    pub fn new(data: Vec<i32>) -> Self {
        Self { data }
    }

    /// 判断另一个 NBT 标签是否为 IntArray 类型且数据相等
    ///
    /// - `nbt`: 待比较的 NBT 标签
    pub fn eq(&self, nbt: &NbtType) -> bool {
        match nbt {
            NbtType::IntArray(nbt) => nbt.data == self.data,
            _ => false,
        }
    }

    /// 将自身转换为 `NbtType` 枚举变体
    pub fn to_nbt(self) -> NbtType {
        NbtType::IntArray(self)
    }
}

impl NbtStream for NbtIntArray {
    /// 先读 4 字节元素个数，再按每元素 4 字节读出
    fn read<R: Read>(&mut self, stream: &mut R) -> CoreResult<()> {
        let mut temp = [0u8; 4];
        stream.read_exact(&mut temp).map_err(|err| io_error(err))?;

        // 长度字段表示元素个数，总字节数 = 元素个数 × 4
        let len = i32::from_be_bytes(temp) * 4;

        let mut temp = vec![0; len as usize];
        stream.read_exact(&mut temp).map_err(|err| io_error(err))?;

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
pub struct NbtLongArray {
    /// 64 位整数数组数据，元素按大端序写出
    pub data: Vec<i64>,
}

impl Default for NbtLongArray {
    fn default() -> Self {
        Self {
            data: Default::default(),
        }
    }
}

impl NbtLongArray {
    /// 创建 LongArray 标签
    ///
    /// - `data`: 64 位整数数组数据
    pub fn new(data: Vec<i64>) -> Self {
        Self { data }
    }

    /// 判断另一个 NBT 标签是否为 LongArray 类型且数据相等
    ///
    /// - `nbt`: 待比较的 NBT 标签
    pub fn eq(&self, nbt: &NbtType) -> bool {
        match nbt {
            NbtType::LongArray(nbt) => nbt.data == self.data,
            _ => false,
        }
    }

    /// 将自身转换为 `NbtType` 枚举变体
    pub fn to_nbt(self) -> NbtType {
        NbtType::LongArray(self)
    }
}

impl NbtStream for NbtLongArray {
    /// 先读 4 字节元素个数，再按每元素 8 字节读出
    fn read<R: Read>(&mut self, stream: &mut R) -> CoreResult<()> {
        let mut temp = [0u8; 4];
        stream.read_exact(&mut temp).map_err(|err| io_error(err))?;

        // 长度字段表示元素个数，总字节数 = 元素个数 × 8
        let len = i32::from_be_bytes(temp) * 8;

        let mut temp = vec![0; len as usize];
        stream.read_exact(&mut temp).map_err(|err| io_error(err))?;

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

/// 创建空的 NbtEnd 实例
pub fn end() -> NbtEnd {
    NbtEnd::new()
}

/// 创建 NbtByte 实例
///
/// - `data`: 字节数据
pub fn byte(data: u8) -> NbtByte {
    NbtByte::new(data)
}

/// 创建 NbtShort 实例
///
/// - `data`: 短整型数据
pub fn short(data: i16) -> NbtShort {
    NbtShort::new(data)
}

/// 创建 NbtInt 实例
///
/// - `data`: 整型数据
pub fn int(data: i32) -> NbtInt {
    NbtInt::new(data)
}

/// 创建 NbtLong 实例
///
/// - `data`: 长整型数据
pub fn long(data: i64) -> NbtLong {
    NbtLong::new(data)
}

/// 创建 NbtFloat 实例
///
/// - `data`: 浮点数据
pub fn float(data: f32) -> NbtFloat {
    NbtFloat::new(data)
}

/// 创建 NbtDouble 实例
///
/// - `data`: 双精度浮点数据
pub fn double(data: f64) -> NbtDouble {
    NbtDouble::new(data)
}

/// 创建 NbtByteArray 实例
///
/// - `data`: 字节数组数据
pub fn byte_array(data: Vec<u8>) -> NbtByteArray {
    NbtByteArray::new(data)
}

/// 创建 NbtString 实例
///
/// - `data`: 字符串数据
pub fn string(data: &str) -> NbtString {
    NbtString::new(String::from(data))
}

/// 创建指定元素类型的空 NbtList 实例
///
/// - `nbt_num`: 元素类型序号，取值 0–12
pub fn list(nbt_num: u8) -> NbtList {
    NbtList::new(nbt_num)
}

/// 创建空的 NbtCompound 实例
pub fn compound() -> NbtCompound {
    NbtCompound::new()
}

/// 创建 NbtIntArray 实例
///
/// - `data`: 32 位整数数组数据
pub fn int_array(data: Vec<i32>) -> NbtIntArray {
    NbtIntArray::new(data)
}

/// 创建 NbtLongArray 实例
///
/// - `data`: 64 位整数数组数据
pub fn long_array(data: Vec<i64>) -> NbtLongArray {
    NbtLongArray::new(data)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    /// Java 写的 NBT 会把补充平面字符拆成两个 3 字节序列，`from_utf8` 直接拒绝
    #[test]
    fn decode_string_accepts_cesu8_surrogate_pair() {
        // ED A0 BE ED BA 93 = D83E DE93 = U+1FA93
        let text = decode_string(vec![0xED, 0xA0, 0xBE, 0xED, 0xBA, 0x93]);
        assert_eq!(text, "\u{1FA93}");
    }

    /// 合法 UTF-8 与落单代理项都要有结果，不能把整棵树读挂
    #[test]
    fn decode_string_keeps_plain_utf8_and_survives_broken_bytes() {
        assert_eq!(decode_string("普通文本".as_bytes().to_vec()), "普通文本");
        assert_eq!(decode_string(vec![0xFF, 0xFE]), "\u{FFFD}\u{FFFD}");
    }


    use super::*;
    use crate::{
        NBT_BYTE_ARRAY_ORDER, NBT_BYTE_ORDER, NBT_COMPOUND_ORDER, NBT_INT_ARRAY_ORDER,
        NBT_INT_ORDER, NBT_LIST_ORDER, NBT_LONG_ARRAY_ORDER, NBT_LONG_ORDER, NBT_SHORT_ORDER,
        NBT_STRING_ORDER,
    };

    /// 从字节切片中读取标签数据
    fn read_from<T: NbtStream>(mut nbt: T, bytes: &[u8]) -> T {
        let mut cursor = Cursor::new(bytes);
        nbt.read(&mut cursor).unwrap();
        nbt
    }

    /// 将标签写入内存并返回字节序列
    fn write_to<T: NbtStream>(nbt: &T) -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::<u8>::new());
        nbt.write(&mut cursor).unwrap();
        cursor.into_inner()
    }

    // ---------- 标量类型 ----------

    /// Byte 往返，写入仅占 1 字节
    #[test]
    fn byte_round_trip() {
        for value in [0u8, 1, 127, 128, 255] {
            let nbt = byte(value);
            assert_eq!(write_to(&nbt), vec![value]);
            assert_eq!(read_from(NbtByte::default(), &[value]).data, value);
        }
    }

    /// Short 使用大端序读写
    #[test]
    fn short_big_endian() {
        let nbt = short(0x0102);
        assert_eq!(write_to(&nbt), vec![0x01, 0x02]);
        assert_eq!(read_from(NbtShort::default(), &[0xFF, 0xFE]).data, -2);
        assert_eq!(read_from(NbtShort::default(), &[0x80, 0x00]).data, i16::MIN);
    }

    /// Int 使用大端序读写
    #[test]
    fn int_big_endian() {
        let nbt = int(0x01020304);
        assert_eq!(write_to(&nbt), vec![0x01, 0x02, 0x03, 0x04]);
        assert_eq!(
            read_from(NbtInt::default(), &[0xFF, 0xFF, 0xFF, 0xFE]).data,
            -2
        );
        assert_eq!(
            read_from(NbtInt::default(), &[0x00, 0x00, 0x00, 0x00]).data,
            0
        );
    }

    /// Long 使用大端序读写
    #[test]
    fn long_big_endian() {
        let nbt = long(0x0102030405060708);
        assert_eq!(
            write_to(&nbt),
            vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
        );
        assert_eq!(
            read_from(
                NbtLong::default(),
                &[0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
            )
            .data,
            i64::MIN
        );
    }

    /// Float / Double 按位精确往返
    #[test]
    fn float_double_round_trip() {
        let f = float(1.5);
        assert_eq!(write_to(&f), f32::to_be_bytes(1.5).to_vec());
        let bytes = f32::to_be_bytes(-0.25);
        assert_eq!(read_from(NbtFloat::default(), &bytes).data, -0.25);

        let d = double(3.141592653589793);
        assert_eq!(write_to(&d), f64::to_be_bytes(3.141592653589793).to_vec());
        let bytes = f64::to_be_bytes(-2.5);
        assert_eq!(read_from(NbtDouble::default(), &bytes).data, -2.5);
    }

    // ---------- 数组类型 ----------

    /// ByteArray 的二进制格式：4 字节长度 + 数据
    #[test]
    fn byte_array_format() {
        let nbt = byte_array(vec![0xAB, 0xCD]);
        assert_eq!(write_to(&nbt), vec![0, 0, 0, 2, 0xAB, 0xCD]);

        let back = read_from(NbtByteArray::default(), &[0, 0, 0, 3, 1, 2, 3]);
        assert_eq!(back.data, vec![1, 2, 3]);

        // 空数组
        let empty = read_from(NbtByteArray::default(), &[0, 0, 0, 0]);
        assert!(empty.data.is_empty());
    }

    /// IntArray 的二进制格式：4 字节元素个数 + 每元素 4 字节，大端序
    #[test]
    fn int_array_format() {
        let nbt = int_array(vec![1, -1]);
        assert_eq!(
            write_to(&nbt),
            vec![0, 0, 0, 2, 0, 0, 0, 1, 0xFF, 0xFF, 0xFF, 0xFF]
        );

        let back = read_from(
            NbtIntArray::default(),
            &[0, 0, 0, 2, 0, 0, 0, 1, 0xFF, 0xFF, 0xFF, 0xFF],
        );
        assert_eq!(back.data, vec![1, -1]);
    }

    /// LongArray 的二进制格式：4 字节元素个数 + 每元素 8 字节，大端序
    #[test]
    fn long_array_format() {
        let nbt = long_array(vec![i64::MIN, 1]);
        let bytes = write_to(&nbt);
        assert_eq!(&bytes[..4], &[0, 0, 0, 2]);
        assert_eq!(bytes.len(), 4 + 16);

        let back = read_from(NbtLongArray::default(), &bytes);
        assert_eq!(back.data, vec![i64::MIN, 1]);
    }

    // ---------- 字符串类型 ----------

    /// String 长度前缀为字节数而非字符数，多字节 UTF-8 可正确往返
    #[test]
    fn string_utf8_length_is_bytes() {
        // "颜" 占 3 个 UTF-8 字节
        let nbt = string("颜");
        let bytes = write_to(&nbt);
        assert_eq!(&bytes[..2], &[0, 3]);
        assert_eq!(bytes.len(), 2 + 3);

        let back = read_from(NbtString::default(), &bytes);
        assert_eq!(back.data, "颜");

        // 空字符串
        let empty = write_to(&string(""));
        assert_eq!(empty, vec![0, 0]);
    }

    /// String 读取遇到非法 UTF-8 不再整棵树失败，按替换符收下
    #[test]
    fn string_invalid_utf8_falls_back_to_replacement() {
        let mut nbt = NbtString::default();
        let mut cursor = Cursor::new([0u8, 1, 0xFF]);
        assert!(nbt.read(&mut cursor).is_ok());
        assert_eq!(nbt.data, "\u{FFFD}");
    }

    // ---------- List 类型 ----------

    /// add_item 强制元素类型一致
    #[test]
    fn list_type_enforcement() {
        let mut list = list(NBT_INT_ORDER);
        // 类型不匹配应被拒绝
        assert!(!list.add_item(byte(1).to_nbt()));
        assert_eq!(list.len(), 0);
        // 类型匹配应成功
        assert!(list.add_item(int(1).to_nbt()));
        assert!(list.add_item(int(2).to_nbt()));
        assert_eq!(list.len(), 2);
    }

    /// set_num / set_type 的行为：非法序号被忽略，合法序号清空数据
    #[test]
    fn list_set_num_and_set_type() {
        let mut list = list(NBT_INT_ORDER);
        assert!(list.add_item(int(1).to_nbt()));

        // 非法序号：忽略操作，数据保留
        list.set_num(13);
        assert_eq!(list.len(), 1);

        // 合法序号：清空数据
        list.set_num(NBT_BYTE_ORDER);
        assert_eq!(list.len(), 0);
        assert!(!list.add_item(int(1).to_nbt()));
        assert!(list.add_item(byte(2).to_nbt()));

        // set_type 同样清空数据
        list.set_type(NbtType::long_array());
        assert_eq!(list.len(), 0);
        assert!(list.add_item(long_array(vec![1]).to_nbt()));
    }

    /// 空列表写入时元素类型序号为 0，即 TAG_End
    #[test]
    fn list_empty_writes_end_type() {
        let list = list(NBT_INT_ORDER);
        let bytes = write_to(&list);
        assert_eq!(bytes, vec![0, 0, 0, 0, 0]);
    }

    /// List 读写往返，含 get_item / remove / iter
    #[test]
    fn list_round_trip_and_accessors() {
        let mut list = list(NBT_SHORT_ORDER);
        assert!(list.add_item(short(1).to_nbt()));
        assert!(list.add_item(short(-2).to_nbt()));
        assert!(list.add_item(short(3).to_nbt()));

        let bytes = write_to(&list);
        // 类型序号 + 元素个数 + 每元素 2 字节
        assert_eq!(bytes, vec![2, 0, 0, 0, 3, 0, 1, 0xFF, 0xFE, 0, 3]);

        let mut back = NbtList::default();
        let mut cursor = Cursor::new(bytes.as_slice());
        back.read(&mut cursor).unwrap();

        assert_eq!(back.len(), 3);
        assert_eq!(back.get_item(1).unwrap().as_short().unwrap().data, -2);
        assert_eq!(back.iter().count(), 3);

        // remove 返回被移除的元素
        let removed = back.remove(0);
        assert_eq!(removed.as_short().unwrap().data, 1);
        assert_eq!(back.len(), 2);
    }

    /// List 读取时元素类型序号非法应返回错误
    #[test]
    fn list_invalid_type_returns_error() {
        let mut nbt = NbtList::default();
        let mut cursor = Cursor::new([13u8, 0, 0, 0, 0]);
        assert!(nbt.read(&mut cursor).is_err());
    }

    // ---------- Compound 类型 ----------

    /// 单条目 Compound 的写入格式：类型序号 + 键名 + 值 + End
    #[test]
    fn compound_write_format() {
        let mut com = compound();
        com.data.insert("k".into(), byte(7).to_nbt());
        let bytes = write_to(&com);
        assert_eq!(
            bytes,
            vec![
                NBT_BYTE_ORDER,
                0,
                1,
                b'k', // 键名长度 + 键名
                7,    // 值
                0,    // TAG_End 终止
            ]
        );
    }

    /// 手工构造字节读取 Compound，并验证各类 getter
    #[test]
    fn compound_read_and_getters() {
        // { "a": byte 1, "s": short 2, "i": int 3, "l": long 4, "str": "hi" }
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[NBT_BYTE_ORDER, 0, 1, b'a', 1]);
        bytes.extend_from_slice(&[NBT_SHORT_ORDER, 0, 1, b's', 0, 2]);
        bytes.extend_from_slice(&[NBT_INT_ORDER, 0, 1, b'i', 0, 0, 0, 3]);
        bytes.extend_from_slice(&[NBT_LONG_ORDER, 0, 1, b'l', 0, 0, 0, 0, 0, 0, 0, 4]);
        bytes.extend_from_slice(&[NBT_STRING_ORDER, 0, 3, b's', b't', b'r', 0, 2, b'h', b'i']);
        bytes.push(0); // TAG_End

        let mut nbt = NbtCompound::default();
        let mut cursor = Cursor::new(bytes.as_slice());
        nbt.read(&mut cursor).unwrap();

        assert_eq!(nbt.get_byte("a"), Some(1));
        assert_eq!(nbt.get_short("s"), Some(2));
        assert_eq!(nbt.get_int("i"), Some(3));
        assert_eq!(nbt.get_long("l"), Some(4));
        assert_eq!(nbt.get_string("str"), Some("hi".to_string()));

        // 类型不匹配时 getter 应返回 None
        assert_eq!(nbt.get_int("a"), None);
        assert_eq!(nbt.get_byte("s"), None);
        assert_eq!(nbt.get_string("i"), None);
        // 不存在的键
        assert_eq!(nbt.get_byte("missing"), None);

        // 引用型 getter
        assert!(nbt.get("a").is_some());
        assert!(nbt.get_mut("a").is_some());
        assert!(nbt.get_compound("a").is_none());
        assert!(nbt.get_list("a").is_none());
        assert!(nbt.get_byte_array("a").is_none());
        assert!(nbt.get_long_array("a").is_none());
    }

    /// 嵌套 Compound 往返
    #[test]
    fn compound_nested_round_trip() {
        let mut inner = compound();
        inner.data.insert("x".into(), int(9).to_nbt());

        let mut root = compound();
        root.data.insert("inner".into(), inner.to_nbt());
        root.data
            .insert("arr".into(), byte_array(vec![1, 2, 3]).to_nbt());
        root.data
            .insert("ia".into(), int_array(vec![1, 2, 3, -4]).to_nbt());
        root.data
            .insert("la".into(), long_array(vec![5, -6]).to_nbt());

        let bytes = write_to(&root);
        let mut back = NbtCompound::default();
        let mut cursor = Cursor::new(bytes.as_slice());
        back.read(&mut cursor).unwrap();

        // 先取嵌套数据，再比较整体；to_nbt 会消耗所有权，不能提前调用
        assert_eq!(back.get_compound("inner").unwrap().get_int("x"), Some(9));
        assert_eq!(back.get_byte_array("arr").unwrap().data, vec![1, 2, 3]);
        // Compound 没有 get_int_array，需通过 get + as_int_array 取值
        assert!(back.get_long_array("ia").is_none());
        assert_eq!(
            back.get("ia").unwrap().as_int_array().unwrap().data,
            vec![1, 2, 3, -4]
        );
        assert_eq!(
            back.get("la").unwrap().as_long_array().unwrap().data,
            vec![5, -6]
        );

        assert!(root.eq(&back.to_nbt()));
    }

    /// Compound 读取时遇到非法类型序号应返回错误
    #[test]
    fn compound_invalid_type_returns_error() {
        let mut nbt = NbtCompound::default();
        // 类型序号 13 非法
        let mut cursor = Cursor::new([13u8]);
        assert!(nbt.read(&mut cursor).is_err());
    }

    /// eq 语义：同型同值为真，异型或异值为假
    #[test]
    fn eq_semantics() {
        let mut com1 = compound();
        com1.data.insert("a".into(), int(1).to_nbt());
        let nbt1 = com1.to_nbt();

        // 同内容
        let mut com2 = compound();
        com2.data.insert("a".into(), int(1).to_nbt());
        let nbt2 = com2.to_nbt();
        assert!(nbt1.eq(&nbt2));

        // 值不同
        let mut com3 = compound();
        com3.data.insert("a".into(), int(2).to_nbt());
        let nbt3 = com3.to_nbt();
        assert!(!nbt1.eq(&nbt3));

        // 键不同
        let mut com4 = compound();
        com4.data.insert("b".into(), int(1).to_nbt());
        let nbt4 = com4.to_nbt();
        assert!(!nbt1.eq(&nbt4));

        // 数量不同
        let mut com5 = compound();
        com5.data.insert("a".into(), int(1).to_nbt());
        com5.data.insert("b".into(), int(2).to_nbt());
        let nbt5 = com5.to_nbt();
        assert!(!nbt1.eq(&nbt5));

        // 与其它类型比较
        assert!(!nbt1.eq(&NbtType::end()));
        assert!(!int(1).to_nbt().eq(&long(1).to_nbt()));
        assert!(int(1).to_nbt().eq(&int(1).to_nbt()));

        // List eq：类型序号、数量、逐元素比较
        let mut l1 = list(NBT_INT_ORDER);
        assert!(l1.add_item(int(1).to_nbt()));
        let mut l2 = list(NBT_INT_ORDER);
        assert!(l2.add_item(int(1).to_nbt()));
        let l1_nbt = l1.to_nbt();
        assert!(l1_nbt.eq(&l2.to_nbt()));

        let mut l3 = list(NBT_SHORT_ORDER);
        assert!(l3.add_item(short(1).to_nbt()));
        assert!(!l1_nbt.eq(&l3.to_nbt()));
    }

    /// End 标签读写均为空操作
    #[test]
    fn end_no_op() {
        assert_eq!(write_to(&end()), Vec::<u8>::new());
        read_from(end(), &[]);
    }

    /// NbtType 的 as_* 系列方法在类型不匹配时应返回 None
    #[test]
    fn nbt_type_as_helpers() {
        let mut nbt = int(1).to_nbt();
        assert!(nbt.as_int().is_some());
        assert!(nbt.as_int_mut().is_some());
        assert!(nbt.as_byte().is_none());
        assert!(nbt.as_short().is_none());
        assert!(nbt.as_long().is_none());
        assert!(nbt.as_float().is_none());
        assert!(nbt.as_double().is_none());
        assert!(nbt.as_string().is_none());
        assert!(nbt.as_list().is_none());
        assert!(nbt.as_compound().is_none());
        assert!(nbt.as_byte_array().is_none());
        assert!(nbt.as_int_array().is_none());
        assert!(nbt.as_long_array().is_none());
        assert!(nbt.as_end().is_none());
        // get_compound 消耗所有权，非 Compound 返回 None
        assert!(nbt.get_compound().is_none());
    }

    /// Display 输出应符合 SNBT 风格
    #[test]
    fn display_format() {
        assert_eq!(byte(1).to_nbt().to_string(), "1b");
        assert_eq!(short(2).to_nbt().to_string(), "2s");
        assert_eq!(int(3).to_nbt().to_string(), "3");
        assert_eq!(long(4).to_nbt().to_string(), "4L");
        assert_eq!(float(1.5).to_nbt().to_string(), "1.5f");
        assert_eq!(double(2.5).to_nbt().to_string(), "2.5d");
        assert_eq!(string("a\"b").to_nbt().to_string(), "\"a\\\"b\"");
        assert_eq!(NbtType::end().to_string(), "END");

        assert_eq!(byte_array(vec![1, 2]).to_nbt().to_string(), "[B;1B, 2B]");
        assert_eq!(int_array(vec![1, 2]).to_nbt().to_string(), "[I;1, 2]");
        assert_eq!(long_array(vec![3]).to_nbt().to_string(), "[L;3L]");

        let mut list = list(NBT_INT_ORDER);
        assert!(list.add_item(int(1).to_nbt()));
        assert!(list.add_item(int(2).to_nbt()));
        assert_eq!(list.to_nbt().to_string(), "[1, 2]");

        let mut com = compound();
        com.data.insert("k".into(), byte(1).to_nbt());
        assert_eq!(com.to_nbt().to_string(), "{k: 1b}");
    }

    /// 数组类型序号常量与 get_num / get_nbt 的映射一致性，详见 lib.rs 测试
    #[test]
    fn type_orders() {
        assert_eq!(byte(0).to_nbt().get_num(), NBT_BYTE_ORDER);
        assert_eq!(byte_array(vec![]).to_nbt().get_num(), NBT_BYTE_ARRAY_ORDER);
        assert_eq!(long_array(vec![]).to_nbt().get_num(), NBT_LONG_ARRAY_ORDER);
        assert_eq!(list(NBT_INT_ORDER).to_nbt().get_num(), NBT_LIST_ORDER);
        assert_eq!(compound().to_nbt().get_num(), NBT_COMPOUND_ORDER);
        assert_eq!(int_array(vec![]).to_nbt().get_num(), NBT_INT_ARRAY_ORDER);
        assert_eq!(string("").to_nbt().get_num(), NBT_STRING_ORDER);
    }
}
