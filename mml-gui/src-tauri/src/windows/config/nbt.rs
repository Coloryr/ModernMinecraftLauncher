//! NBT 树的转换与三条命令
//!
//! 打开的文件留在会话里，保存直接套在会话那份树上、由同一个句柄写回。

use mml_nbt::NbtType;
use uuid::Uuid;

use crate::dtos::{ConfigNbtDto, NbtEditDto, NbtKindDto, NbtNodeDto};
use crate::err_keys;

use crate::windows::{instance_snapshot, nbt_session};

use super::nbt_edit::{MAX_ARRAY_EDIT, apply_nbt_edits};
use super::source::{check_size, resolve_source};

/// 把一个 NBT 标签转成树上的节点
///
/// 数组在 [`MAX_ARRAY_EDIT`] 以内**展开成元素行**（元素不是独立标签，展开只是给前端一个
/// 可编辑的入口），超过就只报个数：一个高度图动辄上万项，铺开既慢也没人看，
/// 要看原始字节走二进制视图。
///
/// - `name`: 键名；列表元素传下标字符串，根节点传空串
/// - `nbt`: 标签本体
///
/// # 返回值
///
/// 见 [`NbtNodeDto`]；复合的成员按**键名排序**，HashMap 的顺序不可靠
pub(super) fn nbt_node(name: String, nbt: &NbtType) -> NbtNodeDto {
    /// 标量节点：只有类型与值
    fn leaf(name: String, kind: NbtKindDto, value: String) -> NbtNodeDto {
        NbtNodeDto {
            name,
            kind,
            value,
            length: 0,
            children: Vec::new(),
            items: Vec::new(),
        }
    }

    /// 数组节点：类型、元素个数，短数组连元素一起展开
    fn array<T: ToString>(
        name: String,
        kind: NbtKindDto,
        item_kind: NbtKindDto,
        data: &[T],
    ) -> NbtNodeDto {
        let items = if data.len() > MAX_ARRAY_EDIT {
            Vec::new()
        } else {
            data.iter()
                .enumerate()
                .map(|(index, item)| leaf(index.to_string(), item_kind, item.to_string()))
                .collect()
        };
        NbtNodeDto {
            name,
            kind,
            value: String::new(),
            length: data.len() as u32,
            children: Vec::new(),
            items,
        }
    }

    match nbt {
        NbtType::End(_) => leaf(name, NbtKindDto::End, String::new()),
        NbtType::Byte(v) => leaf(name, NbtKindDto::Byte, v.data.to_string()),
        NbtType::Short(v) => leaf(name, NbtKindDto::Short, v.data.to_string()),
        NbtType::Int(v) => leaf(name, NbtKindDto::Int, v.data.to_string()),
        NbtType::Long(v) => leaf(name, NbtKindDto::Long, v.data.to_string()),
        NbtType::Float(v) => leaf(name, NbtKindDto::Float, v.data.to_string()),
        NbtType::Double(v) => leaf(name, NbtKindDto::Double, v.data.to_string()),
        NbtType::String(v) => leaf(name, NbtKindDto::String, v.data.clone()),
        NbtType::ByteArray(v) => array(name, NbtKindDto::ByteArray, NbtKindDto::Byte, &v.data),
        NbtType::IntArray(v) => array(name, NbtKindDto::IntArray, NbtKindDto::Int, &v.data),
        NbtType::LongArray(v) => array(name, NbtKindDto::LongArray, NbtKindDto::Long, &v.data),
        NbtType::List(v) => {
            let items: Vec<NbtNodeDto> = v
                .iter()
                .enumerate()
                .map(|(index, item)| nbt_node(index.to_string(), item))
                .collect();
            let length = items.len() as u32;
            NbtNodeDto {
                name,
                kind: NbtKindDto::List,
                value: String::new(),
                length,
                children: Vec::new(),
                items,
            }
        }
        NbtType::Compound(v) => {
            let mut children: Vec<NbtNodeDto> = v
                .data
                .iter()
                .map(|(key, item)| nbt_node(key.clone(), item))
                .collect();
            children.sort_by(|a, b| a.name.cmp(&b.name));
            NbtNodeDto {
                name,
                kind: NbtKindDto::Compound,
                value: String::new(),
                length: 0,
                children,
                items: Vec::new(),
            }
        }
    }
}

