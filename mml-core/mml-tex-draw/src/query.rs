//! 查询接口
//!
//! 图标路径、创造分组、名称键与排序后的条目列表。
use std::path::PathBuf;

use mml_base::{archives::BaseArchive, serialize_tools};
use mml_sys::path_helper;

use crate::block::obj::{BlockFace2d, BlocksObj, ItemsObj};

use super::lang::*;

use crate::{block, order};

/// 获取方块图片路径
///
/// - `id`: 方块 ID，`minecraft:stone` 或裸 ID `stone`
///
/// # 返回值
///
/// 返回图标 PNG 路径，方块未渲染时返回 `None`
pub fn get_block_path(id: &str) -> Option<PathBuf> {
    let dir = BLOCK_DIR.get().unwrap();
    let binding = BLOCKS.read().unwrap();
    let file = binding.tex.get(id)?;

    Some(dir.join(file))
}

/// 方块是否有 2D 平面图标
///
/// - `id`: 方块 ID，`minecraft:stone` 或裸 ID `stone`
///
/// # 返回值
///
/// 渲染过 2D 图标时返回 `true`
pub fn has_block_2d(id: &str) -> bool {
    BLOCKS
        .read()
        .unwrap()
        .tex2d
        .get(id)
        .is_some_and(|groups| !groups.is_empty())
}

/// 方块的 2D 贴图分组
///
/// 顶面与底面共用同一张贴图时（石头、原木这类）会合成一组，界面上只给一个选项。
///
/// - `id`: 方块 ID，`minecraft:stone` 或裸 ID `stone`
///
/// # 返回值
///
/// 按顶 / 侧 / 底顺序排好的分组；没渲染过 2D 时返回空表
pub fn block_2d_faces(id: &str) -> Vec<BlockFace2d> {
    BLOCKS
        .read()
        .unwrap()
        .tex2d
        .get(id)
        .cloned()
        .unwrap_or_default()
}

/// 取方块的 2D 平面图标路径，取侧面，与 `get_block_path_2d_face(id, "side")` 等价
///
/// - `id`: 方块 ID，`minecraft:stone` 或裸 ID `stone`
///
/// # 返回值
///
/// 返回图标 PNG 路径，未渲染过 2D 图标时返回 `None`
pub fn get_block_path_2d(id: &str) -> Option<PathBuf> {
    get_block_path_2d_face(id, "side")
}

/// 取方块某个面的 2D 平面贴图路径
///
/// - `id`: 方块 ID，`minecraft:stone` 或裸 ID `stone`
/// - `face`: `top` / `side` / `bottom`，其它值按侧面
///
/// # 返回值
///
/// 返回该面的 PNG 路径；没渲染过或该面没有贴图时返回 `None`
pub fn get_block_path_2d_face(id: &str, face: &str) -> Option<PathBuf> {
    let dir = BLOCK2D_DIR.get()?;
    let binding = BLOCKS.read().unwrap();
    let groups = binding.tex2d.get(id)?;
    // 该面所在的分组；面名未知时退回第一组
    let group = groups
        .iter()
        .find(|group| group.faces.iter().any(|each| each == face))
        .or_else(|| groups.first())?;

    Some(dir.join(&group.file))
}

pub use block::icons::SpecialForm;

/// 判断方块 ID 是否有特殊形态图标，有则返回对应形态
///
/// - `id`: 方块 ID，带不带 `minecraft:` 前缀均可
///
/// # 返回值
///
/// 无特殊形态时返回 `None`
pub fn block_special_form(id: &str) -> Option<SpecialForm> {
    let base = id.strip_prefix("minecraft:").unwrap_or(id);
    block::icons::special_form(&format!("minecraft:{base}"))
}

/// 获取方块特殊形态的图片路径，形态图标 ID = 基础 ID + "_" + 形态后缀，如 oak_door_open
///
/// - `id`: 方块 ID，带不带 `minecraft:` 前缀均可
/// - `form`: 要取的形态
///
/// # 返回值
///
/// 该方块没有此形态时返回 `None`；形态条目不注册进方块列表，文件名按规则直接拼出
pub fn get_block_path_form(id: &str, form: SpecialForm) -> Option<PathBuf> {
    if block_special_form(id) != Some(form) {
        return None;
    }
    let base = id.strip_prefix("minecraft:").unwrap_or(id);
    Some(
        BLOCK_DIR
            .get()?
            .join(format!("minecraft_{base}_{}.png", form.suffix())),
    )
}

/// 获取方块数据目录
pub(crate) fn get_block_dir() -> Option<PathBuf> {
    BLOCK_DIR.get().cloned()
}

