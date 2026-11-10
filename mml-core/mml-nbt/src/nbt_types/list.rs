//! 列表标签
//!
//! 单元素类型加长度，元素类型在写入时锁定。

use std::io::{Read, Write};

use crate::{NbtStream, NbtType, io_error, is_nbt_num};
use mml_names::i18_items::error_type::{CoreResult, ErrorType};

/// NBT 列表类型，对应 TAG_List
///
/// 存储一组同类型的 NBT 元素，二进制流格式为 1 字节元素类型序号、
/// 4 字节元素个数，大端序 i32，后跟各元素的 NBT 数据；元素自身不带类型序号。
///
/// # 注意
///
/// 不允许混合不同类型的元素，修改列表类型时会清空已有数据。
#[derive(Default)]
pub struct NbtList {
    /// 元素列表，每个元素的类型序号都必须等于 `nbt_num`
    data: Vec<NbtType>,
    /// 元素类型序号，取值 0–12；空列表写出时会临时改写成 0
    nbt_num: u8,
}

impl NbtList {
    /// 创建指定元素类型的空 List 标签
    ///
    /// 序号不做校验，非法序号要到读回时才会报 `NbtTypeError`。
    ///
    /// - `nbt_num`: 元素类型序号，取值 0–12
    ///
    /// # 返回值
    ///
    /// 新建的空 `NbtList` 实例
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
    ///
    /// # 返回值
    ///
    /// 元素数量，空列表为 0
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
    ///
    /// # 返回值
    ///
    /// 一致时返回 `true`，否则返回 `false`
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
    ///
    /// # 返回值
    ///
    /// 对应的 `NbtType` 变体
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
