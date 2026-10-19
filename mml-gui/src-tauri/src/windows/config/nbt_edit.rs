//! NBT 编辑动作
//!
//! 按前端给的「改值 / 新增 / 删除」操作，在树上定位后用 JSON 形状的文本赋值。


use mml_nbt::nbt_types;
use mml_nbt::{NBT_END_ORDER, NbtType};

use crate::dtos::{NbtEditDto, NbtKindDto, NbtOpDto};
use crate::err_keys;



/// 数组（字节 / 整数 / 长整数）最多展开多少个元素给前端编辑
///
/// 短数组展开成元素行，一行一个下标；比这长的（高度图、位图之类）只报个数，也不接受增删，
/// 免得一次 IPC 白搬几万个数。前端那份常量要与此一致。
pub(super) const MAX_ARRAY_EDIT: usize = 1024;

/// 依次套用一批改动
///
/// - `root`: 根标签
/// - `edits`: 要套的改动，按发生顺序
///
/// # 返回值
///
/// 全部套成功返回 `Ok(())`；路径对不上返回 `err.nbtPath`，值解析不了返回 `err.nbtValue`
pub(super) fn apply_nbt_edits(root: &mut NbtType, edits: &[NbtEditDto]) -> Result<(), String> {
    for edit in edits {
        match edit.op {
            NbtOpDto::Set => set_nbt_at(root, &edit.path, &edit.value)?,
            NbtOpDto::Add => add_nbt_node(root, &edit.path, edit.kind, &edit.value)?,
            NbtOpDto::Rename => rename_nbt_node(root, &edit.path, &edit.value)?,
            NbtOpDto::Remove => remove_nbt_node(root, &edit.path)?,
        }
    }

    Ok(())
}

/// 按路径改一处值
///
/// 复合取成员、列表与数组取下标的元素；路径为空就是根标签自己。数组元素不是独立标签，
/// 只能在这里按下标落到具体的那一格上。
///
/// - `root`: 根标签
/// - `path`: 逐层的键名或下标
/// - `value`: 新值文本
///
/// # 返回值
///
/// 改成功返回 `Ok(())`；路径对不上返回 `err.nbtPath`，值解析不了返回 `err.nbtValue`
pub(super) fn set_nbt_at(root: &mut NbtType, path: &[String], value: &str) -> Result<(), String> {
    let Some((last, parent_path)) = path.split_last() else {
        return set_nbt_scalar(root, value);
    };
    let bad_path = || String::from(err_keys::NBT_PATH);

    match nbt_at_mut(root, parent_path)? {
        NbtType::Compound(compound) => {
            let node = compound.data.get_mut(last).ok_or_else(bad_path)?;
            set_nbt_scalar(node, value)
        }
        NbtType::List(list) => {
            let node = list.get_item_mut(parse_index(last)?).ok_or_else(bad_path)?;
            set_nbt_scalar(node, value)
        }
        NbtType::ByteArray(v) => set_array_item(&mut v.data, last, value),
        NbtType::IntArray(v) => set_array_item(&mut v.data, last, value),
        NbtType::LongArray(v) => set_array_item(&mut v.data, last, value),
        _ => Err(bad_path()),
    }
}

/// 路径里的一段下标：十进制，别的写法一律算路径对不上
pub(super) fn parse_index(segment: &str) -> Result<usize, String> {
    segment
        .parse::<usize>()
        .map_err(|_| String::from(err_keys::NBT_PATH))
}

/// 改数组里的一个元素，值按元素类型解析
///
/// - `data`: 数组本体
/// - `index`: 元素下标的十进制字符串
/// - `value`: 新值文本
///
/// # 返回值
///
/// 改成功返回 `Ok(())`；下标越界返回 `err.nbtPath`，值解析不了返回 `err.nbtValue`
pub(super) fn set_array_item<T: std::str::FromStr>(
    data: &mut [T],
    index: &str,
    value: &str,
) -> Result<(), String> {
    let slot = data
        .get_mut(parse_index(index)?)
        .ok_or_else(|| String::from(err_keys::NBT_PATH))?;
    *slot = value
        .trim()
        .parse::<T>()
        .map_err(|_| String::from(err_keys::NBT_VALUE))?;

    Ok(())
}