/// 获取 2D 平面方块图标目录
pub(crate) fn get_block2d_dir() -> Option<PathBuf> {
    BLOCK2D_DIR.get().cloned()
}

/// 获取方块语言文件目录
pub(crate) fn get_lang_dir() -> Option<PathBuf> {
    LANG_DIR.get().cloned()
}

/// jar 语言文件里提出的两张 ID → 语言键映射
///
/// 只存语言键，译文由界面按当前语言经 [`get_lang`] 查询
pub(crate) struct LangKeys {
    /// 方块 ID → 语言键，如 `minecraft:stone` → `block.minecraft.stone`
    pub(crate) block: std::collections::HashMap<String, String>,
    /// 物品 ID → 语言键，如 `minecraft:apple` → `item.minecraft.apple`
    pub(crate) item: std::collections::HashMap<String, String>,
}

/// 提取语言文件到 langs 目录
///
/// - `archive`: 客户端 jar 归档
///
/// # 返回值
///
/// 见 [`LangKeys`]；zh_cn 优先，en_us 兜底，两者都取不到时两张表均为空
pub(crate) fn extract_langs(archive: &BaseArchive) -> LangKeys {
    for name in ["zh_cn", "en_us"] {
        let Ok(data) = archive.read(&format!("assets/minecraft/lang/{name}.json")) else {
            continue;
        };
        // 复制到 langs 目录，供 get_lang 查询翻译
        if let Some(dir) = get_lang_dir() {
            let _ = path_helper::write_bytes(dir.join(format!("{name}.json")), &data);
        }
        let Ok(buf) = String::from_utf8(data) else {
            continue;
        };
        let Ok(lang) =
            serialize_tools::json_from_str::<std::collections::HashMap<String, String>>(&buf)
        else {
            continue;
        };
        // 方块与物品 ID → 语言键：block.minecraft.stone → minecraft:stone
        // Name 只存语言键，由 GUI 按当前语言经 get_lang 翻译
        let mut block_map = std::collections::HashMap::new();
        let mut item_map = std::collections::HashMap::new();
        for lang_key in lang.into_keys() {
            let keys: Vec<&str> = lang_key.split('.').collect();
            if keys.len() == 3 && keys[0] == "block" {
                block_map.insert(format!("{}:{}", keys[1], keys[2]), lang_key.clone());
            } else if keys.len() == 3 && keys[0] == "item" {
                item_map.insert(format!("{}:{}", keys[1], keys[2]), lang_key);
            }
        }
        return LangKeys {
            block: block_map,
            item: item_map,
        };
    }
    LangKeys {
        block: std::collections::HashMap::new(),
        item: std::collections::HashMap::new(),
    }
}

/// 获取方块数据，集合语义，顺序无意义
///
/// 底层是 `HashMap::keys()`，每次调用顺序都可能不同；要能直接展示的顺序用
/// [`ordered_entries`]
///
/// # 返回值
///
/// 全部方块 ID 的列表，顺序不固定；没有渲染结果时为空列表
pub fn blocks() -> Vec<String> {
    BLOCKS
        .read()
        .unwrap()
        .tex
        .keys()
        .map(|item| item.clone())
        .collect()
}

/// 方块数据的渲染版本，未渲染过为空串
///
/// # 返回值
///
/// 游戏版本号；未渲染过是空串
pub fn block_version() -> String {
    BLOCKS.read().unwrap().id.clone()
}

/// 是否已有渲染结果，方块或物品任一非空
///
/// 给"只想知道有没有渲染过"的调用方用：[`blocks`] / [`items`] 都是整表克隆，
/// 拿它们判空等于每次白拷两份全表，mml-gui 的方块状态快照就在每个进度回调里调
///
/// # 返回值
///
/// 方块或物品任一有结果时返回 `true`
pub fn has_rendered() -> bool {
    !BLOCKS.read().unwrap().tex.is_empty() || !ITEMS.read().unwrap().tex.is_empty()
}

/// 方块 ID → 语言键，如 block.minecraft.stone，翻译经 `get_lang`
///
/// - `id`: 方块 ID
///
/// # 返回值
///
/// 语言键，形如 `block.minecraft.stone`；ID 不在表里返回 `None`
pub fn block_name_key(id: &str) -> Option<String> {
    BLOCKS.read().unwrap().name.get(id).cloned()
}

/// 物品数据的渲染版本，未渲染过为空串
///
/// # 返回值
///
/// 游戏版本号；未渲染过是空串
pub fn item_version() -> String {
    ITEMS.read().unwrap().id.clone()
}

/// 物品 ID → 语言键，如 item.minecraft.apple，翻译经 `get_lang`
///
/// - `id`: 物品 ID
///
/// # 返回值
///
/// 语言键，形如 `item.minecraft.apple`；ID 不在表里返回 `None`
pub fn item_name_key(id: &str) -> Option<String> {
    ITEMS.read().unwrap().name.get(id).cloned()
}