/// 读取一个 NBT 文件，转成可展示、可编辑的树
///
/// - `game`: 实例 uuid
/// - `source`: 来源，空串为实例本身，否则是存档目录名
/// - `file`: 来源内的相对路径
///
/// 打开一个 NBT 文件，转成可展示、可编辑的树
///
/// **打开就是真的打开**：文件句柄与解析出的内容都留在会话里（见 [`nbt_session`]），
/// 之后的保存直接在那份状态上改、由同一个句柄写回。会话还在时再打开一次给的是会话里
/// 那棵树，不碰磁盘；要重新读盘就先 [`config_close_nbt`]，再打开。
///
/// - `game`: 实例 uuid
/// - `source`: 来源，空串为实例本身，否则是存档目录名
/// - `file`: 来源内的相对路径
/// - `index`: 文件是区域文件时的区块下标，整份文件为 `None`
///
/// # 返回值
///
/// 见 [`ConfigNbtDto`]：树 + **文件字节**的 SHA1（保存时按它核对）；
/// 路径非法返回 `err.fileName`，体积超过 [`MAX_FILE_SIZE`] 返回 `err.fileTooLarge`，
/// 不是 NBT 或解不开时返回内核错误串
#[gui_macros::ipc_group("config")]
#[gui_macros::command]
pub fn config_open_nbt(
    game: String,
    source: String,
    file: String,
    index: Option<u32>,
) -> Result<ConfigNbtDto, String> {
    // 会话按实例归属，关那个实例的配置窗口时只清它这一份，见 nbt_session
    let owner = Uuid::parse_str(&game).map_err(|_| err_keys::UUID.to_string())?;
    let game = instance_snapshot(&game)?;
    let path = resolve_source(&game, &source, &file)?;
    // 上限是给"整份树要塞进一次 IPC"定的：区域文件里只装一个区块，不该按整份 mca 卡
    if index.is_none() {
        check_size(&path)?;
    }

    let opened = nbt_session::open_with(&owner, &path, index, |nbt| {
        nbt_node(String::new(), &nbt.nbt)
    })?;
    Ok(ConfigNbtDto {
        node: opened.value,
        sha1: opened.sha1,
    })
}

/// 关掉一个 NBT 文件的会话，句柄跟着释放
///
/// 前端在切走配置窗口、或不想再保持这个文件打开时调用；下次打开就是重新读盘。
///
/// - `game`: 实例 uuid
/// - `source`: 来源，空串为实例本身，否则是存档目录名
/// - `file`: 来源内的相对路径，与 [`config_open_nbt`] 同一口径
/// - `index`: 文件是区域文件时的区块下标，整份文件为 `None`
///
/// # 返回值
///
/// 路径非法返回 `err.fileName`；没打开过的文件也当成功
#[gui_macros::ipc_group("config")]
#[gui_macros::command]
pub fn config_close_nbt(
    game: String,
    source: String,
    file: String,
    index: Option<u32>,
) -> Result<(), String> {
    let game = instance_snapshot(&game)?;
    let path = resolve_source(&game, &source, &file)?;

    nbt_session::close(&path, index);
    Ok(())
}

/// 按前端记下的一批改动改写 NBT 文件
///
/// 改动按顺序套用，见 [`NbtEditDto`]：改值 / 新增 / 删除三种。写回在**打开时那个句柄**上做
/// （见 [`nbt_session::update_with`]），不重新打开也不重新读盘；压缩方式沿用原文件。
/// 改之前核对前端那份基准 SHA1 与文件有没有被别处动过。
///
/// - `game`: 实例 uuid
/// - `source`: 来源，空串为实例本身，否则是存档目录名
/// - `file`: 来源内的相对路径，与 [`config_open_nbt`] 同一口径
/// - `index`: 文件是区域文件时的区块下标，整份文件为 `None`
/// - `base_sha1`: 打开时那一份文件字节的 SHA1
/// - `edits`: 要套的改动，按发生顺序
///
/// # 返回值
///
/// 写回后文件字节的 SHA1；路径非法返回 `err.fileName`，没打开过、基准对不上或文件被别处
/// 改过返回 `err.fileChanged`，只读文件返回 `err.fileReadOnly`，路径对不上返回 `err.nbtPath`，
/// 值解析不了或数组超上限返回 `err.nbtValue`
#[gui_macros::ipc_group("config")]
#[gui_macros::command]
pub fn config_set_nbt(
    game: String,
    source: String,
    file: String,
    index: Option<u32>,
    base_sha1: String,
    edits: Vec<NbtEditDto>,
) -> Result<String, String> {
    let game = instance_snapshot(&game)?;
    let path = resolve_source(&game, &source, &file)?;
    if index.is_none() {
        check_size(&path)?;
    }

    nbt_session::update_with(&path, index, &base_sha1, |nbt| {
        apply_nbt_edits(&mut nbt.nbt, &edits)
    })
}