/// 按路径走到要改的那个标签
///
/// 复合按键名、列表按下标；路径为空就是根标签自己。
///
/// - `root`: 根标签
/// - `path`: 逐层的键名或列表下标
///
/// # 返回值
///
/// 目标标签的可变引用；走到非复合 / 非列表上、索引越界或下标不是整数时返回 `err.nbtPath`
pub(super) fn nbt_at_mut<'a>(root: &'a mut NbtType, path: &[String]) -> Result<&'a mut NbtType, String> {
    let mut cur = root;
    for segment in path {
        cur = match cur {
            NbtType::Compound(compound) => compound
                .data
                .get_mut(segment)
                .ok_or_else(|| String::from(err_keys::NBT_PATH))?,
            NbtType::List(list) => {
                let index = segment
                    .parse::<usize>()
                    .map_err(|_| String::from(err_keys::NBT_PATH))?;
                list.get_item_mut(index)
                    .ok_or_else(|| String::from(err_keys::NBT_PATH))?
            }
            _ => return Err(String::from(err_keys::NBT_PATH)),
        };
    }

    Ok(cur)
}

/// 按标签类型把文本解析成新值，只认标量
///
/// Byte 在本仓库里是 `u8`（0–255），与 `nbt_node` 的展示口径一致。
///
/// - `nbt`: 目标标签
/// - `value`: 新值文本
///
/// # 返回值
///
/// 解析成功返回 `Ok(())`；不是标量返回 `err.nbtPath`，值解析不了返回 `err.nbtValue`
pub(super) fn set_nbt_scalar(nbt: &mut NbtType, value: &str) -> Result<(), String> {
    let text = value.trim();
    let bad = || String::from(err_keys::NBT_VALUE);

    match nbt {
        NbtType::Byte(v) => v.data = text.parse().map_err(|_| bad())?,
        NbtType::Short(v) => v.data = text.parse().map_err(|_| bad())?,
        NbtType::Int(v) => v.data = text.parse().map_err(|_| bad())?,
        NbtType::Long(v) => v.data = text.parse().map_err(|_| bad())?,
        NbtType::Float(v) => v.data = text.parse().map_err(|_| bad())?,
        NbtType::Double(v) => v.data = text.parse().map_err(|_| bad())?,
        NbtType::String(v) => v.data = value.to_string(),
        _ => return Err(String::from(err_keys::NBT_PATH)),
    }

    Ok(())
}

/// 解析数组的值文本：逗号或空白分隔的一串数
///
/// 空串是空数组；元素个数超过 [`MAX_ARRAY_EDIT`] 直接拒掉，别让一次 IPC 塞进上万个数。
///
/// - `text`: 值文本，如 `1, 2, 3`
///
/// # 返回值
///
/// 解析出的元素；有解析不了的或超过上限时返回 `err.nbtValue`
pub(super) fn parse_array<T: std::str::FromStr>(text: &str) -> Result<Vec<T>, String> {
    let mut out = Vec::new();
    for item in text.split([',', ' ', '\t', '\r', '\n']) {
        if item.is_empty() {
            continue;
        }
        if out.len() >= MAX_ARRAY_EDIT {
            return Err(String::from(err_keys::NBT_VALUE));
        }
        out.push(
            item.parse::<T>()
                .map_err(|_| String::from(err_keys::NBT_VALUE))?,
        );
    }

    Ok(out)
}