/// 锁定方块数据表，内部写入用
pub(crate) fn blocks_write() -> std::sync::RwLockWriteGuard<'static, BlocksObj> {
    BLOCKS.write().unwrap()
}

/// 锁定方块数据表，内部读取用
pub(crate) fn blocks_read() -> std::sync::RwLockReadGuard<'static, BlocksObj> {
    BLOCKS.read().unwrap()
}

/// 获取物品图片路径
///
/// - `id`: 物品 ID，`minecraft:apple` 或裸 ID `apple`
///
/// # 返回值
///
/// 返回图标 PNG 路径，物品未渲染时返回 `None`
pub fn get_item_path(id: &str) -> Option<PathBuf> {
    let dir = ITEM_DIR.get().unwrap();
    let binding = ITEMS.read().unwrap();
    let file = binding.tex.get(id)?;

    Some(dir.join(file))
}

/// 获取方块创造分组，值为 itemGroup lang 键尾段，翻译经 `get_lang`
///
/// - `id`: 方块 ID
///
/// # 返回值
///
/// 分组键尾段；ID 不在表里或没有分组返回 `None`
pub fn block_cat(id: &str) -> Option<String> {
    BLOCKS.read().unwrap().cat.get(id).cloned()
}

/// 获取物品创造分组，值与 `block_cat` 同一套
///
/// - `id`: 物品 ID
///
/// # 返回值
///
/// 与 `block_cat` 同一套的分组键尾段；ID 不在表里或没有分组返回 `None`
pub fn item_cat(id: &str) -> Option<String> {
    ITEMS.read().unwrap().cat.get(id).cloned()
}

/// 获取物品数据目录
pub(crate) fn get_item_dir() -> Option<PathBuf> {
    ITEM_DIR.get().cloned()
}

/// 获取物品数据，集合语义，顺序无意义
///
/// 同 [`blocks`]，要能直接展示的顺序用 [`ordered_entries`]
///
/// # 返回值
///
/// 全部物品 ID 的列表，顺序不固定；没有渲染结果时为空列表
pub fn items() -> Vec<String> {
    ITEMS
        .read()
        .unwrap()
        .tex
        .keys()
        .map(|item| item.clone())
        .collect()
}

/// 锁定物品数据表，内部写入用
pub(crate) fn items_write() -> std::sync::RwLockWriteGuard<'static, ItemsObj> {
    ITEMS.write().unwrap()
}

/// 列表条目的来源，决定图标走 `block/` 还是 `item/`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// 方块，等轴测 3D 图标
    Block,
    /// 物品，平面精灵图标
    Item,
}

/// 一个待展示的方块 / 物品条目
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// 分类值，来自 `block_cat` / `item_cat`；显示名由调用方按当前语言翻译
    pub cat: String,
    /// 来源，方块还是物品
    pub kind: EntryKind,
    /// 条目 id，如 `minecraft:stone`
    pub id: String,
}

/// 方块与物品合并后的展示顺序，列表窗口直接用这个
///
/// 全局按 [`order::order_key`] 排序，所以同一分类只出现一次、分类先后即创造栏标签页顺序；
/// 方块与物品分开排再拼接是错的，只有物品的分类会被挤到最后
///
/// - 同一个 id 既是方块又是物品时只保留方块那条，避免重复条目
/// - 分类值来自 `block_cat` / `item_cat`，显示名由调用方按当前语言翻译
///
/// # 返回值
///
/// 按展示顺序排列的条目，见 [`Entry`]
pub fn ordered_entries() -> Vec<Entry> {
    let mut list: Vec<Entry> = Vec::new();

    let all_blocks = blocks();
    let block_ids: std::collections::HashSet<&str> =
        all_blocks.iter().map(|id| id.as_str()).collect();
    for id in &all_blocks {
        let cat = block_cat(id).unwrap_or_default();
        list.push(Entry {
            cat,
            kind: EntryKind::Block,
            id: id.clone(),
        });
    }
    for id in items() {
        if block_ids.contains(id.as_str()) {
            continue;
        }
        let cat = item_cat(&id).unwrap_or_default();
        list.push(Entry {
            cat,
            kind: EntryKind::Item,
            id,
        });
    }

    list.sort_by(|a, b| order::order_key(&a.cat, &a.id).cmp(&order::order_key(&b.cat, &b.id)));
    list
}

/// 锁定物品数据表，内部读取用
pub(crate) fn items_read() -> std::sync::RwLockReadGuard<'static, ItemsObj> {
    ITEMS.read().unwrap()
}