/// 按路径改一个复合成员的键名
///
/// 只换键名，标签本体（类型、值、子树）原样搬到新键下；列表与数组的元素按序号认，
/// 没有键名可改，根标签自己也没有键名，三种都拒掉。
///
/// - `root`: 根标签
/// - `path`: 目标标签的路径，至少一段
/// - `new_name`: 新键名
///
/// # 返回值
///
/// 改成功返回 `Ok(())`；父容器不是复合、旧键不存在、新键已被占用或路径为空返回 `err.nbtPath`
pub(super) fn rename_nbt_node(root: &mut NbtType, path: &[String], new_name: &str) -> Result<(), String> {
    let (last, parent_path) = path
        .split_last()
        .ok_or_else(|| String::from(err_keys::NBT_PATH))?;

    match nbt_at_mut(root, parent_path)? {
        NbtType::Compound(compound) => {
            // 改成同名等于没改：别把它当成"新键已被占用"
            if last == new_name {
                return Ok(());
            }
            if compound.data.contains_key(new_name) {
                return Err(String::from(err_keys::NBT_PATH));
            }
            let node = compound
                .data
                .remove(last)
                .ok_or_else(|| String::from(err_keys::NBT_PATH))?;
            compound.data.insert(new_name.to_string(), node);
            Ok(())
        }
        _ => Err(String::from(err_keys::NBT_PATH)),
    }
}

/// 按路径新建一个标签并挂进父容器
///
/// 路径的最后一段在复合里当键名、在列表与数组里当插入下标（等于长度就是追加），前面几段
/// 照常走到父容器；空列表的元素类型跟随新元素，因为列表是同类型容器。数组的元素类型随
/// 数组自己，不看 `kind`。
///
/// - `root`: 根标签
/// - `path`: 新标签的路径，至少一段
/// - `kind`: 新标签的类型；缺省或为 [`NbtKindDto::End`] 都报错，父容器是数组时忽略
/// - `value`: 新标签的值文本
///
/// # 返回值
///
/// 挂上返回 `Ok(())`；父容器不是复合 / 列表 / 数组、键已存在、下标越界或元素类型对不上
/// 返回 `err.nbtPath`，值解析不了返回 `err.nbtValue`
pub(super) fn add_nbt_node(
    root: &mut NbtType,
    path: &[String],
    kind: Option<NbtKindDto>,
    value: &str,
) -> Result<(), String> {
    let (last, parent_path) = path
        .split_last()
        .ok_or_else(|| String::from(err_keys::NBT_PATH))?;
    let kind = kind.ok_or_else(|| String::from(err_keys::NBT_PATH))?;
    let node = make_nbt_node(kind, value)?;

    match nbt_at_mut(root, parent_path)? {
        NbtType::Compound(compound) => {
            if compound.data.contains_key(last) {
                return Err(String::from(err_keys::NBT_PATH));
            }
            compound.data.insert(last.clone(), node);
            Ok(())
        }
        NbtType::List(list) => {
            let index = parse_index(last)?;
            // 空列表还没有元素类型：跟着第一个元素定下来
            if list.len() == 0 {
                list.set_num(node.get_num());
            }
            if list.insert_item(index, node) {
                Ok(())
            } else {
                Err(String::from(err_keys::NBT_PATH))
            }
        }
        NbtType::ByteArray(v) => add_array_item(&mut v.data, last, value),
        NbtType::IntArray(v) => add_array_item(&mut v.data, last, value),
        NbtType::LongArray(v) => add_array_item(&mut v.data, last, value),
        _ => Err(String::from(err_keys::NBT_PATH)),
    }
}

/// 往数组里插一个元素，值按元素类型解析
///
/// 元素个数按 [`MAX_ARRAY_EDIT`] 封顶，别让界面上的数组长到展开不动。
///
/// - `data`: 数组本体
/// - `index`: 插入下标的十进制字符串，等于长度就是追加
/// - `value`: 新元素文本
///
/// # 返回值
///
/// 插上返回 `Ok(())`；下标越界或已到上限返回 `err.nbtPath`，值解析不了返回 `err.nbtValue`
pub(super) fn add_array_item<T: std::str::FromStr>(
    data: &mut Vec<T>,
    index: &str,
    value: &str,
) -> Result<(), String> {
    let index = parse_index(index)?;
    if index > data.len() || data.len() >= MAX_ARRAY_EDIT {
        return Err(String::from(err_keys::NBT_PATH));
    }
    let item = value
        .trim()
        .parse::<T>()
        .map_err(|_| String::from(err_keys::NBT_VALUE))?;
    data.insert(index, item);

    Ok(())
}

/// 按路径删掉一个标签
///
/// 路径的最后一段在复合里是键名、在列表与数组里是下标；根标签删不得，路径为空直接报错。
///
/// - `root`: 根标签
/// - `path`: 目标标签的路径，至少一段
///
/// # 返回值
///
/// 删掉返回 `Ok(())`；父容器不是复合 / 列表 / 数组、键不存在或下标越界返回 `err.nbtPath`
pub(super) fn remove_nbt_node(root: &mut NbtType, path: &[String]) -> Result<(), String> {
    let (last, parent_path) = path
        .split_last()
        .ok_or_else(|| String::from(err_keys::NBT_PATH))?;

    match nbt_at_mut(root, parent_path)? {
        NbtType::Compound(compound) => {
            if compound.data.remove(last).is_some() {
                Ok(())
            } else {
                Err(String::from(err_keys::NBT_PATH))
            }
        }
        NbtType::List(list) => {
            let index = parse_index(last)?;
            if index >= list.len() {
                return Err(String::from(err_keys::NBT_PATH));
            }
            list.remove(index);
            Ok(())
        }
        NbtType::ByteArray(v) => remove_array_item(&mut v.data, last),
        NbtType::IntArray(v) => remove_array_item(&mut v.data, last),
        NbtType::LongArray(v) => remove_array_item(&mut v.data, last),
        _ => Err(String::from(err_keys::NBT_PATH)),
    }
}

/// 删掉数组里的一个元素
///
/// - `data`: 数组本体
/// - `index`: 元素下标的十进制字符串
///
/// # 返回值
///
/// 删掉返回 `Ok(())`；下标越界返回 `err.nbtPath`
pub(super) fn remove_array_item<T>(data: &mut Vec<T>, index: &str) -> Result<(), String> {
    let index = parse_index(index)?;
    if index >= data.len() {
        return Err(String::from(err_keys::NBT_PATH));
    }
    data.remove(index);

    Ok(())
}

/// 按类型与值文本造一个新标签
///
/// 复合与列表造空的，值文本忽略，随后可以往里加成员 / 元素。
///
/// - `kind`: 标签类型，[`NbtKindDto::End`] 不可造
/// - `value`: 值文本
///
/// # 返回值
///
/// 新建的标签；类型为 End 返回 `err.nbtPath`，值解析不了返回 `err.nbtValue`
pub(super) fn make_nbt_node(kind: NbtKindDto, value: &str) -> Result<NbtType, String> {
    let text = value.trim();
    let bad = || String::from(err_keys::NBT_VALUE);

    let node = match kind {
        NbtKindDto::End => return Err(String::from(err_keys::NBT_PATH)),
        NbtKindDto::Byte => nbt_types::byte(text.parse().map_err(|_| bad())?).to_nbt(),
        NbtKindDto::Short => nbt_types::short(text.parse().map_err(|_| bad())?).to_nbt(),
        NbtKindDto::Int => nbt_types::int(text.parse().map_err(|_| bad())?).to_nbt(),
        NbtKindDto::Long => nbt_types::long(text.parse().map_err(|_| bad())?).to_nbt(),
        NbtKindDto::Float => nbt_types::float(text.parse().map_err(|_| bad())?).to_nbt(),
        NbtKindDto::Double => nbt_types::double(text.parse().map_err(|_| bad())?).to_nbt(),
        NbtKindDto::String => nbt_types::string(value).to_nbt(),
        NbtKindDto::ByteArray => nbt_types::byte_array(parse_array(value)?).to_nbt(),
        NbtKindDto::IntArray => nbt_types::int_array(parse_array(value)?).to_nbt(),
        NbtKindDto::LongArray => nbt_types::long_array(parse_array(value)?).to_nbt(),
        NbtKindDto::List => nbt_types::list(NBT_END_ORDER).to_nbt(),
        NbtKindDto::Compound => nbt_types::compound().to_nbt(),
    };

    Ok(node)
}
